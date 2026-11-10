//! Empirical Adversarial Challenger Suite for M2 - Native Gemini HTTP Client & Resilience
//! Tests MockHttpTransport against adversarial server behaviors:
//! - Empty API key (zero network I/O)
//! - HTTP 429 RateLimitExceeded and model cascade
//! - HTTP 503 ServiceUnavailable and 3-step cascade
//! - HTTP 400/403 InvalidApiKey fast-fail without crashing
//! - Connection and read timeouts -> AiError classification
//! - Malformed / non-JSON responses and empty candidate structures
//! - Concurrency and thread safety

#[path = "../src/core/mod.rs"]
pub mod core;

#[path = "../src/ai/mod.rs"]
pub mod ai;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use ai::gemini::{GeminiProvider, MockHttpTransport};
use ai::models::{MODEL_GEMINI_3_1_FLASH_LITE, MODEL_GEMINI_3_8_FLASH, MODEL_GEMINI_FLASH_LITE_LATEST};
use ai::provider::{AiAction, AiError, AiProvider, AiQueryRequest};

/// Helper to build a standard test request
fn make_request(prompt: &str, model: &str, api_key: &str) -> AiQueryRequest {
    AiQueryRequest {
        prompt: prompt.to_string(),
        model: model.to_string(),
        context_code: String::new(),
        action: AiAction::CustomQuery,
        api_key: api_key.to_string(),
        filename: "test.py".to_string(),
    }
}

// ---------------------------------------------------------------------------
// 1. Empty API Key Handling: Zero Network I/O
// ---------------------------------------------------------------------------
#[test]
fn test_empty_api_key_zero_network_io() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let count_clone = Arc::clone(&call_count);

    // If network / transport is touched, this will record a call or panic
    let mock = Arc::new(MockHttpTransport::new(move |_url, _body| {
        count_clone.fetch_add(1, Ordering::SeqCst);
        panic!("Transport MUST NOT be invoked when API key is empty!");
    }));

    let provider = GeminiProvider::with_transport(mock);

    // Test totally empty string
    let req_empty = make_request("Hello", MODEL_GEMINI_3_1_FLASH_LITE, "");
    let res = provider.query(&req_empty);
    assert_eq!(res.unwrap_err(), AiError::EmptyApiKey);
    assert_eq!(call_count.load(Ordering::SeqCst), 0, "Network I/O occurred on empty key!");

    // Test whitespace-only string
    let req_spaces = make_request("Hello", MODEL_GEMINI_3_1_FLASH_LITE, "   \t\n  ");
    let res_spaces = provider.query(&req_spaces);
    assert_eq!(res_spaces.unwrap_err(), AiError::EmptyApiKey);
    assert_eq!(call_count.load(Ordering::SeqCst), 0, "Network I/O occurred on whitespace key!");
}

// ---------------------------------------------------------------------------
// 2. HTTP 429 Quota Exhausted: Cascade & RateLimitExceeded
// ---------------------------------------------------------------------------
#[test]
fn test_http_429_triggers_cascade_and_succeeds_on_fallback() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let count_clone = Arc::clone(&call_count);

    // Model 1 (3.8) returns 429, Model 2 (3.1-lite) succeeds
    let mock = Arc::new(MockHttpTransport::new(move |_url, _body| {
        let attempt = count_clone.fetch_add(1, Ordering::SeqCst);
        if attempt == 0 {
            let err_json = serde_json::json!({
                "error": {
                    "code": 429,
                    "message": "Resource exhausted: quota exceeded for gemini-3.8-flash",
                    "status": "RESOURCE_EXHAUSTED"
                }
            });
            Ok((429, err_json.to_string()))
        } else {
            let success_json = serde_json::json!({
                "candidates": [{
                    "content": {
                        "parts": [{ "text": "Respuesta desde modelo de respaldo" }]
                    }
                }],
                "usageMetadata": { "totalTokenCount": 25 }
            });
            Ok((200, success_json.to_string()))
        }
    }));

    let provider = GeminiProvider::with_transport(mock);
    let req = make_request("Deobfuscate this", MODEL_GEMINI_3_8_FLASH, "TEST_API_KEY");

    let res = provider.query(&req).expect("Cascade should have succeeded on fallback model");
    assert_eq!(res.reply_text, "Respuesta desde modelo de respaldo");
    assert_eq!(res.model_used, MODEL_GEMINI_3_1_FLASH_LITE);
    assert!(res.mode_badge.contains("Fallback"), "Badge should indicate fallback mode");
    assert_eq!(call_count.load(Ordering::SeqCst), 2, "Should have made exactly 2 attempts");
}

