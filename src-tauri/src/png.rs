use crate::error::{ArcaferryError, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use flate2::read::ZlibDecoder;
use std::collections::HashMap;
use std::io::Read;

const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// Represents a PNG chunk with type and data.
#[derive(Debug, Clone)]
pub struct PngChunk {
    pub chunk_type: [u8; 4],
    pub data: Vec<u8>,
}

impl PngChunk {
    /// Returns the chunk type as a string.
    pub fn type_string(&self) -> String {
        String::from_utf8_lossy(&self.chunk_type).to_string()
    }

    /// Creates a new PngChunk with the given type and data.
    pub fn new(chunk_type: &[u8; 4], data: Vec<u8>) -> Self {
        Self {
            chunk_type: *chunk_type,
            data,
        }
    }
}

/// Calculate CRC32 for a chunk (type + data).
fn calculate_crc(chunk_type: &[u8], data: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(chunk_type);
    hasher.update(data);
    hasher.finalize()
}

/// Read all chunks from PNG data.
///
/// Stops parsing at IEND chunk, ignoring any garbage data after it.
pub fn read_chunks(data: &[u8]) -> Result<Vec<PngChunk>> {
    if data.len() < 8 || data[..8] != PNG_SIGNATURE {
        return Err(ArcaferryError::InvalidPngSignature);
    }

    let mut chunks = Vec::new();
    let mut pos = 8;

    while pos < data.len() {
        if pos + 8 > data.len() {
            break;
        }

        let length =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        let chunk_type: [u8; 4] = [data[pos], data[pos + 1], data[pos + 2], data[pos + 3]];
        pos += 4;

        if pos + length + 4 > data.len() {
            return Err(ArcaferryError::PngChunkError("Truncated chunk".to_string()));
        }

        let chunk_data = data[pos..pos + length].to_vec();
        pos += length;

        // Skip CRC (we don't validate on read, but we calculate on write)
        pos += 4;

        let type_str = String::from_utf8_lossy(&chunk_type);
        chunks.push(PngChunk {
            chunk_type,
            data: chunk_data,
        });

        // Stop at IEND chunk
        if type_str == "IEND" {
            break;
        }
    }

    Ok(chunks)
}

/// Build PNG from chunks.
///
/// Writes PNG signature followed by all chunks with proper CRC.
pub fn build_png(chunks: &[PngChunk]) -> Vec<u8> {
    let mut result = Vec::new();
    result.extend_from_slice(&PNG_SIGNATURE);

    for chunk in chunks {
        let length = chunk.data.len() as u32;
        result.extend_from_slice(&length.to_be_bytes());
        result.extend_from_slice(&chunk.chunk_type);
        result.extend_from_slice(&chunk.data);

        let crc = calculate_crc(&chunk.chunk_type, &chunk.data);
        result.extend_from_slice(&crc.to_be_bytes());
    }

    result
}

/// Decode a tEXt chunk.
///
/// Format: keyword\0text
fn decode_text_chunk(chunk_data: &[u8]) -> Option<(String, String)> {
    let null_pos = chunk_data.iter().position(|&b| b == 0)?;

    let keyword = String::from_utf8_lossy(&chunk_data[..null_pos]).to_string();
    let text_data = &chunk_data[null_pos + 1..];

    // Try base64 decode first, fall back to raw text
    let text = if let Ok(decoded) = BASE64.decode(text_data) {
        String::from_utf8_lossy(&decoded).to_string()
    } else {
        String::from_utf8_lossy(text_data).to_string()
    };

    Some((keyword, text))
}

/// Decode an iTXt (international text) chunk.
///
/// Format: keyword\0compression_flag\0compression_method\0language_tag\0translated_keyword\0text
fn decode_itxt_chunk(chunk_data: &[u8]) -> Option<(String, String)> {
    let null_pos = chunk_data.iter().position(|&b| b == 0)?;

    let keyword = String::from_utf8_lossy(&chunk_data[..null_pos]).to_string();
    let rest = &chunk_data[null_pos + 1..];

    if rest.len() < 2 {
        return None;
    }

    let compression_flag = rest[0];
    // Skip compression_method (rest[1])
    let rest = &rest[2..];

    // Skip language tag
    let lang_null = rest.iter().position(|&b| b == 0)?;
    let rest = &rest[lang_null + 1..];

    // Skip translated keyword
    let trans_null = rest.iter().position(|&b| b == 0)?;
    let text_data = &rest[trans_null + 1..];

    let text_data = if compression_flag == 1 {
        // Decompress with zlib
        let mut decoder = ZlibDecoder::new(text_data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).ok()?;
        decompressed
    } else {
        text_data.to_vec()
    };

    let text = String::from_utf8_lossy(&text_data).to_string();
    Some((keyword, text))
}

/// Decode a zTXt (compressed text) chunk.
///
/// Format: keyword\0compression_method\0compressed_text
fn decode_ztxt_chunk(chunk_data: &[u8]) -> Option<(String, String)> {
    let null_pos = chunk_data.iter().position(|&b| b == 0)?;

    let keyword = String::from_utf8_lossy(&chunk_data[..null_pos]).to_string();

    if null_pos + 1 >= chunk_data.len() {
        return None;
    }

    // Skip compression_method byte (null_pos + 1), compressed data starts at null_pos + 2
    let compressed_data = &chunk_data[null_pos + 2..];

    // Decompress with zlib
    let mut decoder = ZlibDecoder::new(compressed_data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).ok()?;

    let text = String::from_utf8_lossy(&decompressed).to_string();
    Some((keyword, text))
}

/// Read all text chunks (tEXt, iTXt, zTXt) from a PNG file.
///
/// Returns a HashMap mapping keywords to text content.
pub fn read_text_chunks(data: &[u8]) -> Result<HashMap<String, String>> {
    let chunks = read_chunks(data)?;
    let mut result = HashMap::new();

    for chunk in chunks {
        let type_str = chunk.type_string();
        let decoded = match type_str.as_str() {
            "tEXt" => decode_text_chunk(&chunk.data),
            "iTXt" => decode_itxt_chunk(&chunk.data),
            "zTXt" => decode_ztxt_chunk(&chunk.data),
            _ => None,
        };

        if let Some((keyword, text)) = decoded {
            result.insert(keyword, text);
        }
    }

    Ok(result)
}

/// Extract character card data from PNG, preferring ccv3 over chara.
///
/// Returns (format_type, json_string) where format_type is "ccv3" or "chara",
/// or None if no card data found.
pub fn get_card_data(data: &[u8]) -> Result<Option<(String, String)>> {
    let text_chunks = read_text_chunks(data)?;

    // Priority: ccv3 > chara
    if let Some(json) = text_chunks.get("ccv3") {
        return Ok(Some(("ccv3".to_string(), json.clone())));
    }

    if let Some(json) = text_chunks.get("chara") {
        return Ok(Some(("chara".to_string(), json.clone())));
    }

    Ok(None)
}

/// Build tEXt chunk data.
///
/// Uses Base64 encoding for the text content (no newlines).
fn build_text_chunk_data(keyword: &str, text: &str) -> Vec<u8> {
    let encoded = BASE64.encode(text.as_bytes());
    let mut data = keyword.as_bytes().to_vec();
    data.push(0); // null separator
    data.extend_from_slice(encoded.as_bytes());
    data
}

/// Inject or replace a tEXt chunk in a PNG file.
///
/// 🚨 IDAT PROTECTION: This function only modifies text chunks.
/// All other chunks (IHDR, IDAT, IEND, etc.) are preserved exactly.
///
/// If `replace` is true and a chunk with the same keyword exists, it will be replaced.
/// Otherwise, the new chunk is inserted before IEND.
pub fn inject_text_chunk(data: &[u8], keyword: &str, text: &str, replace: bool) -> Result<Vec<u8>> {
    let chunks = read_chunks(data)?;
    let new_chunk_data = build_text_chunk_data(keyword, text);
    let new_chunk = PngChunk::new(b"tEXt", new_chunk_data);

    let mut new_chunks: Vec<PngChunk> = Vec::new();
    let mut replaced = false;

    for chunk in &chunks {
        let type_str = chunk.type_string();

        // Check if this is a text chunk we should replace
        if replace && type_str == "tEXt" {
            if let Some((kw, _)) = decode_text_chunk(&chunk.data) {
                if kw == keyword {
                    new_chunks.push(new_chunk.clone());
                    replaced = true;
                    continue;
                }
            }
        }

        new_chunks.push(chunk.clone());
    }

    // If not replaced, insert before IEND
    if !replaced {
        let iend_index = new_chunks.iter().position(|c| c.type_string() == "IEND");

        if let Some(idx) = iend_index {
            new_chunks.insert(idx, new_chunk);
        } else {
            new_chunks.push(new_chunk);
        }
    }

    Ok(build_png(&new_chunks))
}

/// Remove a text chunk with the specified keyword.
///
/// Removes tEXt, iTXt, or zTXt chunks matching the keyword.
pub fn remove_text_chunk(data: &[u8], keyword: &str) -> Result<Vec<u8>> {
    let chunks = read_chunks(data)?;
    let mut new_chunks: Vec<PngChunk> = Vec::new();

    for chunk in &chunks {
        let type_str = chunk.type_string();

        let should_remove = match type_str.as_str() {
            "tEXt" => decode_text_chunk(&chunk.data)
                .map(|(kw, _)| kw == keyword)
                .unwrap_or(false),
            "iTXt" => decode_itxt_chunk(&chunk.data)
                .map(|(kw, _)| kw == keyword)
                .unwrap_or(false),
            "zTXt" => decode_ztxt_chunk(&chunk.data)
                .map(|(kw, _)| kw == keyword)
                .unwrap_or(false),
            _ => false,
        };

        if !should_remove {
            new_chunks.push(chunk.clone());
        }
    }

    Ok(build_png(&new_chunks))
}

/// Embed CCv3 card data into PNG.
///
/// This is a convenience wrapper around inject_text_chunk that:
/// 1. Base64 encodes the JSON data
/// 2. Injects it as a "ccv3" tEXt chunk
/// 3. Replaces any existing ccv3 chunk
pub fn embed_card(png_data: &[u8], card_json: &str) -> Result<Vec<u8>> {
    // Note: inject_text_chunk already base64 encodes, so we pass raw JSON
    inject_text_chunk(png_data, "ccv3", card_json, true)
}

/// Extract all IDAT chunk data from a PNG file.
///
/// Used for verifying IDAT integrity before/after operations.
pub fn extract_idat_chunks(data: &[u8]) -> Result<Vec<Vec<u8>>> {
    let chunks = read_chunks(data)?;
    Ok(chunks
        .into_iter()
        .filter(|c| c.type_string() == "IDAT")
        .map(|c| c.data)
        .collect())
}

// Legacy function names for compatibility
pub fn extract_ccv3_data(data: &[u8]) -> Result<Option<String>> {
    match get_card_data(data)? {
        Some((_, json)) => Ok(Some(json)),
        None => Ok(None),
    }
}

pub fn embed_ccv3_data(png_data: &[u8], json_data: &str) -> Result<Vec<u8>> {
    embed_card(png_data, json_data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a minimal valid PNG for testing.
    fn create_minimal_png() -> Vec<u8> {
        let mut png = Vec::new();
        png.extend_from_slice(&PNG_SIGNATURE);

        // IHDR chunk (13 bytes of data)
        let ihdr_data: [u8; 13] = [
            0, 0, 0, 1, // width = 1
            0, 0, 0, 1, // height = 1
            8, // bit depth = 8
            0, // color type = grayscale
            0, // compression = deflate
            0, // filter = adaptive
            0, // interlace = none
        ];
        let ihdr_chunk = PngChunk::new(b"IHDR", ihdr_data.to_vec());

        // IDAT chunk (minimal compressed data for 1x1 grayscale)
        let idat_data: [u8; 10] = [0x08, 0xD7, 0x63, 0x60, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01];
        let idat_chunk = PngChunk::new(b"IDAT", idat_data.to_vec());

        // IEND chunk (empty)
        let iend_chunk = PngChunk::new(b"IEND", vec![]);

        // Write chunks
        for chunk in [&ihdr_chunk, &idat_chunk, &iend_chunk] {
            let length = chunk.data.len() as u32;
            png.extend_from_slice(&length.to_be_bytes());
            png.extend_from_slice(&chunk.chunk_type);
            png.extend_from_slice(&chunk.data);
            let crc = calculate_crc(&chunk.chunk_type, &chunk.data);
            png.extend_from_slice(&crc.to_be_bytes());
        }

        png
    }

    #[test]
    fn test_read_chunks() {
        let png = create_minimal_png();
        let chunks = read_chunks(&png).unwrap();

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].type_string(), "IHDR");
        assert_eq!(chunks[1].type_string(), "IDAT");
        assert_eq!(chunks[2].type_string(), "IEND");
    }

    #[test]
    fn test_build_png_roundtrip() {
        let original = create_minimal_png();
        let chunks = read_chunks(&original).unwrap();
        let rebuilt = build_png(&chunks);

        // Compare chunk count and types
        let rebuilt_chunks = read_chunks(&rebuilt).unwrap();
        assert_eq!(chunks.len(), rebuilt_chunks.len());

        for (orig, rebuilt) in chunks.iter().zip(rebuilt_chunks.iter()) {
            assert_eq!(orig.type_string(), rebuilt.type_string());
            assert_eq!(orig.data, rebuilt.data);
        }
    }

    #[test]
    fn test_inject_text_chunk() {
        let png = create_minimal_png();
        let json = r#"{"name":"Test"}"#;

        let modified = inject_text_chunk(&png, "ccv3", json, false).unwrap();
        let chunks = read_chunks(&modified).unwrap();

        // Should have 4 chunks now: IHDR, IDAT, tEXt, IEND
        assert_eq!(chunks.len(), 4);

        // Find the tEXt chunk
        let text_chunk = chunks.iter().find(|c| c.type_string() == "tEXt").unwrap();
        let (keyword, text) = decode_text_chunk(&text_chunk.data).unwrap();
        assert_eq!(keyword, "ccv3");
        assert_eq!(text, json);
    }

    #[test]
    fn test_inject_preserves_idat() {
        let png = create_minimal_png();
        let original_idat = extract_idat_chunks(&png).unwrap();

        let modified = inject_text_chunk(&png, "ccv3", r#"{"test":true}"#, false).unwrap();
        let modified_idat = extract_idat_chunks(&modified).unwrap();

        // IDAT chunks must be identical
        assert_eq!(original_idat, modified_idat);
    }

    #[test]
    fn test_get_card_data_priority() {
        let png = create_minimal_png();

        // Add both chara and ccv3 chunks
        let with_chara = inject_text_chunk(&png, "chara", r#"{"v2":"data"}"#, false).unwrap();
        let with_both = inject_text_chunk(&with_chara, "ccv3", r#"{"v3":"data"}"#, false).unwrap();

        // Should prefer ccv3
        let (format, data) = get_card_data(&with_both).unwrap().unwrap();
        assert_eq!(format, "ccv3");
        assert_eq!(data, r#"{"v3":"data"}"#);
    }

    #[test]
    fn test_replace_existing_chunk() {
        let png = create_minimal_png();

        let first = inject_text_chunk(&png, "ccv3", r#"{"version":1}"#, false).unwrap();
        let second = inject_text_chunk(&first, "ccv3", r#"{"version":2}"#, true).unwrap();

        let text_chunks = read_text_chunks(&second).unwrap();

        // Should only have one ccv3 chunk with the updated value
        assert_eq!(text_chunks.len(), 1);
        assert_eq!(text_chunks.get("ccv3").unwrap(), r#"{"version":2}"#);
    }

    #[test]
    fn test_invalid_png_signature() {
        let invalid = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
        let result = read_chunks(&invalid);
        assert!(matches!(result, Err(ArcaferryError::InvalidPngSignature)));
    }
}
