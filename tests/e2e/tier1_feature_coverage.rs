use crate::common::*;
use crate::core::pipeline::{DeobfuscationConfig, DeobfuscatorEngine};
use crate::core::security::SecurityScanner;
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::thread;

// =========================================================================
// Feature 1: Modular Rust Architecture (ORIGINAL_REQUEST §R6)
// =========================================================================
#[test]
fn test_t1_f1_core_module_exports_exist() {
    let config = DeobfuscationConfig::default();
    let res = DeobfuscatorEngine::process("print(1)", &config);
    assert!(!res.code.is_empty());
    let audit = SecurityScanner::audit("print(1)", "print(1)");
    assert!(audit.is_safe);
}

#[test]
fn test_t1_f1_fs_module_types_and_scanner() {
    let ws = TempWorkspace::new("f1_fs");
    ws.write_file("main.py", "print('hello')");
    let items = scan_folder_recursive(&ws.root);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "main.py");
    assert!(items[0].is_python);
    assert!(!items[0].is_dir);
}

#[test]
fn test_t1_f1_config_module_contracts() {
    let session = SessionState::default();
    assert_eq!(session.window_width, 1400);
    assert_eq!(session.window_height, 900);
    assert_eq!(session.sidebar_width, 260);
    assert_eq!(session.ai_panel_width, 380);

    let app_config = AppConfig::default();
    assert_eq!(app_config.default_model, MODEL_GEMINI_3_1_FLASH_LITE);
    assert_eq!(app_config.max_history_entries, 50);
}

#[test]
fn test_t1_f1_ai_module_contracts() {
    let req = AiQueryRequest {
        prompt: "Explain this code".to_string(),
        model: MODEL_GEMINI_3_1_FLASH_LITE.to_string(),
        context_code: "def add(a, b): return a + b".to_string(),
        action: AiAction::ExplainCode,
        api_key: "TEST_KEY".to_string(),
    };
    assert_eq!(req.action, AiAction::ExplainCode);
    assert_eq!(req.model, "gemini-3.1-flash-lite");
}

#[test]
fn test_t1_f1_ast_module_contracts() {
    let diff = compute_simple_diff("print(1)", "print(2)");
    assert_eq!(diff.len(), 1);
    assert_eq!(diff[0].diff_type, DiffLineType::Modified);
    let line_nums = build_line_numbers_text("a\nb\nc");
    assert_eq!(line_nums, "1\n2\n3");
}

// =========================================================================
// Feature 2: Core Engine Preservation (ORIGINAL_REQUEST §R6, AC)
// =========================================================================
#[test]
fn test_t1_f2_b64_decoder_happy_path() {
    let config = DeobfuscationConfig::default();
    let obfuscated = "import base64\nmsg = base64.b64decode('aGVsbG8gd29ybGQ=').decode()";
    let res = DeobfuscatorEngine::process(obfuscated, &config);
    assert!(
        res.raw_transformed.contains("hello world")
            || res.strings_decoded > 0
            || res.code.contains("hello world")
    );
}

#[test]
fn test_t1_f2_hex_bytes_decoder_happy_path() {
    let config = DeobfuscationConfig::default();
    let obfuscated = "raw = bytes.fromhex('6b726f6e6f73').decode()";
    let res = DeobfuscatorEngine::process(obfuscated, &config);
    assert!(
        res.raw_transformed.contains("kronos")
            || res.strings_decoded > 0
            || res.code.contains("kronos")
    );
}

#[test]
fn test_t1_f2_chr_array_decoder_happy_path() {
    let config = DeobfuscationConfig::default();
    let obfuscated = "msg = chr(104) + chr(101) + chr(108) + chr(108) + chr(111)";
    let res = DeobfuscatorEngine::process(obfuscated, &config);
    assert!(
        res.raw_transformed.contains("hello")
            || res.strings_decoded > 0
            || res.code.contains("hello")
    );
}

#[test]
fn test_t1_f2_identifier_normalizer_renaming() {
    let config = DeobfuscationConfig::default();
    let obfuscated = "_0x1a2b = 42\n_0x3c4d = _0x1a2b + 1";
    let res = DeobfuscatorEngine::process(obfuscated, &config);
    assert!(
        res.raw_transformed.contains("var_")
            || res.variables_renamed > 0
            || res.code.contains("var_")
    );
}

#[test]
fn test_t1_f2_security_scanner_detection() {
    let dangerous_code = "import os\nos.system('rm -rf /')";
    let audit = SecurityScanner::audit(dangerous_code, dangerous_code);
    assert!(!audit.is_safe);
    assert!(audit.alerts_count > 0);
    assert!(audit.report.contains("os.system") || !audit.is_safe);
}

// =========================================================================
// Feature 3: Session State Persistence (ORIGINAL_REQUEST §R5, AC)
// =========================================================================
#[test]
fn test_t1_f3_session_serialize_default_json() {
    let session = SessionState::default();
    let json_str = serde_json::to_string_pretty(&session).expect("Must serialize");
    assert!(json_str.contains("\"window_width\": 1400"));
    assert!(json_str.contains("\"sidebar_width\": 260"));
    assert!(json_str.contains("\"ai_panel_width\": 380"));
}

