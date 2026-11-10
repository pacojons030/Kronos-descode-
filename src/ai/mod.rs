pub mod budget;
pub mod gemini;
pub mod key;
pub mod models;
pub mod parser;
pub mod prompts;
pub mod provider;

pub use budget::{budget_code_context, estimate_tokens, ContextBudget};
pub use gemini::{
    build_semantic_clean_code, call_gemini_with_fallback, execute_gemini_http_request,
    extract_code_block, query_gemini_or_ast_assistant, AiResponse, GeminiProvider, HttpTransport,
    MockHttpTransport, UreqTransport,
};
pub use key::{mask_api_key, resolve_gemini_api_key, DEFAULT_GEMINI_API_KEY};
pub use models::{
    get_fallback_chain, normalize_model_name, sanitize_model_name, DEFAULT_MODEL,
    MODEL_GEMINI_3_1_FLASH_LITE, MODEL_GEMINI_3_8_FLASH, MODEL_GEMINI_FLASH_LITE_LATEST,
    PRIMARY_FALLBACK_MODEL, SECONDARY_FALLBACK_MODEL, SUPPORTED_MODELS,
};
pub use parser::{
    ConversationHistory, ConversationMessage, ExtractedCodeBlock, MarkdownCodeParser, MessageRole,
};
pub use prompts::{
    compute_shannon_entropy, prompt_analyze_file, prompt_detect_obfuscation, prompt_explain_code,
    prompt_interpret_ast, prompt_suggest_reconstruction, KRONOS_SYSTEM_PROMPT,
};
pub use provider::{AiAction, AiError, AiProvider, AiQueryRequest, AiQueryResponse};
