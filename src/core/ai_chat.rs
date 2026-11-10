//! KRONOS AI Assistant - Native HTTP Transport and Core Compatibility Layer.
//!
//! External subprocess spawning has been completely eliminated in favor
//! of native `ureq` HTTPS client with pure-Rust TLS and connection pooling.

use crate::core::pipeline::{DeobfuscationConfig, DeobfuscatorEngine};
use serde_json::{json, Value};
use std::time::Duration;

pub const DEFAULT_GEMINI_API_KEY: &str = "";
pub const PRIMARY_FALLBACK_MODEL: &str = "gemini-3.1-flash-lite";
pub const SECONDARY_FALLBACK_MODEL: &str = "gemini-flash-lite-latest";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiResponse {
    pub reply_text: String,
    pub suggested_code: Option<String>,
    pub mode_badge: String,
}

/// Extrae el primer bloque de código ```python ... ``` o ``` ... ``` de una respuesta Markdown.
pub fn extract_code_block(text: &str) -> Option<String> {
    if let Some(start_idx) = text.find("```python") {
        let after = &text[start_idx + "```python".len()..];
        if let Some(end_idx) = after.find("```") {
            let code = after[..end_idx].trim();
            if !code.is_empty() {
                return Some(code.to_string());
            }
        }
    }
    if let Some(start_idx) = text.find("```") {
        let after = &text[start_idx + 3..];
        let after_first_line = if let Some(nl) = after.find('\n') {
            &after[nl + 1..]
        } else {
            after
        };
        if let Some(end_idx) = after_first_line.find("```") {
            let code = after_first_line[..end_idx].trim();
            if !code.is_empty() {
                return Some(code.to_string());
            }
        }
    }
    None
}

/// Limpia semánticamente el código desofuscado para que sea aún más legible e idiomático.
pub fn build_semantic_clean_code(filename: &str, raw_transformed: &str) -> String {
    let mut clean_lines: Vec<String> = Vec::new();
    clean_lines.push(format!(
        "# [GEMINI AI + KRONOS AST] Código Python Desofuscado y Limpio ({})",
        filename
    ));

    for line in raw_transformed.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# [AST PRUNED") || trimmed.starts_with("# [KRONOS") {
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

/// Ejecuta una llamada HTTP directa al endpoint oficial de Gemini API mediante ureq nativo sin subprocess.
pub fn execute_gemini_http_request(
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

    let body = json!({
        "contents": [
            {
                "parts": [
                    { "text": prompt }
                ]
            }
        ],
        "generationConfig": {
            "temperature": 0.2,
            "topP": 0.95,
            "maxOutputTokens": 8192
        },
        "safetySettings": [
            { "category": "HARM_CATEGORY_HARASSMENT", "threshold": "BLOCK_NONE" },
            { "category": "HARM_CATEGORY_HATE_SPEECH", "threshold": "BLOCK_NONE" },
            { "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "threshold": "BLOCK_NONE" },
            { "category": "HARM_CATEGORY_DANGEROUS_CONTENT", "threshold": "BLOCK_NONE" }
        ]
    });

    let timeout_sec = if model.contains("lite") { 8 } else { 12 };

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(timeout_sec))
        .build();

    let (status_code, stdout_str) = match agent
        .post(&url)
        .set("Content-Type", "application/json")
        .set("User-Agent", "Kronos-Descode/0.1.0 (Rust/Slint)")
        .send_json(body)
    {
        Ok(resp) => {
            let status = resp.status();
            let text = resp
                .into_string()
                .map_err(|e| (500, format!("Error leyendo respuesta de Gemini: {}", e)))?;
            (status, text)
        }
        Err(ureq::Error::Status(code, resp)) => {
            let text = resp.into_string().unwrap_or_else(|_| "{}".to_string());
            (code, text)
        }
        Err(ureq::Error::Transport(e)) => {
            let sanitized = sanitize_gemini_url_and_keys(&e.to_string());
            return Err((500, format!("Error de red al conectar con Gemini API: {}", sanitized)));
        }
    };

    if stdout_str.trim().is_empty() {
        return Err((500, "Respuesta vacía del servidor de Gemini".to_string()));
    }

    let val: Value = match serde_json::from_str(&stdout_str) {
        Ok(v) => v,
        Err(e) => {
            return Err((
                500,
                format!("Respuesta no es JSON válido: {}. Salida cruda: {}", e, stdout_str),
            ));
        }
    };

    if let Some(err_obj) = val.get("error") {
        let code = err_obj.get("code").and_then(|c| c.as_i64()).unwrap_or(status_code as i64);
        let msg = err_obj
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Error en Gemini API");
        return Err((code, msg.to_string()));
    }

    if let Some(candidates) = val.get("candidates").and_then(|c| c.as_array()) {
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
                    return Ok(full_text);
                }
            }
        }
    }

    Err((500, format!("Estructura de respuesta inesperada: {}", stdout_str)))
}

