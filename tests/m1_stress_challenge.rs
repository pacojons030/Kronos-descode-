//! Adversarial Empirical Stress Test Suite for Milestone 1
//! Evaluates SessionState and Workspace File Manager under extreme, malformed, and concurrent conditions.

#![allow(clippy::field_reassign_with_default, clippy::permissions_set_readonly_false)]

slint::include_modules!();

#[path = "../src/core/mod.rs"]
pub mod core;

#[path = "../src/config/session.rs"]
pub mod session;

#[path = "../src/fs/workspace.rs"]
pub mod workspace;

#[path = "../src/project/manager.rs"]
pub mod project;

#[path = "../src/analysis/history.rs"]
pub mod analysis;

pub mod fs {
    pub use super::workspace::*;
}

#[path = "../src/ast/tree.rs"]
pub mod ast_tree;

use session::{SessionState, TabSessionItem};
use std::fs as std_fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn create_temp_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kronos_m1_challenger_{}_{}", name, std::process::id()));
    let _ = std_fs::remove_dir_all(&dir);
    std_fs::create_dir_all(&dir).expect("Failed to create temporary challenge directory");
    dir
}

// =========================================================================
// TEST 1: Corrupted, Malformed, Truncated, and Garbage JSON Inputs
// =========================================================================
#[test]
fn test_stress_corrupt_and_malformed_json_inputs() {
    let temp_dir = create_temp_test_dir("corrupt_json");

    let malformed_cases: &[(&str, &str)] = &[
        ("empty.json", ""),
        ("whitespace.json", "   \t\r\n   "),
        ("truncated_start.json", "{"),
        ("truncated_mid.json", "{\"window_width\": 1280, \"open_tabs\": [{\"id\": 1"),
        ("truncated_end.json", "{\"window_width\": 1280, \"open_tabs\": []"),
        ("syntax_error.json", "{\"window_width\": 1280, , , \"window_height\": 800}"),
        ("wrong_type_top_array.json", "[1, 2, 3, 4]"),
        ("wrong_type_top_integer.json", "123456789"),
        ("wrong_type_top_string.json", "\"not a session state object\""),
        ("wrong_type_top_bool.json", "true"),
        ("wrong_type_top_null.json", "null"),
        ("wrong_fields.json", "{\"unexpected_key_1\": 42, \"nested\": {\"foo\": \"bar\"}}"),
        ("null_bytes.json", "{\x00\"window_width\":\x001280\x00}"),
        ("unicode_garbage.json", "🔥💀⚡🚀\u{FFFF}\u{FFFE}<!DOCTYPE html><html>"),
    ];

    for (filename, content) in malformed_cases {
        let file_path = temp_dir.join(filename);
        std_fs::write(&file_path, content).expect("Failed to write test file");

        // 1. load_from_path must return an Err, never panic
        let load_res = SessionState::load_from_path(&file_path);
        // Note: null might or might not deserialize depending on serde, but must not panic
        if *filename == "wrong_fields.json" {
            // #[serde(default)] should accept unknown fields and populate defaults
            assert!(load_res.is_ok(), "Expected wrong_fields to fallback cleanly via serde(default)");
        } else if *filename == "wrong_type_top_null.json" {
            // null top level is invalid for struct
            assert!(load_res.is_err());
        }

        // 2. load_or_default_from_path must never panic and must return a valid default state
        let recovered = SessionState::load_or_default_from_path(&file_path);
        assert!(
            recovered.window_width >= 1100 || recovered.window_width == 1280,
            "Recovered state must have valid default dimensions for {}",
            filename
        );
        assert_eq!(recovered.active_tool, "explorer");
    }

    let _ = std_fs::remove_dir_all(&temp_dir);
}