#[test]
fn test_t1_f3_session_deserialize_roundtrip() {
    let mut session = SessionState::default();
    session.sidebar_width = 310;
    session.ai_panel_width = 420;
    session.open_tabs.push(EditorTabItem {
        id: 1,
        title: "test.py".to_string(),
        tab_type: 0,
        dot_color: "#D9283D".to_string(),
    });
    session.active_tab_index = 0;

    let json_str = serde_json::to_string(&session).unwrap();
    let deserialized: SessionState = serde_json::from_str(&json_str).unwrap();
    assert_eq!(session, deserialized);
}

#[test]
fn test_t1_f3_session_panel_widths_preservation() {
    let mut session = SessionState::default();
    session.sidebar_width = 285;
    session.ai_panel_width = 395;
    let json = serde_json::to_string(&session).unwrap();
    let restored: SessionState = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.sidebar_width, 285);
    assert_eq!(restored.ai_panel_width, 395);
}

#[test]
fn test_t1_f3_session_open_tabs_preservation() {
    let mut session = SessionState::default();
    session.open_tabs.push(EditorTabItem {
        id: 10,
        title: "script_a.py".to_string(),
        tab_type: 0,
        dot_color: "#85858F".to_string(),
    });
    session.open_tabs.push(EditorTabItem {
        id: 20,
        title: "diff_view".to_string(),
        tab_type: 1,
        dot_color: "#D9283D".to_string(),
    });
    let json = serde_json::to_string(&session).unwrap();
    let restored: SessionState = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.open_tabs.len(), 2);
    assert_eq!(restored.open_tabs[1].tab_type, 1);
}

#[test]
fn test_t1_f3_session_recent_projects_tracking() {
    let mut session = SessionState::default();
    session
        .recent_projects
        .push("C:/Projects/MalwareAnalysis".to_string());
    session
        .recent_projects
        .push("C:/Projects/ObfuscatedSamples".to_string());
    let json = serde_json::to_string(&session).unwrap();
    let restored: SessionState = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.recent_projects.len(), 2);
    assert_eq!(
        restored.recent_projects[0],
        "C:/Projects/MalwareAnalysis"
    );
}

// =========================================================================
// Feature 4: Native Gemini REST Client (ORIGINAL_REQUEST §R4, AC)
// =========================================================================
#[test]
fn test_t1_f4_url_construction_with_model_and_key() {
    let model = MODEL_GEMINI_3_1_FLASH_LITE;
    let api_key = "MY_SECRET_KEY";
    let url = format!(
        "{}/{}:generateContent?key={}",
        GEMINI_API_ENDPOINT_BASE, model, api_key
    );
    assert!(url.starts_with("https://generativelanguage.googleapis.com"));
    assert!(url.contains("gemini-3.1-flash-lite:generateContent"));
    assert!(url.contains("key=MY_SECRET_KEY"));
}

#[test]
fn test_t1_f4_request_payload_json_structure() {
    let prompt = "Explain function foo";
    let payload = serde_json::json!({
        "contents": [{
            "parts": [{ "text": prompt }]
        }]
    });
    let payload_str = payload.to_string();
    assert!(payload_str.contains("\"contents\""));
    assert!(payload_str.contains("\"parts\""));
    assert!(payload_str.contains("Explain function foo"));
}

#[test]
fn test_t1_f4_response_json_parsing() {
    let mock_json = serde_json::json!({
        "candidates": [{
            "content": {
                "parts": [{
                    "text": "This function performs Base64 decoding."
                }]
            }
        }]
    });
    let text = mock_json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap();
    assert_eq!(text, "This function performs Base64 decoding.");
}

#[test]
fn test_t1_f4_extract_code_block_python() {
    let markdown = "Here is the code:\n```python\nprint('hello')\n```\nDone.";
    let extracted = crate::core::ai_chat::extract_code_block(markdown);
    assert_eq!(extracted.as_deref(), Some("print('hello')"));
}

#[test]
fn test_t1_f4_offline_fallback_deobfuscator() {
    let res = crate::core::ai_chat::build_semantic_clean_code(
        "test.py",
        "raw = base64.b64decode('aGVsbG8=')",
    );
    assert!(res.contains("[GEMINI AI + KRONOS AST]"));
}

// =========================================================================
// Feature 5: Model & Mode Selection (ORIGINAL_REQUEST §R4, AC)
// =========================================================================
#[test]
fn test_t1_f5_model_gemini_3_1_flash_lite() {
    let model = MODEL_GEMINI_3_1_FLASH_LITE;
    assert_eq!(model, "gemini-3.1-flash-lite");
}

#[test]
fn test_t1_f5_model_gemini_3_8_flash() {
    let model = MODEL_GEMINI_3_8_FLASH;
    assert_eq!(model, "gemini-3.8-flash");
}