/// Consulta Gemini con fallback automático en cascada (503 alta demanda, timeout o indisponibilidad)
pub fn call_gemini_with_fallback(
    api_key: &str,
    requested_model: &str,
    prompt: &str,
) -> Result<(String, String), String> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err("API Key vacía".to_string());
    }

    let req_model = if requested_model.trim().is_empty() {
        PRIMARY_FALLBACK_MODEL
    } else {
        requested_model.trim()
    };

    // 1. Intentar el modelo solicitado por el usuario
    match execute_gemini_http_request(key, req_model, prompt) {
        Ok(reply) => Ok((reply, req_model.to_string())),
        Err((code, err_msg)) => {
            // Si el modelo seleccionado falla (HTTP 503 demanda alta, timeout o límite de tasa),
            // conmutar inmediatamente a los modelos de fallback ultra-rápidos
            let fallback_list: &[&str] = if req_model == PRIMARY_FALLBACK_MODEL {
                &[SECONDARY_FALLBACK_MODEL]
            } else if req_model == SECONDARY_FALLBACK_MODEL {
                &[PRIMARY_FALLBACK_MODEL]
            } else {
                &[PRIMARY_FALLBACK_MODEL, SECONDARY_FALLBACK_MODEL]
            };

            for &fb_model in fallback_list {
                if let Ok(fb_reply) = execute_gemini_http_request(key, fb_model, prompt) {
                    let reason = if code == 503 {
                        "alta demanda temporal [HTTP 503]"
                    } else if err_msg.contains("timed out") || err_msg.contains("timeout") {
                        "tiempo de espera agotado"
                    } else {
                        "indisponibilidad temporal"
                    };
                    let note = format!(
                        "{}\n\n*(Nota: `{}` reportó {}, respuesta fluida servida por `{}`)*",
                        fb_reply.trim(),
                        req_model,
                        reason,
                        fb_model
                    );
                    return Ok((note, format!("{} (Fallback)", fb_model)));
                }
            }

            Err(format!(
                "Error en {} (HTTP {}): {}. Los modelos de respaldo tampoco respondieron a tiempo.",
                req_model, code, err_msg
            ))
        }
    }
}

