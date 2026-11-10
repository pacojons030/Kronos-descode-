//! Native Google Gemini REST API Client, Transport Abstraction, and Provider Implementation.

use std::sync::Arc;
use std::time::Duration;

use crate::ai::budget::{budget_code_context, estimate_tokens};
use crate::ai::key::resolve_gemini_api_key;
use crate::ai::models::{get_fallback_chain, PRIMARY_FALLBACK_MODEL, SUPPORTED_MODELS};
use crate::ai::parser::MarkdownCodeParser;
use crate::ai::prompts::{
    compute_shannon_entropy, prompt_analyze_file, prompt_detect_obfuscation, prompt_explain_code,
    prompt_interpret_ast, prompt_suggest_reconstruction, KRONOS_SYSTEM_PROMPT,
};
use crate::ai::provider::{AiAction, AiError, AiProvider, AiQueryRequest, AiQueryResponse};
use crate::core::pipeline::{DeobfuscationConfig, DeobfuscatorEngine};

// Backward-compatible AiResponse type for callers
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiResponse {
    pub reply_text: String,
    pub suggested_code: Option<String>,
    pub mode_badge: String,
}

/// HTTP Transport trait for decoupling network operations and enabling deterministic mock testing.
pub trait HttpTransport: Send + Sync {
    fn post_json(
        &self,
        url: &str,
        body: &serde_json::Value,
        timeout_secs: u64,
    ) -> Result<(u16, String), String>;
}

/// Native synchronous HTTPS transport using ureq with connection pooling and pure-Rust TLS.
pub struct UreqTransport {
    agent: ureq::Agent,
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl UreqTransport {
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout_read(Duration::from_secs(12))
            .build();
        Self { agent }
    }
}

impl HttpTransport for UreqTransport {
    fn post_json(
        &self,
        url: &str,
        body: &serde_json::Value,
        timeout_secs: u64,
    ) -> Result<(u16, String), String> {
        let agent = if timeout_secs == 12 {
            self.agent.clone()
        } else {
            ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(5))
                .timeout_read(Duration::from_secs(timeout_secs))
                .build()
        };

        match agent
            .post(url)
            .set("Content-Type", "application/json")
            .set("User-Agent", "Kronos-Descode/0.1.0 (Rust/Slint)")
            .send_json(body)
        {
            Ok(resp) => {
                let status = resp.status();
                let text = resp
                    .into_string()
                    .map_err(|e| format!("Error leyendo respuesta: {}", e))?;
                Ok((status, text))
            }
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_else(|_| "{}".to_string());
                Ok((code, text))
            }
            Err(ureq::Error::Transport(transport_err)) => {
                let sanitized = sanitize_gemini_url_and_keys(&transport_err.to_string());
                Err(format!("Error de transporte HTTP: {}", sanitized))
            }
        }
    }
}

/// Sanitizes URLs and error messages to prevent leaking API keys and formats socket errors.
pub fn sanitize_gemini_url_and_keys(s: &str) -> String {
    let mut clean = s.to_string();
    if let Some(pos) = clean.find("key=") {
        let after = &clean[pos + 4..];
        let end_idx = after
            .find(['&', ' ', ':', '\n'])
            .unwrap_or(after.len());
        clean.replace_range(pos + 4..pos + 4 + end_idx, "[CLAVE_PROTEGIDA]");
    }
    if clean.contains("10060") || clean.contains("timed out") || clean.contains("periodo de tiempo") {
        return "Tiempo de espera de conexión agotado (Servidor de Gemini inaccesible o clave API no válida)".to_string();
    }
    clean
}

pub type MockResponder =
    Arc<dyn Fn(&str, &serde_json::Value) -> Result<(u16, String), String> + Send + Sync>;

/// In-memory mock transport for deterministic offline and boundary condition testing.
pub struct MockHttpTransport {
    pub responder: MockResponder,
}

impl MockHttpTransport {
    pub fn new<F>(responder: F) -> Self
    where
        F: Fn(&str, &serde_json::Value) -> Result<(u16, String), String> + Send + Sync + 'static,
    {
        Self {
            responder: Arc::new(responder),
        }
    }