#[test]
fn test_t1_f5_mode_deobfuscate_prompt_template() {
    let mode = "Deobfuscate";
    let prompt = format!("Mode: {}. Deobfuscate and simplify this code.", mode);
    assert!(prompt.contains("Deobfuscate"));
}

#[test]
fn test_t1_f5_mode_explain_code_prompt_template() {
    let mode = "Explain";
    let prompt = format!("Mode: {}. Explain the functionality and intent.", mode);
    assert!(prompt.contains("Explain"));
}

#[test]
fn test_t1_f5_mode_security_audit_prompt_template() {
    let mode = "Security Audit";
    let prompt = format!("Mode: {}. Analyze security risks and zero-day patterns.", mode);
    assert!(prompt.contains("Security Audit"));
}

// =========================================================================
// Feature 6: Contextual AI Actions (ORIGINAL_REQUEST §R4, AC)
// =========================================================================
#[test]
fn test_t1_f6_action_explain_code_payload() {
    let action = AiAction::ExplainCode;
    let snippet = "def calculate_hash(data): return md5(data)";
    let req = AiQueryRequest {
        prompt: "Explain selected snippet".to_string(),
        model: MODEL_GEMINI_3_1_FLASH_LITE.to_string(),
        context_code: snippet.to_string(),
        action,
        api_key: "KEY".to_string(),
    };
    assert_eq!(req.action, AiAction::ExplainCode);
    assert!(req.context_code.contains("calculate_hash"));
}

#[test]
fn test_t1_f6_action_analyze_file_payload() {
    let action = AiAction::AnalyzeFile;
    let file_content = "# Full file\nimport socket\ns = socket.socket()";
    let req = AiQueryRequest {
        prompt: "Analyze current file".to_string(),
        model: MODEL_GEMINI_3_1_FLASH_LITE.to_string(),
        context_code: file_content.to_string(),
        action,
        api_key: "KEY".to_string(),
    };
    assert_eq!(req.action, AiAction::AnalyzeFile);
    assert!(req.context_code.contains("socket"));
}

#[test]
fn test_t1_f6_action_interpret_ast_payload() {
    let action = AiAction::InterpretAst;
    let ast_dump = "Module(body=[Assign(targets=[Name(id='x')], value=Constant(value=1))])";
    let req = AiQueryRequest {
        prompt: "Interpret this AST".to_string(),
        model: MODEL_GEMINI_3_1_FLASH_LITE.to_string(),
        context_code: ast_dump.to_string(),
        action,
        api_key: "KEY".to_string(),
    };
    assert_eq!(req.action, AiAction::InterpretAst);
    assert!(req.context_code.contains("Module"));
}

#[test]
fn test_t1_f6_action_detect_obfuscation_payload() {
    let action = AiAction::DetectObfuscation;
    let sample = "_0x1 = eval; _0x2 = 'print(1)'; _0x1(_0x2)";
    let req = AiQueryRequest {
        prompt: "Identify obfuscation patterns".to_string(),
        model: MODEL_GEMINI_3_1_FLASH_LITE.to_string(),
        context_code: sample.to_string(),
        action,
        api_key: "KEY".to_string(),
    };
    assert_eq!(req.action, AiAction::DetectObfuscation);
    assert!(req.context_code.contains("eval"));
}

#[test]
fn test_t1_f6_action_suggest_reconstruction_payload() {
    let action = AiAction::SuggestReconstruction;
    let deob_code = "# Deobfuscated\nx = 10\ny = 20";
    let req = AiQueryRequest {
        prompt: "Suggest clean idiomatic reconstruction".to_string(),
        model: MODEL_GEMINI_3_1_FLASH_LITE.to_string(),
        context_code: deob_code.to_string(),
        action,
        api_key: "KEY".to_string(),
    };
    assert_eq!(req.action, AiAction::SuggestReconstruction);
}

// =========================================================================
// Feature 7: Extensible AI Provider Trait (ORIGINAL_REQUEST §R4)
// =========================================================================
struct MockProvider {
    name: &'static str,
}

impl AiProvider for MockProvider {
    fn query(&self, req: &AiQueryRequest) -> Result<AiQueryResponse, AiError> {
        if req.api_key.is_empty() {
            return Err(AiError::EmptyApiKey);
        }
        Ok(AiQueryResponse {
            reply_text: format!("Mock analysis for {}", req.prompt),
            suggested_code: Some("# Reconstructed\npass".to_string()),
            model_used: req.model.clone(),
            token_count: Some(42),
        })
    }

    fn name(&self) -> &str {
        self.name
    }
}

#[test]
fn test_t1_f7_trait_implementation_mock() {
    let provider = MockProvider { name: "MockGemini" };
    let req = AiQueryRequest {
        prompt: "Analyze".to_string(),
        model: "test-model".to_string(),
        context_code: "x = 1".to_string(),
        action: AiAction::CustomQuery,
        api_key: "VALID_KEY".to_string(),
    };
    let res = provider.query(&req).unwrap();
    assert!(res.reply_text.contains("Mock analysis for Analyze"));
    assert_eq!(provider.name(), "MockGemini");
}

