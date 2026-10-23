/** Static image decoding validates dimensions before retaining page assets. */
use crate::{
    constants::{IMAGE_DECODED_BYTES, IMAGE_DIMENSION},
    protocol::{EngineError, EngineResult},
};
use std::io::Cursor;

/// Decoded metadata enforces a separate memory budget from transferred bytes.
pub(super) struct DecodedImage {
    pub width: u32,
    pub height: u32,
}

/** A committed image keeps its original encoded bytes for AppKit's native decoder. */
#[derive(Clone)]
pub struct ImageAsset {
    pub mime: String,
    pub bytes: std::sync::Arc<Vec<u8>>,
}

/** PNG and JPEG headers are decoded by audited Rust libraries with explicit bounds. */
pub(super) fn dimensions(mime: &str, bytes: &[u8]) -> EngineResult<DecodedImage> {
    let image = match mime {
        "image/png" => png(bytes)?,
        "image/jpeg" => jpeg(bytes)?,
        _ => return Err(invalid()),
    };
    if image.width == 0
        || image.height == 0
        || image.width > IMAGE_DIMENSION
        || image.height > IMAGE_DIMENSION
        || u64::from(image.width) * u64::from(image.height) * 4 > IMAGE_DECODED_BYTES as u64
    {
        return Err(invalid());
    }
    Ok(image)
}

/** PNG decoder expands palette data only after its dimensions fit the page budget. */
fn png(bytes: &[u8]) -> EngineResult<DecodedImage> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|_| invalid())?;
    let info = reader.info();
    checked(info.width, info.height)?;
    if reader.output_buffer_size() > IMAGE_DECODED_BYTES {
        return Err(invalid());
    }
    let mut output = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut output).map_err(|_| invalid())?;
    checked(frame.width, frame.height)?;
    Ok(DecodedImage {
        width: frame.width,
        height: frame.height,
    })
}

/** JPEG decoding rejects unsupported CMYK output rather than mispainting its colors. */
fn jpeg(bytes: &[u8]) -> EngineResult<DecodedImage> {
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(bytes));
    decoder.read_info().map_err(|_| invalid())?;
    let header = decoder.info().ok_or_else(invalid)?;
    checked(u32::from(header.width), u32::from(header.height))?;
    decoder.decode().map_err(|_| invalid())?;
    let info = decoder.info().ok_or_else(invalid)?;
    if info.pixel_format == jpeg_decoder::PixelFormat::CMYK32 {
        return Err(invalid());
    }
    checked(u32::from(info.width), u32::from(info.height))
}

/** One shared check prevents multiplication overflow and oversized decoded surfaces. */
fn checked(width: u32, height: u32) -> EngineResult<DecodedImage> {
    if width == 0
        || height == 0
        || width > IMAGE_DIMENSION
        || height > IMAGE_DIMENSION
        || u64::from(width) * u64::from(height) * 4 > IMAGE_DECODED_BYTES as u64
    {
        return Err(invalid());
    }
    Ok(DecodedImage { width, height })
}

/** Image decode errors never expose bytes, URLs or decoder internals. */
fn invalid() -> EngineError {
    EngineError::unsupported("The image is invalid, unsupported, or exceeds browser limits.")
}

/** The private network service base64-encodes image bytes to preserve JSON framing. */
pub(super) fn decode_base64(input: &str) -> EngineResult<Vec<u8>> {
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let mut value = 0u32;
    let mut bits = 0;
    for byte in input.bytes() {
        if byte == b'=' {
            break;
        }
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err(invalid()),
        };
        value = (value << 6) | u32::from(digit);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((value >> bits) as u8);
        }
    }
    if output.len() > crate::constants::IMAGE_BYTES {
        return Err(invalid());
    }
    Ok(output)
}