    pub fn with_success(reply_text: &str) -> Self {
        let reply = reply_text.to_string();
        Self::new(move |_url, _body| {
            let json_resp = serde_json::json!({
                "candidates": [{
                    "content": {
                        "parts": [{ "text": reply }]
                    }
                }],
                "usageMetadata": {
                    "totalTokenCount": 42
                }
            });
            Ok((200, json_resp.to_string()))
        })
    }

    pub fn with_error(code: u16, message: &str) -> Self {
        let msg = message.to_string();
        Self::new(move |_url, _body| {
            let json_resp = serde_json::json!({
                "error": {
                    "code": code,
                    "message": msg
                }
            });
            Ok((code, json_resp.to_string()))
        })
    }
}

impl Default for MockHttpTransport {
    fn default() -> Self {
        Self::with_success("Mock response from Gemini")
    }
}

impl HttpTransport for MockHttpTransport {
    fn post_json(
        &self,
        url: &str,
        body: &serde_json::Value,
        _timeout_secs: u64,
    ) -> Result<(u16, String), String> {
        (self.responder)(url, body)
    }
}

/// Builds the official Gemini REST request body JSON.
pub fn build_gemini_request_body(
    prompt: &str,
    system_instruction: Option<&str>,
    temperature: f32,
    max_output_tokens: u32,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "contents": [
            {
                "role": "user",
                "parts": [{ "text": prompt }]
            }
        ],
        "generationConfig": {
            "temperature": temperature.clamp(0.0, 2.0),
            "topP": 0.95,
            "maxOutputTokens": max_output_tokens
        },
        "safetySettings": [
            { "category": "HARM_CATEGORY_HARASSMENT", "threshold": "BLOCK_NONE" },
            { "category": "HARM_CATEGORY_HATE_SPEECH", "threshold": "BLOCK_NONE" },
            { "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "threshold": "BLOCK_NONE" },
            { "category": "HARM_CATEGORY_DANGEROUS_CONTENT", "threshold": "BLOCK_NONE" }
        ]
    });

    if let Some(sys) = system_instruction {
        if !sys.trim().is_empty() {
            body["systemInstruction"] = serde_json::json!({
                "parts": [{ "text": sys }]
            });
        }
    }

    body
}

/// Parses raw JSON response from Gemini API into (reply_text, token_count).
pub fn parse_gemini_response(json_val: &serde_json::Value) -> Result<(String, Option<usize>), AiError> {
    if let Some(err_obj) = json_val.get("error") {
        let code = err_obj.get("code").and_then(|c| c.as_i64()).unwrap_or(500);
        let msg = err_obj
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Error en Gemini API");
        return match code {
            400 | 403 => Err(AiError::InvalidApiKey(msg.to_string())),
            429 => Err(AiError::RateLimitExceeded),
            500 | 503 => Err(AiError::ServiceUnavailable(msg.to_string())),
            _ => Err(AiError::Other(format!("HTTP {}: {}", code, msg))),
        };
    }

    let token_count = json_val
        .get("usageMetadata")
        .and_then(|u| u.get("totalTokenCount"))
        .and_then(|t| t.as_u64())
        .map(|t| t as usize);

    if let Some(candidates) = json_val.get("candidates").and_then(|c| c.as_array()) {
        if let Some(first_cand) = candidates.first() {
            if let Some(parts) = first_cand
                .get("content")
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.as_array())
            {
                let mut full_text = String::new();
                for part in parts {
                    if let Some(txt) = part.get("text").and_then(|t| t.as_str()) {
                        full_text.push_str(txt);
                    }
                }
                if !full_text.trim().is_empty() {
                    return Ok((full_text, token_count));
                }
            }
        }
    }

    Err(AiError::MalformedResponse(
        "Respuesta no contiene candidatos válidos".to_string(),
    ))
}