#[test]
fn test_http_429_exhausts_entire_cascade_returns_rate_limit_exceeded() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let count_clone = Arc::clone(&call_count);

    // All models in chain return 429
    let mock = Arc::new(MockHttpTransport::new(move |_url, _body| {
        count_clone.fetch_add(1, Ordering::SeqCst);
        let err_json = serde_json::json!({
            "error": {
                "code": 429,
                "message": "Quota exceeded globally across all tiers",
                "status": "RESOURCE_EXHAUSTED"
            }
        });
        Ok((429, err_json.to_string()))
    }));

    let provider = GeminiProvider::with_transport(mock);
    let req = make_request("Deobfuscate this", MODEL_GEMINI_3_8_FLASH, "TEST_API_KEY");

    let res = provider.query(&req);
    assert!(res.is_err(), "Should return error when all models are 429");
    assert_eq!(res.unwrap_err(), AiError::RateLimitExceeded);
    // Chain for 3.8-flash is: [gemini-3.8-flash, gemini-3.1-flash-lite, gemini-flash-lite-latest] = 3 attempts
    assert_eq!(call_count.load(Ordering::SeqCst), 3, "Should have tried all 3 models in cascade");
}

// ---------------------------------------------------------------------------
// 3. HTTP 503 Overloaded: 3-Step Model Cascade
// ---------------------------------------------------------------------------
#[test]
fn test_http_503_cascade_full_chain_to_third_model() {
    let models_called = Arc::new(std::sync::Mutex::new(Vec::new()));
    let models_clone = Arc::clone(&models_called);

    let mock = Arc::new(MockHttpTransport::new(move |url, _body| {
        let mut list = models_clone.lock().unwrap();
        if url.contains("gemini-3.8-flash") {
            list.push("gemini-3.8-flash");
            let err_json = serde_json::json!({
                "error": {
                    "code": 503,
                    "message": "The model is overloaded. Please try again later."
                }
            });
            Ok((503, err_json.to_string()))
        } else if url.contains("gemini-3.1-flash-lite") {
            list.push("gemini-3.1-flash-lite");
            let err_json = serde_json::json!({
                "error": {
                    "code": 503,
                    "message": "Service unavailable: high traffic load"
                }
            });
            Ok((503, err_json.to_string()))
        } else if url.contains("gemini-flash-lite-latest") {
            list.push("gemini-flash-lite-latest");
            let success_json = serde_json::json!({
                "candidates": [{
                    "content": {
                        "parts": [{ "text": "Respuesta exitosa de gemini-flash-lite-latest" }]
                    }
                }]
            });
            Ok((200, success_json.to_string()))
        } else {
            panic!("Unexpected model URL called: {}", url);
        }
    }));

    let provider = GeminiProvider::with_transport(mock);
    let req = make_request("Optimize AST", MODEL_GEMINI_3_8_FLASH, "TEST_API_KEY");

    let res = provider.query(&req).expect("Cascade should reach gemini-flash-lite-latest and succeed");
    assert_eq!(res.model_used, MODEL_GEMINI_FLASH_LITE_LATEST);
    assert_eq!(res.reply_text, "Respuesta exitosa de gemini-flash-lite-latest");
    assert_eq!(
        *models_called.lock().unwrap(),
        vec!["gemini-3.8-flash", "gemini-3.1-flash-lite", "gemini-flash-lite-latest"]
    );
}

// ---------------------------------------------------------------------------
// 4. HTTP 400 Invalid API Key: Fast Fail Without Crash
// ---------------------------------------------------------------------------
#[test]
fn test_http_400_invalid_api_key_fast_fail_no_cascade() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let count_clone = Arc::clone(&call_count);

    let mock = Arc::new(MockHttpTransport::new(move |_url, _body| {
        count_clone.fetch_add(1, Ordering::SeqCst);
        let err_json = serde_json::json!({
            "error": {
                "code": 400,
                "message": "API key not valid. Please pass a valid API key.",
                "status": "INVALID_ARGUMENT",
                "details": [{
                    "@type": "type.googleapis.com/google.rpc.ErrorInfo",
                    "reason": "API_KEY_INVALID"
                }]
            }
        });
        Ok((400, err_json.to_string()))
    }));

    let provider = GeminiProvider::with_transport(mock);
    let req = make_request("Explain", MODEL_GEMINI_3_8_FLASH, "CORRUPT_KEY");

    let res = provider.query(&req);
    assert!(res.is_err());
    match res.unwrap_err() {
        AiError::InvalidApiKey(msg) => {
            assert!(msg.contains("API key not valid"), "Got unexpected msg: {}", msg);
        }
        other => panic!("Expected AiError::InvalidApiKey, got: {:?}", other),
    }

    // CRITICAL: An invalid key must NOT try other models in the cascade, as it wastes time and network
    assert_eq!(call_count.load(Ordering::SeqCst), 1, "Should fast-fail on 400 without cascade");
}