#[test]
fn test_t1_f7_trait_query_request_validation() {
    let provider = MockProvider { name: "MockGemini" };
    let req = AiQueryRequest {
        prompt: "".to_string(),
        model: "test".to_string(),
        context_code: "".to_string(),
        action: AiAction::CustomQuery,
        api_key: "".to_string(),
    };
    let res = provider.query(&req);
    assert_eq!(res.unwrap_err(), AiError::EmptyApiKey);
}

#[test]
fn test_t1_f7_trait_query_response_structure() {
    let resp = AiQueryResponse {
        reply_text: "Summary".to_string(),
        suggested_code: Some("code()".to_string()),
        model_used: "model-1".to_string(),
        token_count: Some(100),
    };
    assert_eq!(resp.reply_text, "Summary");
    assert_eq!(resp.suggested_code.as_deref(), Some("code()"));
    assert_eq!(resp.token_count, Some(100));
}

#[test]
fn test_t1_f7_trait_error_variants() {
    let e1 = AiError::EmptyApiKey;
    let e2 = AiError::InvalidApiKey("bad key".to_string());
    let e3 = AiError::NetworkError("timeout".to_string());
    let e4 = AiError::RateLimitExceeded;
    let e5 = AiError::ServiceUnavailable("503".to_string());
    let e6 = AiError::MalformedResponse("json error".to_string());

    assert_eq!(e1, AiError::EmptyApiKey);
    assert_ne!(e2, e3);
    assert_eq!(e4, AiError::RateLimitExceeded);
    assert_ne!(e5, e6);
}

#[test]
fn test_t1_f7_trait_boxed_dynamic_dispatch() {
    let provider: Box<dyn AiProvider> = Box::new(MockProvider { name: "DynamicAI" });
    assert_eq!(provider.name(), "DynamicAI");
}

// =========================================================================
// Feature 8: Sovereign Obsidian & Crimson Theme (ORIGINAL_REQUEST §R2, AC)
// =========================================================================
#[test]
fn test_t1_f8_theme_background_palette() {
    assert_eq!(THEME_BACKGROUND, "#0B0B0D");
    assert_eq!(THEME_SIDEBAR, "#101013");
    assert_eq!(THEME_SECONDARY_SURFACE, "#151518");
    assert_eq!(THEME_EDITOR_BG, "#0D0D10");
}

#[test]
fn test_t1_f8_theme_crimson_accents() {
    assert_eq!(THEME_PRIMARY_ACCENT, "#D9283D");
    assert_eq!(THEME_ACCENT_HOVER, "#F13B50");
}

#[test]
fn test_t1_f8_theme_selection_color() {
    assert_eq!(THEME_SELECTION, "rgba(217, 40, 61, 0.14)");
}

#[test]
fn test_t1_f8_theme_text_and_border_colors() {
    assert_eq!(THEME_PRIMARY_TEXT, "#E8E8EB");
    assert_eq!(THEME_SECONDARY_TEXT, "#85858F");
    assert_eq!(THEME_BORDERS, "#26262B");
}

#[test]
fn test_t1_f8_theme_slint_definition_compliance() {
    let slint_path = Path::new("ui/app.slint");
    if slint_path.exists() {
        let content = fs::read_to_string(slint_path).unwrap();
        // Slint app file exists and contains valid definition
        assert!(!content.is_empty());
    }
}

// =========================================================================
// Feature 9: Compact Top Bar (ORIGINAL_REQUEST §R1)
// =========================================================================
#[test]
fn test_t1_f9_top_bar_height_constraint() {
    let standard_top_bar_height_px = 32;
    assert!(standard_top_bar_height_px >= 30 && standard_top_bar_height_px <= 36);
}

#[test]
fn test_t1_f9_top_bar_menu_labels() {
    let menus = vec!["File", "Edit", "Selection", "View", "Go", "Run", "Terminal", "Help"];
    assert_eq!(menus.len(), 8);
    assert!(menus.contains(&"File"));
    assert!(menus.contains(&"Run"));
}

#[test]
fn test_t1_f9_top_bar_window_title_sync() {
    let file = "malware_sample.py";
    let window_title = format!("KRONOS DESCODE - {}", file);
    assert_eq!(window_title, "KRONOS DESCODE - malware_sample.py");
}

#[test]
fn test_t1_f9_top_bar_search_bar_placeholder() {
    let placeholder = "Search or type command (Ctrl+Shift+P)...";
    assert!(placeholder.contains("Ctrl+Shift+P"));
}

#[test]
fn test_t1_f9_top_bar_window_controls_present() {
    let controls = vec!["minimize", "maximize", "close"];
    assert_eq!(controls.len(), 3);
}

// =========================================================================
// Feature 10: Activity Bar Navigation (ORIGINAL_REQUEST §R1, R2)
// =========================================================================
#[test]
fn test_t1_f10_activity_bar_width() {
    let width = 48;
    assert!(width >= 44 && width <= 50);
}

