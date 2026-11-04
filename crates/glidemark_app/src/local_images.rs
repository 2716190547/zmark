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
pub const MAX_CACHE_BYTES: u64 = 64 * 1024 * 1024;

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

#[derive(Debug)]
pub struct LocalImage {
    pub snapshot: NamedTempFile,
    pub width: u32,
    pub height: u32,
    pub decoded_bytes: u64,
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
            let decoder = reader
                .into_decoder()
                .map_err(|e| format!("图片损坏或解码超限：{e}"))?;
            let (w, h) = decoder.dimensions();
            check_dimensions(w, h)?;
            // ImageDecoder reads the static first frame, never collects animation frames.
            let decoded =
                DynamicImage::from_decoder(decoder).map_err(|e| format!("图片解码失败：{e}"))?;
            let used = check_dimensions(decoded.width(), decoded.height())?;
            if matches!(format, ImageFormat::Gif | ImageFormat::WebP) {
                let mut png = Cursor::new(Vec::new());
                decoded
                    .write_to(&mut png, ImageFormat::Png)
                    .map_err(|e| e.to_string())?;
                (w, h, used, ".png", png.into_inner())
            } else {
                (
                    w,
                    h,
                    used,
                    if format == ImageFormat::Png {
                        ".png"
                    } else {
                        ".jpg"
                    },
                    bytes,
                )
            }
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
            // GPUI's resource loader rasterizes SVG at 2x. Budget the real bitmap.
            let w = (tree.size().width() * 2.0) as u32;
            let h = (tree.size().height() * 2.0) as u32;
            let used = check_dimensions(w, h)?;
            (w, h, used, ".svg", bytes)
        }
    };
    let mut snapshot = tempfile::Builder::new()
        .prefix("glidemark-image-")
        .suffix(extension)
        .tempfile()
        .map_err(|e| format!("图片快照写入失败：{e}"))?;
    snapshot
        .write_all(&output)
        .map_err(|e| format!("图片快照写入失败：{e}"))?;
    Ok(Arc::new(LocalImage {
        snapshot,
        width,
        height,
        decoded_bytes,
    }))
}

#[derive(Debug, Clone)]
pub enum ImageState {
    Waiting,
    Ready(Arc<LocalImage>),
    Failed(String),
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
    pub loading: Option<Arc<LocalImage>>,
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
            Ok(_) => ImageState::Failed("可见图片缓存超过 64MiB，请减少同时显示的图片".into()),
            Err(error) => ImageState::Failed(error),
        };
        self.entries.insert(url, state);
        true
    }
}
