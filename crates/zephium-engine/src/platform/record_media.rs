//! Closed native record-image wire and bounded metadata-free raster custody.
use super::semantic_pixel_privacy::PixelPrivacyProof;
use base64::Engine as _;
use image::{ImageEncoder, ImageFormat, ImageReader};
use std::io::{Cursor, Write};
use zephium_agentic::{
    SemanticRecordMediaPlan, WorkBrowserRecordImage, WorkBrowserRecordImageOrigin,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Crop {
    pub(crate) rect: [f64; 4],
    pub(crate) proof: PixelPrivacyProof,
}
pub(crate) enum NativeMediaResult {
    Unavailable,
    Fetched(Vec<u8>),
    Crop(Crop),
}
pub(crate) type MediaCompletion = Box<dyn FnOnce(Option<NativeMediaResult>)>;
pub(crate) struct MediaInvocation {
    wire: String,
}
impl MediaInvocation {
    pub(crate) fn new(plan: &SemanticRecordMediaPlan, validate: bool) -> Self {
        let mut wire = plan.native_invocation();
        if validate {
            wire.replace_range(..1, "V");
        }
        Self { wire }
    }
    pub(crate) fn as_str(&self) -> &str {
        &self.wire
    }
    pub(crate) fn decode(&self, bytes: &[u8]) -> Option<NativeMediaResult> {
        if bytes.len() > 90 * 1024 {
            return None;
        }
        let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
        let object = value.as_object()?;
        if object.get("v")?.as_u64() != Some(1) {
            return None;
        }
        let status = object.get("s")?.as_str()?;
        if status == "unavailable" {
            return (object.len() == 2).then_some(NativeMediaResult::Unavailable);
        }
        let expected: serde_json::Value = serde_json::from_str(self.wire.get(3..)?).ok()?;
        for key in ["i", "g", "k", "n"] {
            if object.get(key) != expected.get(key) {
                return None;
            }
        }
        match status {
            "fetch" if object.len() == 7 && self.wire.starts_with("M1:") => {
                let bytes = object.get("bytes")?.as_str()?;
                if bytes.len() > 87384 {
                    return None;
                }
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(bytes)
                    .ok()?;
                (!bytes.is_empty() && bytes.len() <= 65536)
                    .then_some(NativeMediaResult::Fetched(bytes))
            }
            "crop" if object.len() == 8 => {
                let proof = PixelPrivacyProof::decode(&serde_json::to_vec(object.get("p")?).ok()?)?;
                if !proof.clear() {
                    return Some(NativeMediaResult::Unavailable);
                }
                let rect = object.get("b")?.as_object()?;
                if rect.len() != 4 {
                    return None;
                }
                let rect = [
                    rect.get("x")?.as_f64()?,
                    rect.get("y")?.as_f64()?,
                    rect.get("w")?.as_f64()?,
                    rect.get("h")?.as_f64()?,
                ];
                let (w, h) = proof.viewport();
                if !rect.iter().all(|v| v.is_finite())
                    || rect[0] < 0.0
                    || rect[1] < 0.0
                    || rect[2] < 64.0
                    || rect[3] < 48.0
                    || rect[0] + rect[2] > w
                    || rect[1] + rect[3] > h
                {
                    return None;
                }
                Some(NativeMediaResult::Crop(Crop { rect, proof }))
            }
            _ => None,
        }
    }
}
struct Sink(Vec<u8>);
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > 65536 {
            return Err(std::io::Error::other("media limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(crate) fn raster(
    plan: &SemanticRecordMediaPlan,
    bytes: Vec<u8>,
    geometry: Option<zephium_agentic::WorkBrowserCropGeometry>,
) -> Option<WorkBrowserRecordImage> {
    if bytes.len()
        > if geometry.is_some() {
            1024 * 1024
        } else {
            65536
        }
    {
        return None;
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    if !matches!(
        reader.format(),
        Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP | ImageFormat::Gif)
    ) {
        return None;
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    limits.max_alloc = Some(24 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode().ok()?;
    for (width, height) in [(320, 240), (240, 180), (160, 120), (80, 60)] {
        let small = record_thumbnail(&decoded, width, height).to_rgba8();
        let mut sink = Sink(Vec::new());
        if image::codecs::png::PngEncoder::new(&mut sink)
            .write_image(
                small.as_raw(),
                small.width(),
                small.height(),
                image::ExtendedColorType::Rgba8,
            )
            .is_ok()
        {
            return WorkBrowserRecordImage::from_native(
                plan,
                if geometry.is_some() {
                    WorkBrowserRecordImageOrigin::NativeCrop
                } else {
                    WorkBrowserRecordImageOrigin::NativeFetch
                },
                small.width().try_into().ok()?,
                small.height().try_into().ok()?,
                sink.0,
            )
            .map(|image| match geometry {
                Some(geometry) => image.with_crop_geometry(geometry),
                None => image,
            });
        }
    }
    None
}

pub(crate) struct RecordFrame {
    pub(crate) png: Vec<u8>,
    pub(crate) geometry: zephium_agentic::WorkBrowserCropGeometry,
}
pub(crate) type FrameCompletion = Box<dyn FnOnce(Option<RecordFrame>)>;
pub(crate) type FrameCapture = std::rc::Rc<dyn Fn(Crop, FrameCompletion) -> bool>;

pub(crate) type MediaDispatch =
    std::rc::Rc<dyn Fn(&SemanticRecordMediaPlan, bool, MediaCompletion)>;

pub(crate) type PrivacyDispatch =
    std::rc::Rc<dyn Fn(super::semantic_pixel_privacy::PrivacyCompletion)>;

fn record_thumbnail(image: &image::DynamicImage, width: u32, height: u32) -> image::DynamicImage {
    image.thumbnail(width.min(image.width()), height.min(image.height()))
}
#[cfg(test)]
mod size_tests {
    use super::*;
    #[test]
    fn native_record_images_never_upscale_a_small_crop() {
        let small = image::DynamicImage::new_rgba8(64, 47);
        let preview = record_thumbnail(&small, 320, 240);
        assert_eq!((preview.width(), preview.height()), (64, 47));
        let wide = image::DynamicImage::new_rgba8(800, 400);
        let preview = record_thumbnail(&wide, 320, 240);
        assert_eq!((preview.width(), preview.height()), (320, 160));
    }
}