// ---------------------------------------------------------------------------
// 5. Connection Timeout / Read Timeout
// ---------------------------------------------------------------------------
#[test]
fn test_connection_and_read_timeout_error_classification() {
    let mock_conn_timeout = Arc::new(MockHttpTransport::new(|_url, _body| {
        Err("Error de transporte HTTP: connection timed out".to_string())
    }));

    let provider = GeminiProvider::with_transport(mock_conn_timeout);
    let req = make_request("Test timeout", MODEL_GEMINI_3_1_FLASH_LITE, "TEST_API_KEY");

    let res = provider.query(&req);
    assert!(res.is_err(), "Timeout must produce an error");
    let err = res.unwrap_err();
    eprintln!("[OBSERVED TIMEOUT ERROR]: {:?}", err);
    match err {
        AiError::NetworkError(msg) => {
            assert!(msg.contains("timed out") || msg.contains("transporte"), "Expected transport timeout in message, got: {}", msg);
        }
        other => panic!("CONTRACT VIOLATION: Expected AiError::NetworkError for connection/read timeout, but got: {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// 6. Malformed Responses, Non-JSON, and Empty Candidates
// ---------------------------------------------------------------------------
#[test]
fn test_malformed_non_json_response() {
    let mock_html = Arc::new(MockHttpTransport::new(|_url, _body| {
        Ok((200, "<html><head><title>502 Bad Gateway</title></head><body><h1>Bad Gateway</h1></body></html>".to_string()))
    }));

    let provider = GeminiProvider::with_transport(mock_html);
    let req = make_request("Test malformed", MODEL_GEMINI_3_1_FLASH_LITE, "TEST_API_KEY");

    let res = provider.query(&req);
    assert!(res.is_err(), "HTML/Non-JSON response must return error");
    let err = res.unwrap_err();
    eprintln!("[OBSERVED NON-JSON ERROR]: {:?}", err);
    match err {
        AiError::MalformedResponse(_) => {},
        other => panic!("CONTRACT VIOLATION: Expected AiError::MalformedResponse for non-JSON HTML response, but got: {:?}", other),
    }
}

#[test]
fn test_malformed_empty_candidate_array() {
    let mock_empty = Arc::new(MockHttpTransport::new(|_url, _body| {
        let json_val = serde_json::json!({
            "candidates": []
        });
        Ok((200, json_val.to_string()))
    }));

    let provider = GeminiProvider::with_transport(mock_empty);
    let req = make_request("Test empty candidates", MODEL_GEMINI_3_1_FLASH_LITE, "TEST_API_KEY");

    let res = provider.query(&req);
    assert!(res.is_err(), "Empty candidates array must return error");
    let err = res.unwrap_err();
    eprintln!("[OBSERVED EMPTY CANDIDATES ERROR]: {:?}", err);
    match err {
        AiError::MalformedResponse(_) => {},
        other => panic!("CONTRACT VIOLATION: Expected AiError::MalformedResponse for empty candidates array, but got: {:?}", other),
    }
}

#[test]
fn test_malformed_empty_parts_array() {
    let mock_parts = Arc::new(MockHttpTransport::new(|_url, _body| {
        let json_val = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": []
                }
            }]
        });
        Ok((200, json_val.to_string()))
    }));

    let provider = GeminiProvider::with_transport(mock_parts);
    let req = make_request("Test empty parts", MODEL_GEMINI_3_1_FLASH_LITE, "TEST_API_KEY");

    let res = provider.query(&req);
    assert!(res.is_err(), "Empty parts array must return error");
    let err = res.unwrap_err();
    eprintln!("[OBSERVED EMPTY PARTS ERROR]: {:?}", err);
    match err {
        AiError::MalformedResponse(_) => {},
        other => panic!("CONTRACT VIOLATION: Expected AiError::MalformedResponse for empty parts array, but got: {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// 7. Multi-Threaded Concurrent Query Execution
// ---------------------------------------------------------------------------
#[test]
fn test_concurrent_gemini_provider_queries() {
    let counter = Arc::new(AtomicUsize::new(0));
    let c_clone = Arc::clone(&counter);

    let mock = Arc::new(MockHttpTransport::new(move |_url, _body| {
        let val = c_clone.fetch_add(1, Ordering::SeqCst);
        let resp = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{ "text": format!("Worker response {}", val) }]
                }
            }]
        });
        Ok((200, resp.to_string()))
    }));

    let provider = Arc::new(GeminiProvider::with_transport(mock));
    let mut handles = Vec::new();

    for i in 0..16 {
        let p = Arc::clone(&provider);
        handles.push(thread::spawn(move || {
            let req = make_request(&format!("Prompt {}", i), MODEL_GEMINI_3_1_FLASH_LITE, "MOCK_KEY");
            let res = p.query(&req).expect("Concurrent query should succeed");
            assert!(res.reply_text.starts_with("Worker response "));
        }));
    }

    for h in handles {
        h.join().expect("Worker thread panicked");
    }

    assert_eq!(counter.load(Ordering::SeqCst), 16);
}