// =========================================================================
// TEST 2: Quarantine Behavior and Corrupt File Longevity
// =========================================================================
#[test]
fn test_stress_quarantine_creates_backup_and_check_original_file() {
    let temp_dir = create_temp_test_dir("quarantine");
    let corrupt_file = temp_dir.join("corrupt_session.json");
    std_fs::write(&corrupt_file, "{ this is obviously corrupted json }").unwrap();

    // Call load_or_default_from_path which triggers quarantine
    let loaded = SessionState::load_or_default_from_path(&corrupt_file);
    assert_eq!(loaded, SessionState::default());

    // Check if quarantine backup file was created
    let mut found_backup = false;
    for entry in std_fs::read_dir(&temp_dir).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.contains("corrupt.") && name.ends_with(".json") {
            found_backup = true;
            let backup_content = std_fs::read_to_string(entry.path()).unwrap();
            assert_eq!(backup_content, "{ this is obviously corrupted json }");
        }
    }
    assert!(found_backup, "Quarantine backup file was not created");

    // EMPIRICAL OBSERVATION: Does the original corrupt file still exist?
    let original_still_exists = corrupt_file.exists();
    println!("[EMPIRICAL] Original corrupt file still exists after quarantine: {}", original_still_exists);
    assert!(!original_still_exists, "Original corrupt file must be removed to prevent quarantine loops");

    let _ = std_fs::remove_dir_all(&temp_dir);
}

// =========================================================================
// TEST 3: Negative Dimensions and Boundary Clamping Gap
// =========================================================================
#[test]
fn test_stress_negative_and_out_of_bounds_dimensions() {
    // 1. Negative integer in JSON for window_width (u32)
    let negative_json = r#"{
        "window_width": -100,
        "window_height": -200
    }"#;
    let res = SessionState::from_json(negative_json);
    assert!(res.is_err(), "Negative numbers must be rejected by serde for unsigned u32 fields");

    // 2. Direct construction with extreme / out-of-bounds values
    let mut state = SessionState::default();
    state.window_width = 10;
    state.window_height = 10;
    state.context_panel_width = -500.0;
    state.ai_panel_width = 99999.0;
    state.active_model = "   ".to_string();
    state.active_tab_index = 999;
    state.open_tabs = vec![
        TabSessionItem {
            id: 1,
            title: "test.py".to_string(),
            file_path: None,
            tab_type: 0,
            is_dirty: false,
        }
    ];

    // Call clamp() explicitly
    state.clamp();

    assert_eq!(state.window_width, 1100, "window_width must be clamped to minimum 1100");
    assert_eq!(state.window_height, 700, "window_height must be clamped to minimum 700");
    assert_eq!(state.context_panel_width, 160.0, "context_panel_width must be clamped to minimum 160.0");
    assert_eq!(state.ai_panel_width, 700.0, "ai_panel_width must be clamped to maximum 700.0");
    assert_eq!(state.active_model, "gemini-3.1-flash-lite", "Empty model must fallback to default");
    assert_eq!(state.active_tab_index, 0, "Out of bounds tab index must be clamped to 0");

    // 3. CRITICAL EMPIRICAL CHECK: Is clamp() called during from_json or load_or_default_from_path?
    let unclamped_json = r#"{
        "window_width": 20,
        "window_height": 20,
        "context_panel_width": -50.0,
        "ai_panel_width": 10000.0,
        "active_tab_index": 500,
        "open_tabs": []
    }"#;

    let loaded_state = SessionState::from_json(unclamped_json).unwrap();
    println!("[EMPIRICAL] loaded_state.window_width after automatic clamp: {}", loaded_state.window_width);
    println!("[EMPIRICAL] loaded_state.context_panel_width after automatic clamp: {}", loaded_state.context_panel_width);
    println!("[EMPIRICAL] loaded_state.active_tab_index after automatic clamp: {}", loaded_state.active_tab_index);

    assert_eq!(loaded_state.window_width, 1100, "window_width must be clamped to min 1100 upon deserialization");
    assert_eq!(loaded_state.window_height, 700, "window_height must be clamped to min 700 upon deserialization");
    assert_eq!(loaded_state.context_panel_width, 160.0, "context_panel_width must be clamped to min 160.0 upon deserialization");
    assert_eq!(loaded_state.ai_panel_width, 700.0, "ai_panel_width must be clamped to max 700.0 upon deserialization");
    assert_eq!(loaded_state.active_tab_index, -1, "active_tab_index must be -1 when open_tabs is empty");
}