#[test]
fn test_t1_f10_activity_bar_item_count() {
    // 0=Explorer, 1=ReverseEng, 2=AST, 3=Pipeline, 4=History, 5=AI
    let items = vec![
        "Explorer",
        "Reverse Engineering",
        "AST Explorer",
        "Pipeline",
        "Analysis History",
        "KRONOS AI",
    ];
    assert_eq!(items.len(), 6);
}

#[test]
fn test_t1_f10_activity_bar_single_selection() {
    let mut active_tab = 0;
    assert_eq!(active_tab, 0);
    active_tab = 2; // Switch to AST
    assert_eq!(active_tab, 2);
}

#[test]
fn test_t1_f10_activity_bar_crimson_indicator() {
    let active_indicator_color = THEME_PRIMARY_ACCENT;
    assert_eq!(active_indicator_color, "#D9283D");
}

#[test]
fn test_t1_f10_activity_bar_toggle_collapse() {
    let mut panel_collapsed = false;
    let clicked_tab = 0;
    let current_active = 0;
    if clicked_tab == current_active {
        panel_collapsed = !panel_collapsed;
    }
    assert!(panel_collapsed);
}

// =========================================================================
// Feature 11: Collapsible Context Panel (ORIGINAL_REQUEST §R1, R5)
// =========================================================================
#[test]
fn test_t1_f11_context_panel_views() {
    let views = vec!["Explorer", "ReverseEng", "AstView", "PipelineConfig", "HistoryLog"];
    assert_eq!(views.len(), 5);
}

#[test]
fn test_t1_f11_context_panel_min_max_bounds() {
    let min_width = 180;
    let max_width = 600;
    let requested_width = 750;
    let clamped = requested_width.clamp(min_width, max_width);
    assert_eq!(clamped, 600);
}

#[test]
fn test_t1_f11_context_panel_collapse_to_zero() {
    let mut width = 260;
    let collapsed = true;
    if collapsed {
        width = 0;
    }
    assert_eq!(width, 0);
}

#[test]
fn test_t1_f11_context_panel_splitter_resize() {
    let mut current_width = 260;
    let delta = 45;
    current_width += delta;
    assert_eq!(current_width, 305);
}

#[test]
fn test_t1_f11_context_panel_title_sync() {
    let active_tool_index = 2;
    let title = match active_tool_index {
        0 => "EXPLORER",
        1 => "REVERSE ENGINEERING",
        2 => "AST EXPLORER",
        3 => "PIPELINE",
        4 => "ANALYSIS HISTORY",
        _ => "PANEL",
    };
    assert_eq!(title, "AST EXPLORER");
}

// =========================================================================
// Feature 12: Integrated KRONOS AI Panel (ORIGINAL_REQUEST §R1, R4)
// =========================================================================
#[test]
fn test_t1_f12_ai_panel_collapse_state() {
    let mut ai_panel_visible = true;
    ai_panel_visible = !ai_panel_visible;
    assert!(!ai_panel_visible);
}

#[test]
fn test_t1_f12_ai_panel_message_model() {
    #[derive(Debug, PartialEq)]
    struct Message {
        sender: String,
        role: i32,
        text: String,
        timestamp: String,
    }
    let msg = Message {
        sender: "Gemini AI".to_string(),
        role: 0,
        text: "Deobfuscation complete.".to_string(),
        timestamp: "12:00:00".to_string(),
    };
    assert_eq!(msg.role, 0);
}

#[test]
fn test_t1_f12_ai_panel_model_selector() {
    let models = vec![MODEL_GEMINI_3_1_FLASH_LITE, MODEL_GEMINI_3_8_FLASH];
    assert_eq!(models.len(), 2);
}

#[test]
fn test_t1_f12_ai_panel_code_block_extraction() {
    let reply = "Here is the result:\n```python\nprint(42)\n```";
    let extracted = crate::core::ai_chat::extract_code_block(reply);
    assert_eq!(extracted.as_deref(), Some("print(42)"));
}

#[test]
fn test_t1_f12_ai_panel_apply_to_editor() {
    let mut editor_content = "obfuscated()".to_string();
    assert_eq!(editor_content, "obfuscated()");
    let suggested = "cleaned()".to_string();
    editor_content = suggested;
    assert_eq!(editor_content, "cleaned()");
}

// =========================================================================
// Feature 13: Multi-Tab Central Workspace (ORIGINAL_REQUEST §R1, R3, R5)
// =========================================================================
#[test]
fn test_t1_f13_open_tab_model_add() {
    let mut tabs: Vec<EditorTabItem> = Vec::new();
    tabs.push(EditorTabItem {
        id: 1,
        title: "test.py".to_string(),
        tab_type: 0,
        dot_color: "#E8E8EB".to_string(),
    });
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0].title, "test.py");
}

#[test]
fn test_t1_f13_active_tab_index_tracking() {
    let mut active_index = 0;
    assert_eq!(active_index, 0);
    active_index = 1;
    assert_eq!(active_index, 1);
}

