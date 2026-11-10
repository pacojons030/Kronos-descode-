//! Model Constants, Normalization, Deprecated Substitution, and Fallback Cascades.

pub const MODEL_GEMINI_3_1_FLASH_LITE: &str = "gemini-3.1-flash-lite";
pub const MODEL_GEMINI_3_8_FLASH: &str = "gemini-3.8-flash";
pub const MODEL_GEMINI_FLASH_LITE_LATEST: &str = "gemini-flash-lite-latest";

pub const DEFAULT_MODEL: &str = MODEL_GEMINI_3_1_FLASH_LITE;
pub const PRIMARY_FALLBACK_MODEL: &str = MODEL_GEMINI_3_1_FLASH_LITE;
pub const SECONDARY_FALLBACK_MODEL: &str = MODEL_GEMINI_FLASH_LITE_LATEST;

pub const SUPPORTED_MODELS: &[&str] = &[
    MODEL_GEMINI_3_1_FLASH_LITE,
    MODEL_GEMINI_3_8_FLASH,
    MODEL_GEMINI_FLASH_LITE_LATEST,
];

/// Normalizes requested model name and substitutes deprecated legacy models.
pub fn normalize_model_name(requested: &str) -> &'static str {
    let trimmed = requested.trim();
    if trimmed.is_empty() {
        return MODEL_GEMINI_3_1_FLASH_LITE;
    }

    // Intercept deprecated models (gemini-1.5-*, gemini-2.0-*, gemini-2.5-*)
    if trimmed.contains("1.5") || trimmed.contains("2.0") || trimmed.contains("2.5") {
        return MODEL_GEMINI_3_8_FLASH;
    }

    match trimmed {
        "gemini-3.8-flash" | "gemini-3.8" => MODEL_GEMINI_3_8_FLASH,
        "gemini-flash-lite-latest" | "flash-lite-latest" => MODEL_GEMINI_FLASH_LITE_LATEST,
        "gemini-3.1-flash-lite" | "gemini-3.1" => MODEL_GEMINI_3_1_FLASH_LITE,
        _ => MODEL_GEMINI_3_1_FLASH_LITE,
    }
}

/// Returns ordered fallback cascade list for a requested model.
pub fn get_fallback_chain(primary_model: &str) -> Vec<&'static str> {
    let normalized = normalize_model_name(primary_model);
    match normalized {
        MODEL_GEMINI_3_8_FLASH => vec![
            MODEL_GEMINI_3_8_FLASH,
            MODEL_GEMINI_3_1_FLASH_LITE,
            MODEL_GEMINI_FLASH_LITE_LATEST,
        ],
        MODEL_GEMINI_FLASH_LITE_LATEST => vec![
            MODEL_GEMINI_FLASH_LITE_LATEST,
            MODEL_GEMINI_3_1_FLASH_LITE,
        ],
        _ => vec![
            MODEL_GEMINI_3_1_FLASH_LITE,
            MODEL_GEMINI_FLASH_LITE_LATEST,
        ],
    }
}

/// Sanitizes a model string by stripping control characters and invalid chars.
pub fn sanitize_model_name(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_standard_models() {
        assert_eq!(normalize_model_name("gemini-3.1-flash-lite"), MODEL_GEMINI_3_1_FLASH_LITE);
        assert_eq!(normalize_model_name("gemini-3.8-flash"), MODEL_GEMINI_3_8_FLASH);
        assert_eq!(normalize_model_name("gemini-flash-lite-latest"), MODEL_GEMINI_FLASH_LITE_LATEST);
    }

    #[test]
    fn test_normalize_empty_and_unknown() {
        assert_eq!(normalize_model_name(""), MODEL_GEMINI_3_1_FLASH_LITE);
        assert_eq!(normalize_model_name("unknown-model-xyz"), MODEL_GEMINI_3_1_FLASH_LITE);
    }

    #[test]
    fn test_normalize_deprecated_substitution() {
        assert_eq!(normalize_model_name("gemini-1.5-pro"), MODEL_GEMINI_3_8_FLASH);
        assert_eq!(normalize_model_name("gemini-2.0-flash"), MODEL_GEMINI_3_8_FLASH);
        assert_eq!(normalize_model_name("gemini-2.5-flash-lite"), MODEL_GEMINI_3_8_FLASH);
    }

    #[test]
    fn test_fallback_chains() {
        let chain_38 = get_fallback_chain(MODEL_GEMINI_3_8_FLASH);
        assert_eq!(chain_38, vec![MODEL_GEMINI_3_8_FLASH, MODEL_GEMINI_3_1_FLASH_LITE, MODEL_GEMINI_FLASH_LITE_LATEST]);

        let chain_31 = get_fallback_chain(MODEL_GEMINI_3_1_FLASH_LITE);
        assert_eq!(chain_31, vec![MODEL_GEMINI_3_1_FLASH_LITE, MODEL_GEMINI_FLASH_LITE_LATEST]);
    }

    #[test]
    fn test_sanitize_model_name() {
        assert_eq!(sanitize_model_name("gemini-3.1\0\r\n"), "gemini-3.1");
        assert_eq!(sanitize_model_name("model; rm -rf /"), "modelrm-rf");
    }
}