// =========================================================================
// TEST 4: NaN and Infinity Float Serialization Behavior
// =========================================================================
#[test]
fn test_stress_nan_and_infinity_serialization() {
    let mut state = SessionState::default();
    state.context_panel_width = f32::NAN;

    // 1. Clamping in to_json neutralizes NaN before serialization:
    let json_res = state.to_json().expect("to_json serializes pre-clamped state");
    assert!(!json_res.contains("\"context_panel_width\": null"), "NaN must be clamped and not serialized as null");
    assert!(json_res.contains("\"context_panel_width\": 160.0"));

    // 2. Pre-clamped state deserializes cleanly without null error:
    let deserialized = SessionState::from_json(&json_res).expect("Pre-clamped state must deserialize cleanly");
    assert_eq!(deserialized.context_panel_width, 160.0);

    // 3. Clamping neutralizes NaN to 160.0, restoring valid JSON roundtrip
    state.clamp();
    assert!(!state.context_panel_width.is_nan());
    assert_eq!(state.context_panel_width, 160.0);
    let clamped_json = state.to_json().expect("Clamped state must serialize cleanly");
    assert!(!clamped_json.contains("\"context_panel_width\": null"));
    let restored = SessionState::from_json(&clamped_json).expect("Clamped state must deserialize cleanly");
    assert_eq!(restored.context_panel_width, 160.0);
}

// =========================================================================
// TEST 5: Enormous Tab Arrays and Recent Projects Stress
// =========================================================================
#[test]
fn test_stress_enormous_tab_arrays() {
    let temp_dir = create_temp_test_dir("large_arrays");
    let session_file = temp_dir.join("large_session.json");

    let mut state = SessionState::default();
    let num_tabs = 5_000;
    for i in 0..num_tabs {
        state.open_tabs.push(TabSessionItem {
            id: i,
            title: format!("script_{}.py", i),
            file_path: Some(format!("C:\\workspace\\subfolder\\script_{}.py", i)),
            tab_type: i % 4,
            is_dirty: (i % 2) == 0,
        });
    }

    let num_projects = 2_000;
    for i in 0..num_projects {
        state.recent_projects.push(format!("C:\\projects\\project_repo_{}", i));
    }

    let t0 = Instant::now();
    state.save_to_path(&session_file).expect("Saving 5000 tabs must succeed");
    let save_duration = t0.elapsed();
    println!("[EMPIRICAL] Save 5000 tabs + 2000 projects took: {:?}", save_duration);

    let t1 = Instant::now();
    let loaded = SessionState::load_from_path(&session_file).expect("Loading 5000 tabs must succeed");
    let load_duration = t1.elapsed();
    println!("[EMPIRICAL] Load 5000 tabs + 2000 projects took: {:?}", load_duration);

    assert_eq!(loaded.open_tabs.len(), num_tabs as usize);
    assert_eq!(loaded.recent_projects.len(), num_projects as usize);
    assert!(save_duration < Duration::from_secs(5), "Save must complete in under 5 seconds");
    assert!(load_duration < Duration::from_secs(5), "Load must complete in under 5 seconds");

    let _ = std_fs::remove_dir_all(&temp_dir);
}