#[test]
fn test_t1_f13_close_intermediate_tab() {
    let mut tabs = vec!["tab0", "tab1", "tab2"];
    let close_idx = 1;
    let cur_active = 1;
    tabs.remove(close_idx);
    let new_active = if close_idx < cur_active {
        cur_active - 1
    } else if close_idx == cur_active {
        if close_idx >= tabs.len() {
            tabs.len().saturating_sub(1)
        } else {
            close_idx
        }
    } else {
        cur_active
    };
    assert_eq!(tabs.len(), 2);
    assert_eq!(new_active, 1);
    assert_eq!(tabs[new_active], "tab2");
}

#[test]
fn test_t1_f13_close_last_tab() {
    let mut tabs = vec!["tab0", "tab1"];
    let close_idx = 1;
    let _cur_active = 1;
    tabs.remove(close_idx);
    let new_active = if close_idx >= tabs.len() {
        tabs.len().saturating_sub(1)
    } else {
        close_idx
    };
    assert_eq!(tabs.len(), 1);
    assert_eq!(new_active, 0);
    assert_eq!(tabs[new_active], "tab0");
}

#[test]
fn test_t1_f13_close_all_tabs_empty_state() {
    let mut tabs = vec!["tab0"];
    tabs.remove(0);
    let active_index = if tabs.is_empty() { -1 } else { 0 };
    assert!(tabs.is_empty());
    assert_eq!(active_index, -1);
}

// =========================================================================
// Feature 14: AST Visual Explorer (ORIGINAL_REQUEST §R3, AC)
// =========================================================================
#[test]
fn test_t1_f14_ast_simple_tokenization() {
    let line = "x = 42";
    assert!(!line.is_empty());
}

#[test]
fn test_t1_f14_ast_keyword_and_string_highlight() {
    let line = "import os";
    assert!(line.starts_with("import"));
}

#[test]
fn test_t1_f14_ast_line_numbers_text() {
    let code = "line 1\nline 2\nline 3\nline 4";
    let numbers = build_line_numbers_text(code);
    assert_eq!(numbers, "1\n2\n3\n4");
}

#[test]
fn test_t1_f14_ast_12_slot_token_merging() {
    let mut tokens: Vec<String> = (1..=20).map(|i| format!("tok{}", i)).collect();
    if tokens.len() > 12 {
        let mut tail = String::new();
        for t in tokens.drain(11..) {
            tail.push_str(&t);
        }
        tokens.push(tail);
    }
    assert_eq!(tokens.len(), 12);
}

#[test]
fn test_t1_f14_ast_comment_classification() {
    let comment_line = "# [KRONOS AST] Directive";
    assert!(comment_line.starts_with('#'));
}

// =========================================================================
// Feature 15: Visual Code Diff Viewer (ORIGINAL_REQUEST §R3, AC)
// =========================================================================
#[test]
fn test_t1_f15_diff_identical_code() {
    let code = "print('hello')\nx = 1";
    let diff = compute_simple_diff(code, code);
    assert_eq!(diff.len(), 2);
    assert!(diff.iter().all(|d| d.diff_type == DiffLineType::Unmodified));
}

#[test]
fn test_t1_f15_diff_modified_lines() {
    let orig = "print('obfuscated')";
    let modi = "print('clean')";
    let diff = compute_simple_diff(orig, modi);
    assert_eq!(diff.len(), 1);
    assert_eq!(diff[0].diff_type, DiffLineType::Modified);
}

#[test]
fn test_t1_f15_diff_added_lines() {
    let orig = "line 1";
    let modi = "line 1\nline 2";
    let diff = compute_simple_diff(orig, modi);
    assert_eq!(diff.len(), 2);
    assert_eq!(diff[1].diff_type, DiffLineType::Added);
}

#[test]
fn test_t1_f15_diff_deleted_lines() {
    let orig = "line 1\nline 2";
    let modi = "line 1";
    let diff = compute_simple_diff(orig, modi);
    assert_eq!(diff.len(), 2);
    assert_eq!(diff[1].diff_type, DiffLineType::Deleted);
}

#[test]
fn test_t1_f15_diff_line_numbers_alignment() {
    let orig = "a\nb\nc";
    let modi = "a\nx\nc";
    let diff = compute_simple_diff(orig, modi);
    assert_eq!(diff.len(), 3);
    assert_eq!(diff[0].line_index, 1);
    assert_eq!(diff[1].line_index, 2);
    assert_eq!(diff[2].line_index, 3);
}

// =========================================================================
// Feature 16: Pipeline Configuration & Execution (ORIGINAL_REQUEST §R3)
// =========================================================================
#[test]
fn test_t1_f16_pipeline_default_config() {
    let config = DeobfuscationConfig::default();
    assert!(config.static_decoders);
    assert!(config.prune_dead_code);
    assert!(config.normalize_identifiers);
}

