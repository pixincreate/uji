use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, RgbaImage};
use mlua::{BString, Lua};
use uji_macros::function;

use crate::io;

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

pub(crate) type Fitted = (BString, &'static str, u32, u32);

fn media_type(format: ImageFormat) -> Option<&'static str> {
    match format {
        ImageFormat::Png => Some("image/png"),
        ImageFormat::Jpeg => Some("image/jpeg"),
        ImageFormat::Gif => Some("image/gif"),
        ImageFormat::WebP => Some("image/webp"),
        _ => None,
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
) -> Result<Option<(Vec<u8>, &'static str)>, ImageError> {
    if !photo {
        let bytes = png(image)?;
        if bytes.len() <= limit {
            return Ok(Some((bytes, "image/png")));
        }
    }
    for quality in QUALITIES {
        let bytes = jpeg(image, quality)?;
        if bytes.len() <= limit {
            return Ok(Some((bytes, "image/jpeg")));
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
            return Ok((BString::from(bytes), media, image.width(), image.height()));
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
    media: &'static str,
    width: u32,
    height: u32,
    orientation: Orientation,
}

fn header(data: &[u8]) -> Result<Header, ImageError> {
    let reader = ImageReader::new(Cursor::new(data)).with_guessed_format()?;
    let format = reader.format().ok_or(ImageError::Unsupported)?;
    let media = media_type(format).ok_or(ImageError::Unsupported)?;
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

fn fit_data(data: BString, edge: u32, limit: usize) -> Result<Fitted, ImageError> {
    let found = header(&data)?;
    let upright = found.orientation == Orientation::NoTransforms;
    if upright && found.width.max(found.height) <= edge && data.len() <= limit {
        return Ok((data, found.media, found.width, found.height));
    }
    let mut image =
        ImageReader::with_format(Cursor::new(data.as_slice()), found.format).decode()?;
    image.apply_orientation(found.orientation);
    fit_image(image, edge, limit, found.format == ImageFormat::Jpeg)
}

#[function(image)]
async fn fit(
    lua: Lua,
    data: BString,
    edge: u32,
    limit: usize,
) -> mlua::Result<Result<Fitted, ImageError>> {
    io::blocking(&lua, move || fit_data(data, edge, limit)).await
}