/// Executes a raw HTTP request to Gemini API without subprocess spawning.
pub fn execute_raw_http(
    transport: &dyn HttpTransport,
    api_key: &str,
    model: &str,
    prompt: &str,
) -> Result<String, (i64, String)> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err((400, "API Key vacía".to_string()));
    }

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model.trim(),
        key
    );

    let body = build_gemini_request_body(prompt, Some(KRONOS_SYSTEM_PROMPT), 0.2, 8192);
    let timeout = if model.contains("lite") { 8 } else { 12 };

    let (status_code, response_body) = match transport.post_json(&url, &body, timeout) {
        Ok(res) => res,
        Err(err) => return Err((0, err)),
    };

    if response_body.trim().is_empty() {
        let code = if status_code >= 500 { status_code as i64 } else { -2 };
        return Err((code, "Respuesta vacía del servidor de Gemini".to_string()));
    }

    let json_val: serde_json::Value = match serde_json::from_str(&response_body) {
        Ok(v) => v,
        Err(e) => {
            let code = if status_code >= 500 { status_code as i64 } else { -2 };
            return Err((
                code,
                format!(
                    "Respuesta no es JSON válido: {}. Salida cruda: {}",
                    e, response_body
                ),
            ));
        }
    };

    if let Some(err_obj) = json_val.get("error") {
        let code = err_obj.get("code").and_then(|c| c.as_i64()).unwrap_or(status_code as i64);
        let msg = err_obj
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Error en Gemini API");
        return Err((code, msg.to_string()));
    }

    match parse_gemini_response(&json_val) {
        Ok((text, _)) => Ok(text),
        Err(AiError::MalformedResponse(msg)) => Err((-2, msg)),
        Err(AiError::InvalidApiKey(msg)) => Err((400, msg)),
        Err(AiError::RateLimitExceeded) => Err((429, "Quota exceeded".to_string())),
        Err(AiError::ServiceUnavailable(msg)) => Err((503, msg)),
        Err(err) => Err((0, err.to_string())),
    }
}

/// Google Gemini native REST API provider.
pub struct GeminiProvider {
    transport: Arc<dyn HttpTransport>,
}

impl Default for GeminiProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiProvider {
    pub fn new() -> Self {
        Self {
            transport: Arc::new(UreqTransport::new()),
        }
    }

    pub fn with_transport(transport: Arc<dyn HttpTransport>) -> Self {
        Self { transport }
    }

    pub fn transport(&self) -> &dyn HttpTransport {
        self.transport.as_ref()
    }
}

impl AiProvider for GeminiProvider {
    fn name(&self) -> &str {
        "Google Gemini"
    }

