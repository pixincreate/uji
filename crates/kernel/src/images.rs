use std::io::Cursor;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, RgbaImage};
use mlua::BString;
use uji_macros::{function, value};

use crate::io::{self, Blocked};

const QUALITIES: [u8; 3] = [85, 70, 55];
const SMALLEST: u32 = 64;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ImageError {
    #[error("{0}")]
    Image(#[from] image::ImageError),
    #[error("{0}")]
    Read(#[from] std::io::Error),
    #[error("{0}")]
    Clipboard(#[from] arboard::Error),
    #[error("the image is not a png, jpeg, gif or webp")]
    Unsupported,
    #[error("the image is too large")]
    Oversized,
    #[error("the clipboard has no image")]
    Empty,
}

#[value]
#[derive(Clone, Copy)]
enum Media {
    #[serde(rename = "image/png")]
    Png,
    #[serde(rename = "image/jpeg")]
    Jpeg,
    #[serde(rename = "image/gif")]
    Gif,
    #[serde(rename = "image/webp")]
    Webp,
}

impl Media {
    fn of(format: ImageFormat) -> Option<Self> {
        match format {
            ImageFormat::Png => Some(Self::Png),
            ImageFormat::Jpeg => Some(Self::Jpeg),
            ImageFormat::Gif => Some(Self::Gif),
            ImageFormat::WebP => Some(Self::Webp),
            _ => None,
        }
    }
}

#[value]
pub(crate) struct Fitted {
    data: String,
    media_type: Media,
    width: u32,
    height: u32,
}

impl Fitted {
    fn new(bytes: &[u8], media_type: Media, width: u32, height: u32) -> Self {
        Self {
            data: STANDARD.encode(bytes),
            media_type,
            width,
            height,
        }
    }
}

fn png(image: &DynamicImage) -> Result<Vec<u8>, ImageError> {
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, ImageFormat::Png)?;
    Ok(out.into_inner())
}

fn jpeg(image: &DynamicImage, quality: u8) -> Result<Vec<u8>, ImageError> {
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, quality).encode_image(&image.to_rgb8())?;
    Ok(out)
}

fn scaled(image: DynamicImage, edge: u32) -> DynamicImage {
    if image.width().max(image.height()) > edge {
        image.resize(edge, edge, FilterType::Triangle)
    } else {
        image
    }
}

fn encode_within(
    image: &DynamicImage,
    photo: bool,
    limit: usize,
) -> Result<Option<(Vec<u8>, Media)>, ImageError> {
    if !photo {
        let bytes = png(image)?;
        if bytes.len() <= limit {
            return Ok(Some((bytes, Media::Png)));
        }
    }
    for quality in QUALITIES {
        let bytes = jpeg(image, quality)?;
        if bytes.len() <= limit {
            return Ok(Some((bytes, Media::Jpeg)));
        }
    }
    Ok(None)
}

pub(crate) fn fit_image(
    image: DynamicImage,
    edge: u32,
    limit: usize,
    photo: bool,
) -> Result<Fitted, ImageError> {
    let mut image = scaled(image, edge);
    loop {
        if let Some((bytes, media)) = encode_within(&image, photo, limit)? {
            return Ok(Fitted::new(&bytes, media, image.width(), image.height()));
        }
        let (width, height) = (image.width() / 2, image.height() / 2);
        if width.max(height) < SMALLEST {
            return Err(ImageError::Oversized);
        }
        image = image.resize(width, height, FilterType::Triangle);
    }
}

pub(crate) fn rgba(
    width: usize,
    height: usize,
    pixels: Vec<u8>,
) -> Result<DynamicImage, ImageError> {
    let width = u32::try_from(width).map_err(|_| ImageError::Oversized)?;
    let height = u32::try_from(height).map_err(|_| ImageError::Oversized)?;
    let pixels = RgbaImage::from_raw(width, height, pixels).ok_or(ImageError::Oversized)?;
    Ok(DynamicImage::ImageRgba8(pixels))
}

struct Header {
    format: ImageFormat,
    media: Media,
    width: u32,
    height: u32,
    orientation: Orientation,
}

fn header(data: &[u8]) -> Result<Header, ImageError> {
    let reader = ImageReader::new(Cursor::new(data)).with_guessed_format()?;
    let format = reader.format().ok_or(ImageError::Unsupported)?;
    let media = Media::of(format).ok_or(ImageError::Unsupported)?;
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder.orientation()?;
    let (width, height) = decoder.dimensions();
    Ok(Header {
        format,
        media,
        width,
        height,
        orientation,
    })
}

fn fit_data(data: &[u8], edge: u32, limit: usize) -> Result<Fitted, ImageError> {
    let found = header(data)?;
    let upright = found.orientation == Orientation::NoTransforms;
    if upright && found.width.max(found.height) <= edge && data.len() <= limit {
        return Ok(Fitted::new(data, found.media, found.width, found.height));
    }
    let mut image = ImageReader::with_format(Cursor::new(data), found.format).decode()?;
    image.apply_orientation(found.orientation);
    fit_image(image, edge, limit, found.format == ImageFormat::Jpeg)
}

#[function(image)]
async fn fit(data: BString, edge: u32, limit: usize) -> Result<Fitted, Blocked<ImageError>> {
    io::blocking(move || fit_data(&data, edge, limit)).await
}
