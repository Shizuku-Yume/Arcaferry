//! PNG export with CCv3/V2 metadata embedding.
//!
//! Provides functions to create character card PNGs with embedded metadata,
//! supporting both CCv3 (ccv3 chunk) and V2 (chara chunk) formats for
//! maximum compatibility.

use crate::ccv3::CharacterCardV3;
use crate::error::{ArcaferryError, Result};
use crate::png::{embed_card, inject_text_chunk};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use image::{Rgba, RgbaImage};
use std::io::Cursor;

/// Generate a 512x512 gray placeholder PNG.
///
/// Used when no avatar is provided for the character card.
pub fn generate_placeholder_png() -> Vec<u8> {
    let img = RgbaImage::from_pixel(512, 512, Rgba([128, 128, 128, 255]));
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .expect("Failed to encode placeholder PNG");
    buffer
}

/// Convert CCv3 card to V2 JSON format for chara chunk compatibility.
///
/// This creates a backward-compatible V2 representation that older tools
/// (like some SillyTavern versions) can read.
fn convert_to_v2_json(card: &CharacterCardV3) -> String {
    let v2 = serde_json::json!({
        "name": card.data.name,
        "description": card.data.description,
        "personality": card.data.personality,
        "scenario": card.data.scenario,
        "first_mes": card.data.first_mes,
        "mes_example": card.data.mes_example,
        "creator_notes": card.data.creator_notes,
        "system_prompt": card.data.system_prompt,
        "post_history_instructions": card.data.post_history_instructions,
        "alternate_greetings": card.data.alternate_greetings,
        "tags": card.data.tags,
        "creator": card.data.creator,
        "character_version": card.data.character_version,
        "character_book": card.data.character_book,
    });
    serde_json::to_string(&v2).unwrap_or_default()
}

/// Create a PNG with embedded CCv3 and V2 metadata.
///
/// # Arguments
/// * `card` - The character card to embed
/// * `avatar_base64` - Optional avatar image as base64 string
///
/// # Returns
/// PNG bytes with embedded `ccv3` and `chara` tEXt chunks.
///
/// # IDAT Preservation
/// This function NEVER modifies IDAT chunks. All operations are purely
/// on text metadata chunks, preserving the original image data exactly.
pub fn create_card_png(card: &CharacterCardV3, avatar_base64: Option<&str>) -> Result<Vec<u8>> {
    // Get or create base PNG
    let base_png = if let Some(b64) = avatar_base64 {
        BASE64
            .decode(b64)
            .map_err(|e| ArcaferryError::InvalidJson(format!("Invalid avatar base64: {}", e)))?
    } else {
        generate_placeholder_png()
    };

    // Serialize card to JSON
    let card_json = serde_json::to_string(card)
        .map_err(|e| ArcaferryError::InvalidJson(format!("Failed to serialize card: {}", e)))?;

    // Embed ccv3 chunk (primary format)
    let mut result = embed_card(&base_png, &card_json)?;

    // Always add V2 compatible chara chunk for backward compatibility
    let v2_json = convert_to_v2_json(card);
    result = inject_text_chunk(&result, "chara", &v2_json, true)?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::png::{extract_idat_chunks, get_card_data, read_text_chunks};

    #[test]
    fn test_generate_placeholder_png() {
        let png = generate_placeholder_png();
        assert!(png.len() > 100);
        // Verify PNG signature
        assert_eq!(
            &png[0..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );
    }

    #[test]
    fn test_create_card_png_with_placeholder() {
        let card = CharacterCardV3::new("Test".to_string());
        let png = create_card_png(&card, None).unwrap();

        // Verify ccv3 chunk exists
        let card_data = get_card_data(&png).unwrap();
        assert!(card_data.is_some());
        let (format, _) = card_data.unwrap();
        assert_eq!(format, "ccv3");
    }

    #[test]
    fn test_create_card_png_has_v2_chunk() {
        let card = CharacterCardV3::new("TestV2".to_string());
        let png = create_card_png(&card, None).unwrap();

        // Verify chara chunk exists
        let text_chunks = read_text_chunks(&png).unwrap();
        assert!(
            text_chunks.contains_key("chara"),
            "Should have chara chunk for V2 compatibility"
        );
        assert!(text_chunks.contains_key("ccv3"), "Should have ccv3 chunk");
    }

    #[test]
    fn test_create_card_png_preserves_idat() {
        let placeholder = generate_placeholder_png();
        let original_idat = extract_idat_chunks(&placeholder).unwrap();

        let card = CharacterCardV3::new("Test".to_string());
        let b64 = BASE64.encode(&placeholder);
        let result = create_card_png(&card, Some(&b64)).unwrap();

        let result_idat = extract_idat_chunks(&result).unwrap();
        assert_eq!(
            original_idat, result_idat,
            "IDAT chunks must be preserved exactly"
        );
    }

    #[test]
    fn test_convert_to_v2_json() {
        let mut card = CharacterCardV3::new("TestChar".to_string());
        card.data.description = "A test character".to_string();
        card.data.personality = "Friendly".to_string();

        let v2_json = convert_to_v2_json(&card);
        let parsed: serde_json::Value = serde_json::from_str(&v2_json).unwrap();

        assert_eq!(parsed["name"], "TestChar");
        assert_eq!(parsed["description"], "A test character");
        assert_eq!(parsed["personality"], "Friendly");
    }
}