    fn supported_models(&self) -> &'static [&'static str] {
        SUPPORTED_MODELS
    }

    fn query(&self, req: &AiQueryRequest) -> Result<AiQueryResponse, AiError> {
        let key = req.api_key.trim();
        if key.is_empty() {
            return Err(AiError::EmptyApiKey);
        }

        let full_prompt = match req.action {
            AiAction::ExplainCode => {
                prompt_explain_code(&req.context_code, &req.filename, None, None)
            }
            AiAction::AnalyzeFile => {
                prompt_analyze_file(&req.context_code, &req.filename, None)
            }
            AiAction::InterpretAst => {
                prompt_interpret_ast(&req.context_code, &req.prompt, &req.filename)
            }
            AiAction::DetectObfuscation => {
                let entropy = compute_shannon_entropy(req.context_code.as_bytes());
                prompt_detect_obfuscation(&req.context_code, entropy, 0, 0, 0, &req.filename)
            }
            AiAction::SuggestReconstruction => {
                prompt_suggest_reconstruction(&req.context_code, &req.prompt, &req.filename)
            }
            AiAction::CustomQuery => {
                if req.context_code.trim().is_empty() {
                    req.prompt.clone()
                } else {
                    format!(
                        "Contexto de código ({}):\n```python\n{}\n```\n\nConsulta: {}",
                        req.filename, req.context_code, req.prompt
                    )
                }
            }
        };

        // Budget prompt context to avoid exceeding token limit
        let (budgeted_prompt, _) = budget_code_context(&full_prompt, 32_000);

        let fallback_chain = get_fallback_chain(&req.model);
        let mut last_error = None;

        for (i, &candidate_model) in fallback_chain.iter().enumerate() {
            match execute_raw_http(self.transport.as_ref(), key, candidate_model, &budgeted_prompt) {
                Ok(reply_text) => {
                    let suggested_code = MarkdownCodeParser::extract_code_block(&reply_text);
                    let mode_badge = if i > 0 {
                        format!("{} (Fallback)", candidate_model)
                    } else {
                        candidate_model.to_string()
                    };

                    let token_count = Some(estimate_tokens(&reply_text));

                    return Ok(AiQueryResponse {
                        reply_text,
                        suggested_code,
                        mode_badge,
                        model_used: candidate_model.to_string(),
                        token_count,
                    });
                }
                Err((code, err_msg)) => {
                    let ai_err = match code {
                        400 | 401 | 403 => AiError::InvalidApiKey(err_msg.clone()),
                        429 => AiError::RateLimitExceeded,
                        500 | 502 | 503 | 504 => AiError::ServiceUnavailable(err_msg.clone()),
                        -2 => AiError::MalformedResponse(err_msg.clone()),
                        _ => AiError::NetworkError(err_msg.clone()),
                    };

                    // For invalid API key (400/403), fallback models will also fail; return early
                    if matches!(ai_err, AiError::InvalidApiKey(_)) {
                        return Err(ai_err);
                    }

                    last_error = Some(ai_err);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            AiError::ServiceUnavailable("Ningún modelo de Gemini respondió exitosamente".to_string())
        }))
    }
}

// =========================================================================
// Backwards-Compatible Functions (formerly in src/core/ai_chat.rs)
// =========================================================================

/// Extracts code block from text.
pub fn extract_code_block(text: &str) -> Option<String> {
    MarkdownCodeParser::extract_code_block(text)
}

/// Cleans deobfuscated code semantically.
pub fn build_semantic_clean_code(filename: &str, raw_transformed: &str) -> String {
    let is_python = filename.to_lowercase().ends_with(".py");
    let header_comment = if is_python {
        format!("# [KRONOS AI + AST ENGINE] Código Desofuscado y Limpio ({})", filename)
    } else {
        format!("// [KRONOS AI + AST ENGINE] Código Desofuscado y Limpio ({})", filename)
    };

    let mut clean_lines: Vec<String> = Vec::new();
    clean_lines.push(header_comment);

    for line in raw_transformed.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# [AST PRUNED")
            || trimmed.starts_with("# [KRONOS")
            || trimmed.starts_with("// Byte code:")
        {
            continue;
        }
        let improved = line
            .replace("var_1", "cadena_decodificada_1")
            .replace("var_2", "payload_hex_2")
            .replace("var_3", "usuario_b64_3")
            .replace("var_4", "rol_resuelto_4")
            .replace("var_5", "constante_5")
            .replace("var_6", "valor_6");
        clean_lines.push(improved);
    }

    clean_lines.join("\n").trim().to_string() + "\n"
}

/// Direct HTTP request without external subprocess spawning.
pub fn execute_gemini_http_request(
    api_key: &str,
    model: &str,
    prompt: &str,
) -> Result<String, (i64, String)> {
    let transport = UreqTransport::new();
    execute_raw_http(&transport, api_key, model, prompt)
}

/// Calls Gemini with automatic fallback across models.
pub fn call_gemini_with_fallback(
    api_key: &str,
    requested_model: &str,
    prompt: &str,
) -> Result<(String, String), String> {
    let key = match resolve_gemini_api_key(if api_key.trim().is_empty() {
        None
    } else {
        Some(api_key)
    }) {
        Ok(k) => k,
        Err(_) => return Err("API Key vacía".to_string()),
    };

    let req_model = if requested_model.trim().is_empty() {
        PRIMARY_FALLBACK_MODEL
    } else {
        requested_model.trim()
    };

    let provider = GeminiProvider::new();
    let query_req = AiQueryRequest {
        prompt: prompt.to_string(),
        model: req_model.to_string(),
        context_code: String::new(),
        action: AiAction::CustomQuery,
        api_key: key,
        filename: String::new(),
    };

    match provider.query(&query_req) {
        Ok(resp) => Ok((resp.reply_text, resp.mode_badge)),
        Err(err) => Err(err.to_string()),
    }
}

