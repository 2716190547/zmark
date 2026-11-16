//! Local Markdown images: bounded immutable snapshots, one background load at a time.
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use std::collections::{BTreeSet, HashMap};
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tempfile::NamedTempFile;

pub const MAX_INPUT_BYTES: u64 = 20 * 1024 * 1024;
pub const MAX_DECODED_BYTES: u64 = 64_000_000;
/// Budget for decoded images visible in the preview. GPUI retains both this
/// CPU buffer and a matching GPU texture, so the effective peak is roughly 2x.
pub const MAX_CACHE_BYTES: u64 = 48 * 1024 * 1024;
/// Largest side kept in the on-disk snapshot. The preview column is 760pt wide
/// and caps image height at 400pt; at 2x display density that needs at most
/// 1600px on either side, so larger source pixels only wasted texture memory.
pub const MAX_TEXTURE_SIDE: u32 = 1600;

pub fn resolve_image_path(reference: &str, document: Option<&Path>) -> Result<PathBuf, String> {
    let path = reference.split('#').next().unwrap_or("");
    if path.contains('?') {
        return Err("不支持本地图片查询串".into());
    }
    // A colon denotes a URI scheme on macOS; no URI is ever handed to GPUI.
    if path
        .split('/')
        .next()
        .is_some_and(|part| part.contains(':'))
    {
        return Err("仅加载本地路径，不支持网络或 URI 图片".into());
    }
    let decoded = percent_encoding::percent_decode_str(path)
        .decode_utf8()
        .map_err(|_| "图片路径不是有效 UTF-8")?;
    if decoded.is_empty() || decoded.contains('\0') {
        return Err("图片路径为空或含 NUL".into());
    }
    let path = PathBuf::from(decoded.as_ref());
    if path.is_absolute() {
        return Ok(path);
    }
    let parent = document
        .and_then(Path::parent)
        .filter(|p| p.is_absolute())
        .ok_or("未保存文档无法解析相对图片路径")?;
    Ok(parent.join(path))
}

pub fn check_dimensions(width: u32, height: u32) -> Result<u64, String> {
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|n| n.checked_mul(4))
        .ok_or("图片尺寸溢出")?;
    if width == 0 || height == 0 || width > 4000 || height > 4000 || bytes > MAX_DECODED_BYTES {
        return Err("图片超限：最大边 4000px，最多 1600 万像素".into());
    }
    Ok(bytes)
}

fn fit_within(width: u32, height: u32, max_side: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= max_side {
        return (width, height);
    }
    let scale = max_side as f64 / longest as f64;
    (
        ((width as f64 * scale).round() as u32).max(1),
        ((height as f64 * scale).round() as u32).max(1),
    )
}

#[derive(Debug)]
pub struct LocalImage {
    pub snapshot: Option<NamedTempFile>,
    pub render: Option<Arc<gpui::RenderImage>>,
    pub source_path: PathBuf,
    pub width: u32,
    pub height: u32,
    /// Logical source dimensions, before snapshot resampling or SVG 2x rasterization.
    pub natural_size: (f32, f32),
    pub decoded_bytes: u64,
}

impl LocalImage {
    pub fn path(&self) -> &Path {
        self.snapshot.as_ref().map_or(self.source_path.as_path(), |snapshot| snapshot.path())
    }
    pub fn image_source(&self) -> gpui::ImageSource {
        self.render.as_ref().map_or_else(|| self.path().to_path_buf().into(), |render| render.clone().into())
    }
    pub fn release(&self, window: &mut gpui::Window, cx: &mut gpui::App) {
        if let Some(render) = &self.render { cx.drop_image(render.clone(), Some(window)); }
        else { crate::views::lightbox::release_lightbox_image(self.path(), window, cx); }
    }
}