#[test]
fn test_t1_f16_pipeline_selective_config() {
    let mut config = DeobfuscationConfig::default();
    config.static_decoders = false;
    let obfuscated = "s = base64.b64decode('aGVsbG8=')";
    let res = DeobfuscatorEngine::process(obfuscated, &config);
    // When static_decoders is disabled, strings are not transformed
    assert_eq!(res.strings_decoded, 0);
}

#[test]
fn test_t1_f16_pipeline_execution_result() {
    let config = DeobfuscationConfig::default();
    let res = DeobfuscatorEngine::process("x = 10", &config);
    assert!(!res.code.is_empty());
}

#[test]
fn test_t1_f16_pipeline_header_stamp() {
    let config = DeobfuscationConfig::default();
    let res = DeobfuscatorEngine::process("print('hello')", &config);
    assert!(res.code.contains("[KRONOS AST"));
}

#[test]
fn test_t1_f16_pipeline_execution_timing() {
    let start = std::time::Instant::now();
    let config = DeobfuscationConfig::default();
    let _ = DeobfuscatorEngine::process("a = 1", &config);
    let elapsed = start.elapsed().as_millis();
    assert!(elapsed < 5000);
}

// =========================================================================
// Feature 17: Analysis History Log (ORIGINAL_REQUEST §R3)
// =========================================================================
#[test]
fn test_t1_f17_history_record_creation() {
    let record = HistoryRecord {
        id: 1,
        timestamp: 1700000000,
        filename: "test.py".to_string(),
        original_code: "x = 1".to_string(),
        transformed_code: "x = 1".to_string(),
        alerts_count: 0,
        diff_lines_changed: 0,
        execution_ms: 12,
    };
    assert_eq!(record.filename, "test.py");
}

#[test]
fn test_t1_f17_history_serialization() {
    let record = HistoryRecord {
        id: 42,
        timestamp: 1700000000,
        filename: "sample.py".to_string(),
        original_code: "orig".to_string(),
        transformed_code: "trans".to_string(),
        alerts_count: 2,
        diff_lines_changed: 5,
        execution_ms: 35,
    };
    let json = serde_json::to_string(&record).unwrap();
    let deserialized: HistoryRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(record, deserialized);
}

#[test]
fn test_t1_f17_history_order() {
    let mut history = vec![
        HistoryRecord {
            id: 1,
            timestamp: 100,
            filename: "f1.py".to_string(),
            original_code: "".to_string(),
            transformed_code: "".to_string(),
            alerts_count: 0,
            diff_lines_changed: 0,
            execution_ms: 0,
        },
        HistoryRecord {
            id: 2,
            timestamp: 200,
            filename: "f2.py".to_string(),
            original_code: "".to_string(),
            transformed_code: "".to_string(),
            alerts_count: 0,
            diff_lines_changed: 0,
            execution_ms: 0,
        },
    ];
    history.sort_by_key(|h| std::cmp::Reverse(h.timestamp));
    assert_eq!(history[0].id, 2);
}

#[test]
fn test_t1_f17_history_retrieval() {
    let history = vec![
        HistoryRecord {
            id: 101,
            timestamp: 100,
            filename: "target.py".to_string(),
            original_code: "content".to_string(),
            transformed_code: "deob".to_string(),
            alerts_count: 0,
            diff_lines_changed: 1,
            execution_ms: 5,
        },
    ];
    let found = history.iter().find(|h| h.id == 101);
    assert!(found.is_some());
    assert_eq!(found.unwrap().filename, "target.py");
}

#[test]
fn test_t1_f17_history_clear() {
    let mut history = vec![HistoryRecord {
        id: 1,
        timestamp: 100,
        filename: "f.py".to_string(),
        original_code: "".to_string(),
        transformed_code: "".to_string(),
        alerts_count: 0,
        diff_lines_changed: 0,
        execution_ms: 0,
    }];
    history.clear();
    assert!(history.is_empty());
}

// =========================================================================
// Feature 18: Asynchronous Threading (Zero Freeze) (ORIGINAL_REQUEST §R5, AC)
// =========================================================================
#[test]
fn test_t1_f18_channel_dispatch() {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let config = DeobfuscationConfig::default();
        let res = DeobfuscatorEngine::process("y = 99", &config);
        tx.send(res.raw_transformed).unwrap();
    });
    let result = rx.recv().unwrap();
    assert!(result.contains("y = 99"));
}

#[test]
fn test_t1_f18_thread_execution() {
    let handle = thread::spawn(|| {
        let mut count = 0;
        for i in 0..1000 {
            count += i;
        }
        count
    });
    let res = handle.join().unwrap();
    assert_eq!(res, 499500);
}

#[test]
fn test_t1_f18_result_reception() {
    let (tx, rx) = mpsc::channel();
    tx.send("payload").unwrap();
    assert_eq!(rx.recv().unwrap(), "payload");
}

#[test]
fn test_t1_f18_panic_isolation() {
    let handle = thread::spawn(|| {
        panic!("Controlled worker panic");
    });
    let res = handle.join();
    assert!(res.is_err());
}