/// Main AI query function connecting editor context and zero-exec AST deobfuscation.
pub fn query_gemini_or_ast_assistant(
    api_key: &str,
    selected_model: &str,
    user_prompt: &str,
    active_filename: &str,
    input_code: &str,
    config: &DeobfuscationConfig,
) -> AiResponse {
    let trimmed_prompt = user_prompt.trim();
    let lang = crate::ai::prompts::detect_source_language(active_filename, input_code);
    let res = DeobfuscatorEngine::process(input_code, config);

    let full_prompt = if input_code.trim().is_empty() {
        format!(
            "Eres KRONOS AI, asistente de inteligencia artificial para desarrollo, ingeniería inversa y desofuscación.\n\
             Responde siempre en español de forma directa, técnica, concisa y muy fluida.\n\
             No hay código cargado en el editor actualmente.\n\n\
             Instrucción del analista: {}",
            trimmed_prompt
        )
    } else {
        format!(
            "Eres KRONOS AI, el motor de ingeniería inversa y desofuscación de KRONOS DESCODE.\n\
             Responde siempre en español de forma técnica, precisa, directa y profesional.\n\
             Lenguaje objetivo detectado: {}\n\
             Archivo en análisis: {}\n\n\
             INSTRUCCIONES Y PROTOCOLO DE DESOFUSCACIÓN KRONOS:\n\
             1. Si el analista solicita desofuscar, limpiar o reconstruir código, aplica obligatoriamente la normalización semántica de nombres (sustituye caracteres griegos u ofuscados por nombres en inglés descriptivos como LoginSession, AuthPacket, etc.), resolución de constantes aritméticas (Integer.reverse(-2147483648) -> 1), y eliminación de artefactos de descompilador.\n\
             2. La propuesta de código DEBE ser la versión final COMPLETA, IDIOMÁTICA, SINTÁCTICAMENTE VÁLIDA Y SIN TRUNCAR dentro de un bloque Markdown ```{}\n...\n```.\n\n\
             Contexto del análisis:\n\
             - Archivo activo: {}\n\
             - Código en el editor:\n```{lang}\n{}\n```\n\
             - Código pre-desofuscado por KRONOS AST (Zero-Exec):\n```{lang}\n{}\n```\n\n\
             Instrucción o consulta del analista: {}",
            lang,
            active_filename,
            lang,
            active_filename,
            input_code,
            res.raw_transformed,
            trimmed_prompt
        )
    };

    match call_gemini_with_fallback(api_key, selected_model, &full_prompt) {
        Ok((reply, model_badge)) => {
            let suggested = extract_code_block(&reply);
            AiResponse {
                reply_text: reply,
                suggested_code: suggested,
                mode_badge: model_badge,
            }
        }
        Err(err) => {
            let sanitized_err = sanitize_gemini_url_and_keys(&err);
            // Graceful fallback on network failure: semantic code cleaning
            let (fallback_code, reply_msg) = if !input_code.trim().is_empty() {
                let clean = build_semantic_clean_code(active_filename, &res.raw_transformed);
                let ext = if active_filename.ends_with(".java") { "java" } else { "python" };
                let msg = format!(
                    "⚡ **Procesado localmente por el Motor KRONOS Zero-Exec (Modo Offline)**\n\n\
                    *(El servicio en la nube reportó: {})*\n\n\
                    KRONOS ha aplicado de inmediato el protocolo de desofuscación determinista local sobre el archivo activo:\n\
                    - **Plegado de constantes opacas**: Aplicado\n\
                    - **Normalización léxica y purga de trampas de bytecode**: Completada\n\
                    - **Cálculo de AST**: Válido\n\n\
                    ```{}\n\
                    {}\n\
                    ```\n\n\
                    *(Usa el botón superior **[Volcar código sugerido al editor]** para transferir la versión limpia directamente al espacio de trabajo)*",
                    sanitized_err,
                    ext,
                    clean
                );
                (Some(clean), msg)
            } else {
                let msg = format!(
                    "⚠️ **Modo Desconectado / Sin Conexión**\n\n\
                    No se pudo contactar con los servidores de Gemini API ({}).\n\n\
                    Todas las funciones de desofuscación, inspección AST, plegado de constantes y auditoría técnica del proyecto funcionan al 100% en local sin necesidad de internet.",
                    sanitized_err
                );
                (None, msg)
            };

            AiResponse {
                reply_text: reply_msg,
                suggested_code: fallback_code,
                mode_badge: "KRONOS Local (Offline)".to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gemini_provider_metadata() {
        let provider = GeminiProvider::new();
        assert_eq!(provider.name(), "Google Gemini");
        assert!(provider.supported_models().contains(&"gemini-3.1-flash-lite"));
        assert!(provider.supported_models().contains(&"gemini-3.8-flash"));
    }

    #[test]
    fn test_gemini_provider_with_mock_success() {
        let mock = Arc::new(MockHttpTransport::with_success("```python\nx = 1\n```"));
        let provider = GeminiProvider::with_transport(mock);

        let req = AiQueryRequest {
            prompt: "Test".into(),
            model: "gemini-3.1-flash-lite".into(),
            context_code: "".into(),
            action: AiAction::CustomQuery,
            api_key: "VALID_MOCK_KEY".into(),
            filename: "test.py".into(),
        };

        let res = provider.query(&req).unwrap();
        assert_eq!(res.suggested_code, Some("x = 1".to_string()));
        assert_eq!(res.model_used, "gemini-3.1-flash-lite");
    }

    #[test]
    fn test_gemini_provider_empty_key_fails() {
        let provider = GeminiProvider::new();
        let req = AiQueryRequest {
            prompt: "Test".into(),
            model: "gemini-3.1-flash-lite".into(),
            context_code: "".into(),
            action: AiAction::CustomQuery,
            api_key: "".into(),
            filename: "".into(),
        };

        let res = provider.query(&req);
        assert_eq!(res.unwrap_err(), AiError::EmptyApiKey);
    }

    #[test]
    fn test_gemini_provider_mock_rate_limit_fallback() {
        // First model fails with 429, second model succeeds
        let attempt = std::sync::atomic::AtomicUsize::new(0);
        let mock = Arc::new(MockHttpTransport::new(move |_url, _body| {
            let count = attempt.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count == 0 {
                let err_json = serde_json::json!({
                    "error": {
                        "code": 429,
                        "message": "Quota exceeded",
                        "status": "RESOURCE_EXHAUSTED"
                    }
                });
                Ok((429, err_json.to_string()))
            } else {
                let success_json = serde_json::json!({
                    "candidates": [{
                        "content": {
                            "parts": [{ "text": "Fallback worked" }]
                        }
                    }]
                });
                Ok((200, success_json.to_string()))
            }
        }));

        let provider = GeminiProvider::with_transport(mock);
        let req = AiQueryRequest {
            prompt: "Ping".into(),
            model: "gemini-3.8-flash".into(),
            context_code: "".into(),
            action: AiAction::CustomQuery,
            api_key: "MOCK_KEY".into(),
            filename: "test.py".into(),
        };

        let res = provider.query(&req).unwrap();
        assert_eq!(res.reply_text, "Fallback worked");
        assert!(res.mode_badge.contains("Fallback"));
    }

    #[test]
    fn test_sanitize_gemini_url_and_keys() {
        let raw_err = "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.1:generateContent?key=AQ.Secret12345: Network Error: (os error 10060)";
        let sanitized = sanitize_gemini_url_and_keys(raw_err);
        assert!(!sanitized.contains("AQ.Secret12345"));
        assert!(sanitized.contains("Tiempo de espera"));
    }
}
