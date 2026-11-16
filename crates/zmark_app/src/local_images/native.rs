//! Image I/O downsamples large opaque PNG/JPEG images before allocating pixels.
use core_foundation::{
    base::{kCFAllocatorDefault, kCFAllocatorNull, CFType, CFTypeRef, TCFType},
    boolean::CFBoolean,
    data::{CFData, CFDataCreateWithBytesNoCopy, CFDataRef},
    dictionary::{CFDictionary, CFDictionaryRef},
    number::CFNumber,
    string::{CFString, CFStringRef},
};
use core_graphics::{
    color_space::{kCGColorSpaceSRGB, CGColorSpace},
    context::CGContext,
    geometry::{CGPoint, CGRect, CGSize},
    image::{CGImage, CGImageAlphaInfo, CGImageByteOrderInfo},
};
use foreign_types::ForeignType;
use image::{ImageDecoder, ImageFormat, RgbaImage};

#[link(name = "ImageIO", kind = "framework")]
extern "C" {
    fn CGImageSourceCreateWithData(data: CFDataRef, options: CFDictionaryRef) -> CFTypeRef;
    fn CGImageSourceCopyPropertiesAtIndex(
        source: CFTypeRef,
        index: usize,
        options: CFDictionaryRef,
    ) -> CFDictionaryRef;
    fn CGImageSourceCreateThumbnailAtIndex(
        source: CFTypeRef,
        index: usize,
        options: CFDictionaryRef,
    ) -> core_graphics::sys::CGImageRef;
    static kCGImageSourceShouldCache: CFStringRef;
    static kCGImageSourceCreateThumbnailFromImageAlways: CFStringRef;
    static kCGImageSourceCreateThumbnailWithTransform: CFStringRef;
    static kCGImageSourceThumbnailMaxPixelSize: CFStringRef;
    static kCGImagePropertyPixelWidth: CFStringRef;
    static kCGImagePropertyPixelHeight: CFStringRef;
    static kCGImagePropertyOrientation: CFStringRef;
}

pub(super) fn thumbnail(
    bytes: &[u8],
    format: ImageFormat,
) -> Result<Option<(RgbaImage, (f32, f32))>, String> {
    // The no-copy data and every Image I/O object stay in this function; only
    // owned pixels escape, so the borrowed input remains alive until CFRelease.
    unsafe {
        let raw = CFDataCreateWithBytesNoCopy(
            kCFAllocatorDefault,
            bytes.as_ptr(),
            bytes.len() as isize,
            kCFAllocatorNull,
        );
        if raw.is_null() {
            return Err("图片数据分配失败".into());
        }
        let data = CFData::wrap_under_create_rule(raw);
        let cache_key = CFString::wrap_under_get_rule(kCGImageSourceShouldCache);
        let options =
            CFDictionary::from_CFType_pairs(&[(cache_key, CFBoolean::false_value().as_CFType())]);
        let raw =
            CGImageSourceCreateWithData(data.as_concrete_TypeRef(), options.as_concrete_TypeRef());
        if raw.is_null() {
            return Err("图片损坏或格式不支持".into());
        }
        let source = CFType::wrap_under_create_rule(raw);
        let raw = CGImageSourceCopyPropertiesAtIndex(
            source.as_CFTypeRef(),
            0,
            options.as_concrete_TypeRef(),
        );
        if raw.is_null() {
            return Err("无法读取图片尺寸".into());
        }
        let properties: CFDictionary<CFString, CFType> = CFDictionary::wrap_under_create_rule(raw);
        let number = |key: CFStringRef| {
            properties
                .find(&CFString::wrap_under_get_rule(key))
                .and_then(|value| value.downcast::<CFNumber>())
                .and_then(|value| value.to_i64())
        };
        let width = number(kCGImagePropertyPixelWidth)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or("图片宽度无效")?;
        let height = number(kCGImagePropertyPixelHeight)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or("图片高度无效")?;
        super::check_dimensions(width, height)?;
        // Core Graphics premultiplies alpha. Keep the existing decoder for
        // alpha PNGs so their straight-alpha pixels retain full precision.
        if width.max(height) <= super::MAX_TEXTURE_SIDE {
            return Ok(None);
        }
        if format == ImageFormat::Png {
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(4000);
            limits.max_image_height = Some(4000);
            limits.max_alloc = Some(super::MAX_DECODED_BYTES);
            let decoder =
                image::codecs::png::PngDecoder::with_limits(std::io::Cursor::new(bytes), limits)
                    .map_err(|e| format!("图片损坏或解码超限：{e}"))?;
            if decoder.color_type().has_alpha() {
                return Ok(None);
            }
        }
        let natural = if matches!(number(kCGImagePropertyOrientation), Some(5..=8)) {
            (height as f32, width as f32)
        } else {
            (width as f32, height as f32)
        };
        let options = CFDictionary::from_CFType_pairs(&[
            (
                CFString::wrap_under_get_rule(kCGImageSourceCreateThumbnailFromImageAlways),
                CFBoolean::true_value().as_CFType(),
            ),
            (
                CFString::wrap_under_get_rule(kCGImageSourceCreateThumbnailWithTransform),
                CFBoolean::true_value().as_CFType(),
            ),
            (
                CFString::wrap_under_get_rule(kCGImageSourceThumbnailMaxPixelSize),
                CFNumber::from(super::MAX_TEXTURE_SIDE as i32).as_CFType(),
            ),
        ]);
        let raw = CGImageSourceCreateThumbnailAtIndex(
            source.as_CFTypeRef(),
            0,
            options.as_concrete_TypeRef(),
        );
        if raw.is_null() {
            return Err("图片缩略解码失败".into());
        }
        let image = CGImage::from_ptr(raw);
        let (width, height) = (image.width() as u32, image.height() as u32);
        super::check_dimensions(width, height)?;
        if width.max(height) > super::MAX_TEXTURE_SIDE {
            return Err("图片缩略图超限".into());
        }
        let mut pixels = RgbaImage::new(width, height);
        let space =
            CGColorSpace::create_with_name(kCGColorSpaceSRGB).ok_or("无法创建图片色彩空间")?;
        let context = CGContext::create_bitmap_context(
            Some(pixels.as_mut_ptr().cast()),
            width as usize,
            height as usize,
            8,
            width as usize * 4,
            &space,
            CGImageAlphaInfo::CGImageAlphaPremultipliedFirst as u32
                | CGImageByteOrderInfo::CGImageByteOrder32Little as u32,
        );
        context.draw_image(
            CGRect::new(
                &CGPoint::new(0.0, 0.0),
                &CGSize::new(width as f64, height as f64),
            ),
            &image,
        );
        drop(context);
        Ok(Some((pixels, natural)))
    }
}
