use std::fmt;

/// Contextual AI task action types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiAction {
    ExplainCode,
    AnalyzeFile,
    InterpretAst,
    DetectObfuscation,
    SuggestReconstruction,
    CustomQuery,
}

/// Strongly-typed AI error hierarchy matching PROJECT.md and the E2E test suite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiError {
    EmptyApiKey,
    InvalidApiKey(String),
    NetworkError(String),
    RateLimitExceeded,
    ServiceUnavailable(String),
    MalformedResponse(String),
    ContextLengthExceeded(String),
    Other(String),
}

impl fmt::Display for AiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyApiKey => write!(f, "API Key no configurada"),
            Self::InvalidApiKey(msg) => write!(f, "Clave API inválida: {}", msg),
            Self::NetworkError(msg) => write!(f, "Error de red: {}", msg),
            Self::RateLimitExceeded => write!(f, "Límite de tasa excedido (HTTP 429)"),
            Self::ServiceUnavailable(msg) => write!(f, "Servicio no disponible: {}", msg),
            Self::MalformedResponse(msg) => write!(f, "Respuesta con formato erróneo: {}", msg),
            Self::ContextLengthExceeded(msg) => write!(f, "Límite de contexto excedido: {}", msg),
            Self::Other(msg) => write!(f, "Error de IA: {}", msg),
        }
    }
}

impl std::error::Error for AiError {}

/// Request parameters for an AI query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiQueryRequest {
    pub prompt: String,
    pub model: String,
    pub context_code: String,
    pub action: AiAction,
    pub api_key: String,
    pub filename: String,
}

impl AiQueryRequest {
    pub fn new(prompt: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            model: model.into(),
            context_code: String::new(),
            action: AiAction::CustomQuery,
            api_key: String::new(),
            filename: String::new(),
        }
    }

    /// Access the contextual code (compatible with code_context naming).
    pub fn code_context(&self) -> &str {
        &self.context_code
    }
}

impl Default for AiQueryRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            model: "gemini-3.1-flash-lite".to_string(),
            context_code: String::new(),
            action: AiAction::CustomQuery,
            api_key: String::new(),
            filename: String::new(),
        }
    }
}

/// Structured response from an AI provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiQueryResponse {
    pub reply_text: String,
    pub suggested_code: Option<String>,
    pub mode_badge: String,
    pub model_used: String,
    pub token_count: Option<usize>,
}

/// Pluggable AI provider trait for extensible LLM engine integration.
pub trait AiProvider: Send + Sync {
    fn name(&self) -> &str;
    fn supported_models(&self) -> &'static [&'static str] {
        &[]
    }
    fn query(&self, req: &AiQueryRequest) -> Result<AiQueryResponse, AiError>;
    fn max_context_tokens(&self, _model: &str) -> usize {
        128_000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_error_display() {
        assert_eq!(AiError::EmptyApiKey.to_string(), "API Key no configurada");
        assert_eq!(
            AiError::InvalidApiKey("bad_key".into()).to_string(),
            "Clave API inválida: bad_key"
        );
        assert_eq!(
            AiError::RateLimitExceeded.to_string(),
            "Límite de tasa excedido (HTTP 429)"
        );
    }

    #[test]
    fn test_ai_query_request_default() {
        let req = AiQueryRequest::default();
        assert_eq!(req.action, AiAction::CustomQuery);
        assert_eq!(req.model, "gemini-3.1-flash-lite");
        assert!(req.context_code.is_empty());
        assert_eq!(req.code_context(), "");
    }
}
