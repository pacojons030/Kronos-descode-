pub mod ai_chat;
pub mod decoders;
pub mod normalizer;
pub mod pipeline;
pub mod samples;
pub mod security;

pub use ai_chat::{
    build_semantic_clean_code, call_gemini_with_fallback, execute_gemini_http_request,
    extract_code_block, query_gemini_or_ast_assistant, AiResponse, DEFAULT_GEMINI_API_KEY,
    PRIMARY_FALLBACK_MODEL, SECONDARY_FALLBACK_MODEL,
};
pub use decoders::*;
pub use normalizer::normalize_obfuscated_identifiers;
pub use pipeline::{DeobfuscationConfig, DeobfuscationResult, DeobfuscatorEngine};
pub use samples::{CodeSample, BUILTIN_SAMPLES};
pub use security::{SecurityAudit, SecurityScanner};