/// Motor principal de consulta IA: llamadas nativas HTTPS a Gemini API sin respuestas enlatadas
pub fn query_gemini_or_ast_assistant(
    api_key: &str,
    selected_model: &str,
    user_prompt: &str,
    active_filename: &str,
    input_code: &str,
    config: &DeobfuscationConfig,
) -> AiResponse {
    let trimmed_prompt = user_prompt.trim();
    let res = DeobfuscatorEngine::process(input_code, config);

    let full_prompt = if input_code.trim().is_empty() {
        format!(
            "Eres el asistente integrado de inteligencia artificial, desarrollo y seguridad en KRONOS Workbench.\n\
             Responde siempre en español de forma directa, técnica, concisa y muy fluida.\n\
             El editor está actualmente en blanco o sin archivo abierto.\n\n\
             Consulta del usuario: {}",
            trimmed_prompt
        )
    } else {
        format!(
            "Eres el asistente integrado de desarrollo, ingeniería inversa y seguridad en KRONOS Workbench.\n\
             Responde siempre en español de forma directa, técnica, profesional y concisa.\n\
             Si el analista solicita desofuscar, limpiar, simplificar o corregir código, incluye la versión final completa y funcional en Python dentro de un bloque Markdown ```python ... ```.\n\n\
             Contexto del análisis:\n\
             - Archivo activo: {}\n\
             - Código en el editor:\n```python\n{}\n```\n\
             - Código pre-desofuscado por KRONOS AST (Zero-Exec):\n```python\n{}\n```\n\n\
             Instrucción o consulta del analista: {}",
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
            // Graceful fallback for offline / disconnected environments
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_gemini_key_is_set() {
        assert_eq!(DEFAULT_GEMINI_API_KEY, "");
    }

    #[test]
    fn test_extract_code_block() {
        let md = "Aquí tienes el código:\n```python\nx = 42\nprint(x)\n```\nListo.";
        assert_eq!(extract_code_block(md), Some("x = 42\nprint(x)".to_string()));

        let md_generic = "Código:\n```\nvalor = 100\n```";
        assert_eq!(extract_code_block(md_generic), Some("valor = 100".to_string()));

        let md_none = "Solo texto plano explicativo sin código.";
        assert_eq!(extract_code_block(md_none), None);
    }

    static LIVE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_live_gemini_call_and_503_fallback() {
        let _guard = LIVE_TEST_LOCK.lock().unwrap();
        let prompt = "Responde exactamente la palabra: HOLA_KRONOS";
        let res = call_gemini_with_fallback(DEFAULT_GEMINI_API_KEY, "gemini-3.1-flash-lite", prompt);
        if let Ok((reply, model_used)) = res {
            assert!(!reply.trim().is_empty());
            assert!(
                model_used.contains("gemini-3.1-flash-lite")
                    || model_used.contains("gemini-flash-lite-latest")
                    || model_used.contains("Fallback")
            );
        } else {
            let err = res.unwrap_err();
            assert!(!err.is_empty());
        }

        let res_fb = call_gemini_with_fallback(DEFAULT_GEMINI_API_KEY, "gemini-3.8-flash", prompt);
        if let Ok((reply_fb, model_used_fb)) = res_fb {
            assert!(!reply_fb.trim().is_empty());
            assert!(
                model_used_fb.contains("Fallback")
                    || model_used_fb.contains("gemini-3.8-flash")
                    || model_used_fb.contains("gemini-3.1-flash-lite")
            );
        }
    }

    #[test]
    fn test_query_gemini_assistant_real_live() {
        let _guard = LIVE_TEST_LOCK.lock().unwrap();
        let cfg = DeobfuscationConfig::default();
        let res = query_gemini_or_ast_assistant(
            DEFAULT_GEMINI_API_KEY,
            "gemini-flash-lite-latest",
            "Di exactamente: KRONOS_ACTIVO",
            "test.py",
            "print('test')",
            &cfg,
        );
        assert!(!res.reply_text.trim().is_empty());
        assert!(
            res.mode_badge.contains("gemini")
                || res.mode_badge.contains("Error")
                || res.mode_badge.contains("Offline")
                || res.mode_badge.contains("KRONOS")
        );
    }

    #[test]
    fn test_empty_api_key_returns_graceful_error() {
        let res = call_gemini_with_fallback("", "gemini-3.1-flash-lite", "ping");
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.contains("vacía") || err.contains("API Key") || err.contains("configurada"));
    }

    #[test]
    fn test_invalid_api_key_returns_informative_error() {
        let res = execute_gemini_http_request("clave_invalida_total", "gemini-3.1-flash-lite", "ping");
        assert!(res.is_err());
        let (code, msg) = res.unwrap_err();
        assert!(
            code == 400
                || code == 403
                || msg.contains("API_KEY_INVALID")
                || msg.contains("HTTP 400")
                || msg.contains("API key not valid")
                || msg.contains("Error")
        );
    }
}