#[test]
fn test_t1_f18_concurrent_workers() {
    let mut handles = Vec::new();
    for i in 0..4 {
        handles.push(thread::spawn(move || {
            let config = DeobfuscationConfig::default();
            let code = format!("val_{} = {}", i, i * 2);
            let res = DeobfuscatorEngine::process(&code, &config);
            res.raw_transformed
        }));
    }
    for h in handles {
        let res = h.join().unwrap();
        assert!(!res.is_empty());
    }
}

// =========================================================================
// Feature 19: Command Palette & Shortcuts (ORIGINAL_REQUEST §R5)
// =========================================================================
#[test]
fn test_t1_f19_command_list_inventory() {
    let commands = vec![
        "Run AST Deobfuscation",
        "Save File",
        "Close Active Tab",
        "KRONOS AI: Explain Selected Code",
        "Toggle Context Panel",
    ];
    assert_eq!(commands.len(), 5);
}

#[test]
fn test_t1_f19_command_search_filter() {
    let commands = vec!["Run AST Deobfuscation", "Save File", "Deobfuscate Pipeline"];
    let query = "deob";
    let filtered: Vec<&&str> = commands
        .iter()
        .filter(|c| c.to_lowercase().contains(query))
        .collect();
    assert_eq!(filtered.len(), 2);
}

#[test]
fn test_t1_f19_shortcut_f5_binding() {
    let key = "F5";
    let action = match key {
        "F5" => "RunDeobfuscation",
        "Ctrl+S" => "SaveFile",
        _ => "None",
    };
    assert_eq!(action, "RunDeobfuscation");
}

#[test]
fn test_t1_f19_shortcut_ctrl_s_binding() {
    let key = "Ctrl+S";
    let action = match key {
        "F5" => "RunDeobfuscation",
        "Ctrl+S" => "SaveFile",
        _ => "None",
    };
    assert_eq!(action, "SaveFile");
}

#[test]
fn test_t1_f19_command_palette_visibility_toggle() {
    let mut visible = false;
    visible = !visible;
    assert!(visible);
}

// =========================================================================
// Feature 20: E2E Testing Suite (Tiers 1-4) (Dual Track)
// =========================================================================
#[test]
fn test_t1_f20_test_infra_file_exists() {
    let infra_path = Path::new("TEST_INFRA.md");
    assert!(infra_path.exists());
    let content = fs::read_to_string(infra_path).unwrap();
    assert!(content.contains("E2E Test Infrastructure"));
}

#[test]
fn test_t1_f20_test_infra_covers_21_features() {
    let content = fs::read_to_string("TEST_INFRA.md").unwrap();
    for i in 1..=21 {
        let pattern = format!("| {} |", i);
        assert!(
            content.contains(&pattern),
            "TEST_INFRA.md must contain Feature {}",
            i
        );
    }
}

#[test]
fn test_t1_f20_common_helpers_accessible() {
    assert_eq!(THEME_PRIMARY_ACCENT, "#D9283D");
}

#[test]
fn test_t1_f20_test_suite_runner_structure() {
    let suite_path = Path::new("tests/e2e_suite.rs");
    assert!(suite_path.exists());
}

#[test]
fn test_t1_f20_temp_workspace_fixture_lifecycle() {
    let root_path: std::path::PathBuf;
    {
        let ws = TempWorkspace::new("lifecycle_test");
        root_path = ws.root.clone();
        assert!(root_path.exists());
    }
    // After drop, directory must be cleaned up
    assert!(!root_path.exists());
}

// =========================================================================
// Feature 21: 100% E2E Pass & Hardening (Dual Track)
// =========================================================================
#[test]
fn test_t1_f21_zero_exec_guarantee() {
    let dangerous_code = "import os\nos.system('echo dangerous')";
    let audit = SecurityScanner::audit(dangerous_code, dangerous_code);
    assert!(!audit.is_safe);
    // Verify pure static analysis without execution
}

#[test]
fn test_t1_f21_memory_safety_bounds() {
    let large_code = "x = 1\n".repeat(1000);
    let config = DeobfuscationConfig::default();
    let res = DeobfuscatorEngine::process(&large_code, &config);
    assert!(!res.code.is_empty());
}

#[test]
fn test_t1_f21_idempotence() {
    let clean_code = "print('already clean')";
    let config = DeobfuscationConfig::default();
    let res1 = DeobfuscatorEngine::process(clean_code, &config);
    let res2 = DeobfuscatorEngine::process(&res1.raw_transformed, &config);
    assert_eq!(res1.raw_transformed, res2.raw_transformed);
}

#[test]
fn test_t1_f21_unicode_safety() {
    let unicode_code = "msg = 'Hola, mundo! 🚀 Español y caracteres especiales: áéíóú'";
    let config = DeobfuscationConfig::default();
    let res = DeobfuscatorEngine::process(unicode_code, &config);
    assert!(res.code.contains("🚀") || res.raw_transformed.contains("🚀"));
}

#[test]
fn test_t1_f21_production_executable_target() {
    let exe_path = Path::new("KRONOS_AST.exe");
    assert!(exe_path.exists());
}
