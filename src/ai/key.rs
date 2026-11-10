//! API Key Resolution Hierarchy and Key Masking for Safe UI Display.

use crate::ai::provider::AiError;

pub const DEFAULT_GEMINI_API_KEY: &str = "";

/// Resolves active Gemini API key using the 3-tier hierarchy:
/// 1. Explicit configured key (from AppConfig or SessionState)
/// 2. Environment variable `GEMINI_API_KEY`
/// 3. Built-in default fallback key (`DEFAULT_GEMINI_API_KEY`)
pub fn resolve_gemini_api_key(configured_key: Option<&str>) -> Result<String, AiError> {
    // Tier 1: Configured key
    if let Some(key) = configured_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    // Tier 2: Environment variable
    if let Ok(env_key) = std::env::var("GEMINI_API_KEY") {
        let trimmed = env_key.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    // Tier 3: Built-in default fallback
    let default_trimmed = DEFAULT_GEMINI_API_KEY.trim();
    if !default_trimmed.is_empty() {
        return Ok(default_trimmed.to_string());
    }

    Err(AiError::EmptyApiKey)
}

/// Masks an API key for safe UI and log display (e.g., "AQ.Ab8...sEqw").
pub fn mask_api_key(key: &str) -> String {
    let trimmed = key.trim();
    let char_count = trimmed.chars().count();
    if char_count <= 10 {
        return "********".to_string();
    }
    let prefix: String = trimmed.chars().take(6).collect();
    let suffix: String = trimmed.chars().skip(char_count.saturating_sub(4)).collect();
    format!("{}...{}", prefix, suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_key_tier1_configured() {
        let key = resolve_gemini_api_key(Some("custom_config_key_12345")).unwrap();
        assert_eq!(key, "custom_config_key_12345");
    }

    #[test]
    fn test_resolve_key_tier3_default() {
        let key = resolve_gemini_api_key(Some("   "));
        assert!(key.is_err());

        let key_none = resolve_gemini_api_key(None);
        assert!(key_none.is_err());
    }

    #[test]
    fn test_mask_api_key() {
        assert_eq!(mask_api_key(""), "********");
        assert_eq!(mask_api_key("12345"), "********");
        assert_eq!(
            mask_api_key("AIzaSyExampleValidGeminiKey12345"),
            "AIzaSy...2345"
        );
    }
}