// =========================================================================
// TEST 6: Rapid Concurrent Saves (Race Condition & Lock Contention)
// =========================================================================
#[test]
fn test_stress_rapid_concurrent_saves() {
    let temp_dir = create_temp_test_dir("concurrent_saves");
    let target_file = temp_dir.join("concurrent_session.json");

    let num_threads = 8;
    let iterations_per_thread = 15;
    let success_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();

    for t_idx in 0..num_threads {
        let path = target_file.clone();
        let succ = Arc::clone(&success_count);
        let errs = Arc::clone(&error_count);

        let handle = thread::spawn(move || {
            for iter in 0..iterations_per_thread {
                let mut state = SessionState::default();
                state.window_width = 1200 + (t_idx * 10 + iter) as u32;
                state.active_tool = format!("tool_{}_{}", t_idx, iter);

                match state.save_to_path(&path) {
                    Ok(_) => {
                        succ.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(e) => {
                        errs.fetch_add(1, Ordering::SeqCst);
                        // Record verbatim error for empirical analysis
                        println!("[EMPIRICAL CONCURRENT COLLISION] Thread {} iter {}: {:?}", t_idx, iter, e.kind());
                    }
                }
                thread::sleep(Duration::from_millis(2));
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().expect("Thread panicked during concurrent save stress test");
    }

    let successes = success_count.load(Ordering::SeqCst);
    let errors = error_count.load(Ordering::SeqCst);
    println!("[EMPIRICAL] Concurrent saves: {} successes, {} collisions/errors", successes, errors);
    assert_eq!(errors, 0, "Concurrent saves with unique PID/counter temp paths must yield 0 collision errors");
    assert_eq!(successes, (num_threads * iterations_per_thread) as usize);

    // Verify that the final file exists and is valid JSON
    assert!(target_file.exists(), "Target file must exist after concurrent saves");
    let loaded = SessionState::load_or_default_from_path(&target_file);
    assert!(loaded.window_width >= 1100, "Final state must be valid non-corrupt session");

    let _ = std_fs::remove_dir_all(&temp_dir);
}

// =========================================================================
// TEST 7: Read-Only Filesystem / Permission Simulation
// =========================================================================
#[test]
fn test_stress_readonly_file_handling() {
    let temp_dir = create_temp_test_dir("readonly_stress");
    let ro_file = temp_dir.join("readonly_session.json");

    // Write initial file
    let initial_state = SessionState::default();
    initial_state.save_to_path(&ro_file).unwrap();

    // Mark file as read-only
    let mut perms = std_fs::metadata(&ro_file).unwrap().permissions();
    perms.set_readonly(true);
    std_fs::set_permissions(&ro_file, perms.clone()).unwrap();

    let mut state = SessionState::default();
    state.window_width = 1920;

    let save_res = state.save_to_path(&ro_file);
    println!("[EMPIRICAL] save_to_path on read-only file returned: {:?}", save_res);
    
    // Check if the file was replaced or if it retained old content
    let current_content = std_fs::read_to_string(&ro_file).unwrap();
    let current_state = SessionState::from_json(&current_content).unwrap();
    println!("[EMPIRICAL] Current window_width on disk after save to readonly file: {}", current_state.window_width);

    // Reset permissions so cleanup succeeds
    perms.set_readonly(false);
    let _ = std_fs::set_permissions(&ro_file, perms);
    let _ = std_fs::remove_dir_all(&temp_dir);
}

// =========================================================================
// TEST 8: Invalid Path Characters and Non-Existent Parent Dirs
// =========================================================================
#[test]
fn test_stress_invalid_paths() {
    let state = SessionState::default();

    // 1. Completely invalid Windows path characters
    let invalid_path = Path::new("Z:\\non_existent_drive_98765\\invalid:chars*?<>|\\session.json");
    let res = state.save_to_path(invalid_path);
    assert!(res.is_err(), "Saving to invalid drive/path must return Err, not panic");

    // 2. Empty path
    let empty_path = Path::new("");
    let res_empty = state.save_to_path(empty_path);
    assert!(res_empty.is_err(), "Saving to empty path must return Err, not panic");

    // 3. Deep non-existent nested parent dirs: save_to_path has create_dir_all
    let temp_dir = create_temp_test_dir("auto_mkdir");
    let deep_nested = temp_dir.join("sub1").join("sub2").join("sub3").join("deep_session.json");
    let res_deep = state.save_to_path(&deep_nested);
    assert!(res_deep.is_ok(), "save_to_path must automatically create parent directories");
    assert!(deep_nested.exists());

    let _ = std_fs::remove_dir_all(&temp_dir);
}

// =========================================================================
// TEST 9: Workspace File Manager: Deep Recursion (120+ Folders Deep)
// =========================================================================
#[test]
fn test_stress_workspace_deep_recursion_120_levels() {
    let temp_dir = create_temp_test_dir("deep_120_levels");

    // Build 120-deep directory chain: temp_dir/d0/d1/.../d119/leaf.py
    let mut current = temp_dir.clone();
    for i in 0..120 {
        current = current.join(format!("d_{}", i));
    }
    std_fs::create_dir_all(&current).expect("Failed to create 120 deep directory chain");
    std_fs::write(current.join("leaf.py"), "print('deep')").unwrap();

    // 1. scan_folder_recursive (Slint UI model) has hardcoded depth > 4 guard
    let items = workspace::scan_folder_recursive(&temp_dir);
    // Must complete safely without stack overflow
    assert!(items.len() <= 600);
    for item in &items {
        assert!(item.depth <= 4, "scan_folder_recursive must never exceed depth 4");
    }

    // 2. scan_folder_entries with max_depth = 150
    let entries = workspace::scan_folder_entries(&temp_dir, 150, 1000);
    // Must complete safely without stack overflow and respect max_count
    assert!(entries.len() <= 1000);
    let max_observed_depth = entries.iter().map(|e| e.depth).max().unwrap_or(0);
    println!("[EMPIRICAL] Deep folder recursion max observed depth: {}", max_observed_depth);
    assert!(max_observed_depth <= 150);

    let _ = std_fs::remove_dir_all(&temp_dir);
}

// =========================================================================
// TEST 10: Workspace Line Manipulation Extreme Indices and OOM Boundary
// =========================================================================
#[test]
fn test_stress_workspace_line_manipulation_boundary_conditions() {
    let code = "def foo():\n    return 42\n";

    // 1. Normal line replacement
    let r1 = workspace::replace_single_line(code, 0, "def bar():");
    assert_eq!(r1, "def bar():\n    return 42\n");

    // 2. Empty code buffer
    let r2 = workspace::replace_single_line("", 0, "line1");
    assert_eq!(r2, "line1");

    // 3. Moderate out of bounds index (e.g., 5 lines beyond end)
    let r3 = workspace::replace_single_line("a\nb", 5, "f");
    assert_eq!(r3, "a\nb\n\n\n\nf");

    // Extreme index guardrail (must not OOM or allocate millions of strings)
    let safe_replaced = workspace::replace_single_line("hello\nworld", 10_000_000, "boom");
    assert_eq!(safe_replaced, "hello\nworld", "Extreme indices must be safely rejected without OOM");

    let safe_inserted = workspace::insert_newline_after("hello\nworld", 10_000_000, "hello");
    assert_eq!(safe_inserted, "hello\nworld", "Extreme indices must be safely rejected without OOM");

    // 4. insert_newline_after with indentation preservation
    let nl = workspace::insert_newline_after("        indented()", 0, "        indented()");
    assert_eq!(nl, "        indented()\n        ");

    // 5. split_code_lines on empty string vs CRLF vs LF
    assert_eq!(workspace::split_code_lines(""), vec![""]);
    assert_eq!(workspace::split_code_lines("line1\r\nline2\r\n"), vec!["line1", "line2", ""]);
    assert_eq!(workspace::split_code_lines("a\nb\nc"), vec!["a", "b", "c"]);

    // 6. split_code_lines on large buffer (100,000 lines)
    let mut large_buffer = String::with_capacity(1_500_000);
    for i in 0..100_000 {
        large_buffer.push_str("line_");
        large_buffer.push_str(&i.to_string());
        large_buffer.push('\n');
    }
    let t0 = Instant::now();
    let lines = workspace::split_code_lines(&large_buffer);
    let split_duration = t0.elapsed();
    println!("[EMPIRICAL] Splitting 100,000 lines took: {:?}", split_duration);
    assert_eq!(lines.len(), 100_001); // Trailing empty line preserved
    assert!(split_duration < Duration::from_millis(500));
}

// =========================================================================
// TEST 11: Workspace Special Characters, Unicode, and Blacklist Filtering
// =========================================================================
#[test]
fn test_stress_workspace_unicode_and_special_filenames() {
    let temp_dir = create_temp_test_dir("special_names");

    // Unicode filenames, emojis, RTL characters, spaces
    let special_files = &[
        "código_español_ñ.py",
        "файл_тест_кириллица.py",
        "日本語スクリプト.py",
        "script with spaces and symbols #1 (v2).py",
        "emoji_🚀_script.py",
        "README.md.bak",
        "__hidden_script__.py",
    ];

    for name in special_files {
        std_fs::write(temp_dir.join(name), "print('ok')").unwrap();
    }

    // Blacklisted directories
    let blacklisted = &[".git", "__pycache__", "node_modules", "target", "venv", ".env", "dist", "build"];
    for b in blacklisted {
        let bdir = temp_dir.join(b);
        std_fs::create_dir_all(&bdir).unwrap();
        std_fs::write(bdir.join("should_be_ignored.py"), "print('ignored')").unwrap();
    }

    let entries = workspace::scan_folder_entries(&temp_dir, 4, 100);

    // Verify all special unicode files were correctly scanned
    for name in special_files {
        assert!(
            entries.iter().any(|e| e.name == *name),
            "Failed to find unicode file: {}",
            name
        );
    }

    // Verify zero blacklisted entries
    for b in blacklisted {
        assert!(
            !entries.iter().any(|e| e.name == *b),
            "Blacklisted directory must not be present: {}",
            b
        );
    }

    let _ = std_fs::remove_dir_all(&temp_dir);
}

// =========================================================================
// TEST 12: ProjectManager and AnalysisHistory Ring Buffer Stress
// =========================================================================
#[test]
fn test_stress_project_and_history_ring_buffers() {
    // 1. ProjectManager with 0 max_recent -> defaults to 10
    let mut pm = project::ProjectManager::new(0);
    assert_eq!(pm.max_recent, 10);

    let temp_dir = create_temp_test_dir("proj_ring_buffer");
    for i in 0..25 {
        let p = temp_dir.join(format!("proj_{}", i));
        std_fs::create_dir_all(&p).unwrap();
        pm.open_project(&p).expect("Project open must succeed");
    }

    assert_eq!(pm.recent_projects.len(), 10, "Recent projects must be capped at max_recent");
    assert_eq!(pm.recent_projects[0].name, "proj_24", "Most recent must be at index 0 (MRU)");

    // 2. AnalysisHistory ring buffer overflow
    let mut history = analysis::AnalysisHistory::new(5);
    for i in 0..100 {
        history.record(analysis::AnalysisEntry {
            id: i,
            timestamp: "2026-11-09T20:00:00Z".to_string(),
            filename: format!("sample_{}.py", i),
            input_hash: "in".to_string(),
            output_hash: "out".to_string(),
            strings_decoded: i as usize,
            constants_folded: 0,
            variables_renamed: 0,
            dead_branches_pruned: 0,
            security_alerts: 0,
            is_safe: true,
            status_badge: "SEGURO".to_string(),
        });
    }

    assert_eq!(history.entries.len(), 5, "AnalysisHistory must strictly maintain max_entries limit");
    assert_eq!(history.entries[0].id, 99, "Latest entry must be at index 0");

    // 3. compute_diff_metrics stress with large inputs
    let orig = "line 1\nline 2\nline 3\nline 4\n";
    let trans = "line 1\nline 2 modified\nline 3\nline 4\nline 5\n";
    let diff = analysis::compute_diff_metrics(orig, trans);
    assert_eq!(diff.total_lines_original, 4);
    assert_eq!(diff.total_lines_transformed, 5);
    assert_eq!(diff.changed_lines, 2); // 1 modified + 1 added

    let _ = std_fs::remove_dir_all(&temp_dir);
}