pub fn load_image(reference: &str, document: Option<&Path>) -> Result<Arc<LocalImage>, String> {
    let path = resolve_image_path(reference, document)?;
    if !std::fs::metadata(&path)
        .map_err(|e| format!("无法读取图片：{e}"))?
        .is_file()
    {
        return Err("图片路径必须是普通文件".into());
    }
    let file = File::open(&path).map_err(|e| format!("无法读取图片：{e}"))?;
    if file.metadata().map_err(|e| e.to_string())?.len() > MAX_INPUT_BYTES {
        return Err("图片文件超过 20MiB".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err("图片文件超过 20MiB".into());
    }

    let natural_size;
    let (width, height, decoded_bytes, extension, output) = match image::guess_format(&bytes) {
        Ok(
            format @ (ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Gif | ImageFormat::WebP),
        ) => {
            let mut limits = Limits::default();
            limits.max_image_width = Some(4000);
            limits.max_image_height = Some(4000);
            limits.max_alloc = Some(MAX_DECODED_BYTES);
            let mut reader = ImageReader::with_format(Cursor::new(&bytes), format);
            reader.limits(limits);
            let mut decoder = reader
                .into_decoder()
                .map_err(|e| format!("图片损坏或解码超限：{e}"))?;
            let (w, h) = decoder.dimensions();
            check_dimensions(w, h)?;
            // ImageDecoder reads the static first frame, never collects animation frames.
            let orientation = decoder.orientation().map_err(|e| format!("图片方向读取失败：{e}"))?;
            let mut decoded =
                DynamicImage::from_decoder(decoder).map_err(|e| format!("图片解码失败：{e}"))?;
            decoded.apply_orientation(orientation);
            natural_size = (decoded.width() as f32, decoded.height() as f32);
            let (out_w, out_h) = fit_within(decoded.width(), decoded.height(), MAX_TEXTURE_SIDE);
            let resized = (out_w, out_h) != (decoded.width(), decoded.height());
            if resized {
                decoded = decoded.resize(out_w, out_h, image::imageops::FilterType::Lanczos3);
            }
            // `resize` preserves the aspect ratio and can round the requested
            // side down; report the bitmap we actually wrote so the GPUI
            // decoder-size check and cache budget stay accurate.
            let (out_w, out_h) = (decoded.width(), decoded.height());
            let used = check_dimensions(out_w, out_h)?;
            let mut pixels = decoded.into_rgba8();
            for pixel in pixels.pixels_mut() { pixel.0.swap(0, 2); }
            let render = Arc::new(gpui::RenderImage::new(vec![image::Frame::new(pixels)]));
            return Ok(Arc::new(LocalImage { snapshot: None, render: Some(render), source_path: path,
                width: out_w, height: out_h, natural_size, decoded_bytes: used }));
        }
        Ok(_) => return Err("不支持此图片格式".into()),
        Err(_) => {
            let forbidden = Arc::new(AtomicBool::new(false));
            let data_flag = forbidden.clone();
            let string_flag = forbidden.clone();
            let mut options = usvg::Options::default();
            options.image_href_resolver = usvg::ImageHrefResolver {
                resolve_data: Box::new(move |_, _, _| {
                    data_flag.store(true, Ordering::Relaxed);
                    None
                }),
                resolve_string: Box::new(move |_, _| {
                    string_flag.store(true, Ordering::Relaxed);
                    None
                }),
            };
            let tree = usvg::Tree::from_str(
                std::str::from_utf8(&bytes).map_err(|_| "不支持此图片格式，SVG 须为 UTF-8")?,
                &options,
            )
            .map_err(|e| format!("图片损坏或格式不支持：{e}"))?;
            if forbidden.load(Ordering::Relaxed) {
                return Err("不支持 SVG 内嵌位图或外部图像".into());
            }
            natural_size = (tree.size().width(), tree.size().height());
            // GPUI's resource loader rasterizes SVG at 2x. Budget the real bitmap.
            let w = (tree.size().width() * 2.0) as u32;
            let h = (tree.size().height() * 2.0) as u32;
            let used = check_dimensions(w, h)?;
            (w, h, used, ".svg", bytes)
        }
    };
    let mut snapshot = tempfile::Builder::new()
        .prefix("zmark-image-")
        .suffix(extension)
        .tempfile()
        .map_err(|e| format!("图片快照写入失败：{e}"))?;
    snapshot
        .write_all(&output)
        .map_err(|e| format!("图片快照写入失败：{e}"))?;
    Ok(Arc::new(LocalImage {
        snapshot: Some(snapshot),
        render: None,
        source_path: path,
        width,
        height,
        natural_size,
        decoded_bytes,
    }))
}

#[derive(Debug, Clone)]
pub enum ImageState {
    Waiting,
    Ready(Arc<LocalImage>),
    Failed(String),
    RecoveryCandidates(Vec<RecoveryCandidate>),
}

pub type SharedImages = Arc<Mutex<LocalImages>>;

#[derive(Default)]
pub struct LocalImages {
    pub generation: u64,
    pub document: Option<PathBuf>,
    identity: Option<(Option<crate::state::DocKey>, Option<PathBuf>)>,
    pub wanted: BTreeSet<String>,
    pub entries: HashMap<String, ImageState>,
    pub retired: Vec<Arc<LocalImage>>,
    /// Prepared snapshot waiting for GPUI's native decode and completion notification.
    pub loading: Option<(u64, String, Arc<LocalImage>)>,
}

impl LocalImages {
    pub fn refresh(&mut self) {
        self.identity = None;
        self.generation = self.generation.wrapping_add(1);
        self.wanted.clear();
    }

    pub fn begin_frame(&mut self, key: Option<crate::state::DocKey>, path: Option<PathBuf>) {
        let identity = Some((key, path.clone()));
        if identity != self.identity {
            self.generation = self.generation.wrapping_add(1);
            self.document = path;
            self.identity = identity;
            if let Some((_, _, image)) = self.loading.take() {
                self.retired.push(image);
            }
            for (_, state) in self.entries.drain() {
                if let ImageState::Ready(image) = state {
                    self.retired.push(image);
                }
            }
        }
        self.wanted.clear();
    }

    pub fn request(&mut self, url: &str) -> ImageState {
        self.wanted.insert(url.to_owned());
        self.entries
            .entry(url.to_owned())
            .or_insert(ImageState::Waiting)
            .clone()
    }

    pub fn evict_hidden(&mut self) {
        self.entries.retain(|url, state| {
            if self.wanted.contains(url) {
                return true;
            }
            if let ImageState::Ready(image) = state {
                self.retired.push(image.clone());
            }
            false
        });
    }

    pub fn decoded_bytes(&self) -> u64 {
        self.entries
            .values()
            .filter_map(|s| {
                if let ImageState::Ready(i) = s {
                    Some(i.decoded_bytes)
                } else {
                    None
                }
            })
            .sum()
    }

    pub fn complete(
        &mut self,
        generation: u64,
        url: String,
        result: Result<Arc<LocalImage>, String>,
    ) -> bool {
        if generation != self.generation || !self.wanted.contains(&url) {
            return false;
        }
        self.evict_hidden();
        let state = match result {
            Ok(image) if self.decoded_bytes() + image.decoded_bytes <= MAX_CACHE_BYTES => {
                ImageState::Ready(image)
            }
            Ok(_) => ImageState::Failed(format!(
                "可见图片缓存超过 {}MiB，请减少同时显示的图片",
                MAX_CACHE_BYTES / (1024 * 1024)
            )),
            Err(error) => ImageState::Failed(error),
        };
        self.entries.insert(url, state);
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryCandidate {
    pub relative: PathBuf,
    pub canonical: PathBuf,
    pub size: u64,
    pub modified: Option<std::time::SystemTime>,
    #[cfg(unix)]
    pub device: u64,
    #[cfg(unix)]
    pub inode: u64,
}

pub fn is_windows_absolute_image(url: &str) -> bool {
    let b = url.as_bytes();
    b.len() >= 4 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

/// Search only the document directory and its direct, non-symlink assets folder.
pub fn image_recovery_candidates(url: &str, document: Option<&Path>) -> Vec<RecoveryCandidate> {
    if !is_windows_absolute_image(url) { return Vec::new(); }
    let path = url.split('#').next().unwrap_or("");
    if path.contains('?') { return Vec::new(); }
    let Some(raw_name) = path.rsplit(['\\', '/']).next().filter(|n| !n.is_empty() && *n != "." && *n != "..") else { return Vec::new(); };
    let Ok(name) = percent_encoding::percent_decode_str(raw_name).decode_utf8() else { return Vec::new(); };
    let name = name.as_ref();
    if name.contains(['/', '\\', '\0']) || name == "." || name == ".." { return Vec::new(); }
    let Some(base) = document.and_then(Path::parent).and_then(|p| p.canonicalize().ok()) else { return Vec::new(); };
    let mut found = Vec::new();
    for rel_dir in [Path::new(""), Path::new("assets")] {
        let dir = base.join(rel_dir);
        let Ok(dir_meta) = std::fs::symlink_metadata(&dir) else { continue; };
        if dir_meta.file_type().is_symlink() || !dir_meta.is_dir() { continue; }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue; };
        for entry in entries.flatten() {
            if entry.file_name() != std::ffi::OsStr::new(name) { continue; }
            let Ok(meta) = std::fs::symlink_metadata(entry.path()) else { continue; };
            if meta.file_type().is_symlink() || !meta.is_file() { continue; }
            let Ok(canonical) = entry.path().canonicalize() else { continue; };
            if canonical.parent() != Some(dir.as_path()) { continue; }
            #[cfg(unix)]
            use std::os::unix::fs::MetadataExt;
            found.push(RecoveryCandidate {
                relative: rel_dir.join(name), canonical, size: meta.len(), modified: meta.modified().ok(),
                #[cfg(unix)] device: meta.dev(),
                #[cfg(unix)] inode: meta.ino(),
            });
        }
    }
    found
}

pub fn validate_image_recovery(url: &str, document: Option<&Path>, expected: &RecoveryCandidate) -> bool {
    image_recovery_candidates(url, document).iter().any(|candidate| candidate == expected)
}

pub fn markdown_relative_image_path(path: &Path) -> Option<String> {
    use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
    const ENCODE: &AsciiSet = &CONTROLS.add(b' ').add(b'#').add(b'?').add(b'%').add(b'&').add(b'(').add(b')').add(b'<').add(b'>').add(b'\\');
    if path.is_absolute() { return None; }
    let mut parts = Vec::new();
    for component in path.components() {
        let std::path::Component::Normal(part) = component else { return None; };
        parts.push(utf8_percent_encode(part.to_str()?, ENCODE).to_string());
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Preserve a URI fragment while escaping characters that Markdown may reinterpret.
pub fn recovered_image_reference(path: &Path, original_url: &str) -> Option<String> {
    use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
    const FRAGMENT_ENCODE: &AsciiSet = &CONTROLS.add(b' ').add(b'&').add(b'(').add(b')').add(b'<').add(b'>').add(b'\\');
    let mut result = markdown_relative_image_path(path)?;
    if let Some((_, fragment)) = original_url.split_once('#') {
        result.push('#');
        result.push_str(&utf8_percent_encode(fragment, FRAGMENT_ENCODE).to_string());
    }
    Some(result)
}
