use slint::{Color, ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::collections::HashSet;
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Mutex;

use crate::analysis::{
    AnalysisVerdictCase, PhaseStatus, ProjectAnalysisPipeline, ProjectAnalysisReport,
};
use crate::ast::{
    build_line_numbers_text, build_syntax_lines_with_active, parse_python_ast_flat,
};
use crate::config::session::{calculate_adjacent_active_tab_index, save_session, SessionState};
use crate::core::ai_chat::build_semantic_clean_code;
use crate::core::pipeline::{DeobfuscationConfig, DeobfuscatorEngine};
use crate::fs::{
    build_visible_tree, insert_newline_after, replace_single_line, toggle_folder_expanded,
};
use crate::ui::async_worker::{
    execute_ai_query_async, execute_batch_project_deobfuscate_async, execute_project_analysis_async,
};
use crate::{
    AnalysisPhaseItem, AppWindow, AstVisualNode, ChatMessage, EditorTabItem, HistoryUiItem,
    PaletteCommandItem, SyntaxLine, WorkspaceFileItem,
};
use std::sync::atomic::{AtomicI32, Ordering};

static HISTORY_COUNTER: AtomicI32 = AtomicI32::new(1);
static EXPANDED_DIRS: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);
static LAST_ANALYSIS_REPORT: Mutex<Option<ProjectAnalysisReport>> = Mutex::new(None);


const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Copies text to Windows system clipboard via PowerShell without window flicker.
pub fn copy_to_clipboard(text: &str) {
    if text.trim().is_empty() {
        return;
    }
    let temp_path = std::env::temp_dir().join("kronos_clip_out.txt");
    if fs::write(&temp_path, text).is_ok() {
        let _ = std::process::Command::new("powershell")
            .creation_flags(CREATE_NO_WINDOW)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!(
                    "[System.IO.File]::ReadAllText('{}', [System.Text.Encoding]::UTF8) | Set-Clipboard",
                    temp_path.display()
                ),
            ])
            .spawn();
    }
}

/// Retrieves text from Windows system clipboard.
pub fn paste_from_clipboard() -> Option<String> {
    if let Ok(output) = std::process::Command::new("powershell")
        .creation_flags(CREATE_NO_WINDOW)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Get-Clipboard -Raw",
        ])
        .output()
    {
        let pasted = String::from_utf8_lossy(&output.stdout).to_string();
        if !pasted.trim().is_empty() {
            return Some(pasted);
        }
    }
    None
}

/// Ejecuta el motor de desofuscación y sincroniza la interfaz estilo VS Code.
pub fn sync_ui_state(app: &AppWindow, rebuild_input_surface: bool) {
    let input = app.get_input_code().to_string();
    let hide_comments = app.get_opt_hide_comments();
    let search_query = app.get_search_query().to_string();
    let input_active_line = app.get_input_editing_line();
    let output_active_line = app.get_output_editing_line();

    app.set_input_line_numbers(build_line_numbers_text(&input));

    if rebuild_input_surface {
        app.set_input_lines(build_syntax_lines_with_active(
            &input,
            false,
            hide_comments,
            &search_query,
            input_active_line,
        ));
    }

    if input.trim().is_empty() {
        let placeholder = "# Buffer vacío — Escribe código directamente en el editor";
        app.set_output_code(SharedString::from(placeholder));
        app.set_output_lines(build_syntax_lines_with_active(
            placeholder,
            true,
            false,
            "",
            output_active_line,
        ));
        app.set_output_line_numbers("1".into());
        app.set_stat_strings_decoded(0);
        app.set_stat_constants_folded(0);
        app.set_stat_vars_renamed(0);
        app.set_stat_dead_branches(0);
        app.set_security_alerts_count(0);
        app.set_security_is_safe(true);
        app.set_security_badge_text("0 alerts".into());
        app.set_console_log("[READY] Buffer vacío listo para edición directa.\n".into());
        app.set_security_report("Sin alertas de seguridad.".into());
        app.set_tokens_preview("Sin símbolos en el buffer.".into());
        app.set_diff_preview("".into());
        app.set_ast_nodes(ModelRc::from(Rc::new(VecModel::from(Vec::<AstVisualNode>::new()))));
        app.set_status_text("Editor listo para escribir.".into());
        return;
    }

    let config = DeobfuscationConfig {
        normalize_identifiers: app.get_opt_normalize_vars(),
        prune_dead_code: app.get_opt_remove_dead_code(),
        static_decoders: app.get_opt_decode_strings(),
    };

    let res = DeobfuscatorEngine::process(&input, &config);

    // Poblar estructura visual AST
    let flat_ast = parse_python_ast_flat(&input);
    let ast_ui_nodes: Vec<AstVisualNode> = flat_ast
        .into_iter()
        .map(|n| AstVisualNode {
            id: n.id,
            node_type: SharedString::from(n.node_type),
            name: SharedString::from(n.name),
            detail: SharedString::from(n.detail),
            line_number: n.line_number,
            depth: n.depth,
            is_alert: n.is_alert,
        })
        .collect();
    app.set_ast_nodes(ModelRc::from(Rc::new(VecModel::from(ast_ui_nodes))));

    app.set_output_lines(build_syntax_lines_with_active(
        &res.code,
        true,
        hide_comments,
        &search_query,
        output_active_line,
    ));
    app.set_output_code(SharedString::from(&res.code));
    app.set_output_line_numbers(build_line_numbers_text(&res.code));

    // Estadísticas
    app.set_stat_strings_decoded(res.strings_decoded as i32);
    app.set_stat_constants_folded(res.constants_folded as i32);
    app.set_stat_vars_renamed(res.variables_renamed as i32);
    app.set_stat_dead_branches(res.dead_branches_pruned as i32);

    // Seguridad y Guardarraíles
    app.set_security_alerts_count(res.security_audit.alerts_count as i32);
    app.set_security_is_safe(res.security_audit.is_safe);
    app.set_security_badge_text(SharedString::from(&res.security_audit.status_badge));
    app.set_input_hash_short(SharedString::from(&res.security_audit.input_hash));
    app.set_output_hash_short(SharedString::from(&res.security_audit.output_hash));

    // Paneles inferiores
    app.set_console_log(SharedString::from(&res.audit_log));
    app.set_security_report(SharedString::from(&res.security_audit.report));
    app.set_tokens_preview(SharedString::from(&res.tokens_breakdown));
    app.set_diff_preview(SharedString::from(&res.diff));

    let total_ops = res.strings_decoded
        + res.constants_folded
        + res.variables_renamed
        + res.dead_branches_pruned;

    // Registrar en el historial de análisis si es una reconstrucción completa
    if rebuild_input_surface {
        let hist_model = app.get_history_items();
        let mut hist_vec: Vec<HistoryUiItem> = (0..hist_model.row_count())
            .filter_map(|i| hist_model.row_data(i))
            .collect();
        let id = HISTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
        let active_fname = app.get_active_filename().to_string();
        let fname = if active_fname.trim().is_empty() {
            "buffer.py".to_string()
        } else {
            active_fname
        };
        hist_vec.insert(
            0,
            HistoryUiItem {
                id,
                timestamp: SharedString::from(format!("#{:02}", id)),
                filename: SharedString::from(fname),
                ops_summary: SharedString::from(format!(
                    "{} ops (str:{}, var:{}, dead:{})",
                    total_ops, res.strings_decoded, res.variables_renamed, res.dead_branches_pruned
                )),
                status_badge: SharedString::from(&res.security_audit.status_badge),
                is_safe: res.security_audit.is_safe,
            },
        );
        if hist_vec.len() > 40 {
            hist_vec.truncate(40);
        }
        app.set_history_items(ModelRc::from(Rc::new(VecModel::from(hist_vec))));
    }

    app.set_status_text(SharedString::from(format!(
        "Pipeline completado ({} transformaciones) — {}",
        total_ops, res.security_audit.status_badge
    )));
}

/// Persiste el estado actual del espacio de trabajo en `session.json`.
pub fn persist_current_session(app: &AppWindow) {
    let folder_path = app.get_workspace_folder_path().to_string();
    let folder_name = app.get_workspace_folder_name().to_string();
    let state = SessionState {
        sidebar_visible: app.get_sidebar_visible(),
        context_panel_width: app.get_context_panel_width(),
        ai_panel_visible: app.get_ai_chat_visible(),
        ai_panel_width: app.get_ai_panel_width(),
        terminal_visible: app.get_bottom_panel_visible(),
        terminal_height: app.get_bottom_panel_height(),
        split_view_enabled: app.get_split_view_enabled(),
        active_model: app.get_selected_model().to_string(),
        active_mode: app.get_selected_ai_mode().to_string(),
        gemini_api_key: app.get_gemini_api_key().to_string(),
        opt_normalize_vars: app.get_opt_normalize_vars(),
        opt_remove_dead_code: app.get_opt_remove_dead_code(),
        opt_decode_strings: app.get_opt_decode_strings(),
        opt_hide_comments: app.get_opt_hide_comments(),
        workspace_folder_path: (!folder_path.trim().is_empty()).then_some(folder_path),
        workspace_folder_name: (!folder_name.trim().is_empty()).then_some(folder_name),
        ..Default::default()
    };
    let _ = save_session(&state);
}

pub fn execute_and_sync_ui(app: &AppWindow) {
    sync_ui_state(app, true);
}

/// Añade un mensaje al historial del Chat de Gemini AI en la interfaz separando texto explicativo y bloque de código
pub fn append_chat_message(app: &AppWindow, sender: &str, role: i32, raw_text: &str, timestamp: &str) {
    let current_model = app.get_chat_messages();
    let mut msgs: Vec<ChatMessage> = (0..current_model.row_count())
        .filter_map(|i| current_model.row_data(i))
        .collect();

    let (clean_text, has_code, code_snippet, code_lang) = if role == 1 {
        // Mensajes del operador: texto plano sin split
        (raw_text.to_string(), false, String::new(), String::new())
    } else if let Some(first_tick) = raw_text.find("```") {
        let text_part = raw_text[..first_tick].trim().to_string();
        let after_ticks = &raw_text[first_tick + 3..];
        let line_end = after_ticks.find('\n').unwrap_or(after_ticks.len());
        let lang = after_ticks[..line_end].trim().to_uppercase();
        let code_start = first_tick + 3 + line_end + if line_end < after_ticks.len() { 1 } else { 0 };

        if let Some(close_tick) = raw_text[code_start..].find("```") {
            let code = raw_text[code_start..code_start + close_tick].trim().to_string();
            let remainder = raw_text[code_start + close_tick + 3..].trim();
            let final_text = if !remainder.is_empty() {
                if text_part.is_empty() {
                    remainder.to_string()
                } else {
                    format!("{}\n\n{}", text_part, remainder)
                }
            } else if !text_part.is_empty() {
                text_part
            } else {
                "Propuesta de desofuscación y reconstrucción de código:".to_string()
            };
            (final_text, true, code, if lang.is_empty() { "CODE".to_string() } else { lang })
        } else {
            let code = raw_text[code_start..].trim().to_string();
            (if text_part.is_empty() { "Código sugerido:".to_string() } else { text_part }, true, code, if lang.is_empty() { "CODE".to_string() } else { lang })
        }
    } else {
        (raw_text.to_string(), false, String::new(), String::new())
    };

    msgs.push(ChatMessage {
        sender: SharedString::from(sender),
        role,
        text: SharedString::from(clean_text),
        has_code,
        code_snippet: SharedString::from(code_snippet),
        code_lang: SharedString::from(code_lang),
        timestamp: SharedString::from(timestamp),
    });
    app.set_chat_messages(ModelRc::from(Rc::new(VecModel::from(msgs))));
}

/// Asegura que una pestaña esté presente en `open_tabs` y opcionalmente la selecciona.
pub fn ensure_tab_open(app: &AppWindow, tab_type: i32, title: &str, select: bool) {
    let current_model = app.get_open_tabs();
    let mut tabs: Vec<EditorTabItem> = (0..current_model.row_count())
        .filter_map(|i| current_model.row_data(i))
        .collect();

    let target_idx = if let Some(existing_idx) = tabs.iter().position(|t| t.tab_type == tab_type) {
        tabs[existing_idx].title = SharedString::from(title);
        existing_idx
    } else {
        let dot_color = if tab_type == 0 {
            Color::from_rgb_u8(0x51, 0x9A, 0xBA)
        } else {
            Color::from_rgb_u8(0x4E, 0xC9, 0xB0)
        };
        tabs.push(EditorTabItem {
            id: tab_type,
            title: SharedString::from(title),
            tab_type,
            dot_color,
        });
        tabs.len() - 1
    };

    app.set_open_tabs(ModelRc::from(Rc::new(VecModel::from(tabs))));
    app.set_has_open_file(true);
    if select {
        app.set_active_tab_index(target_idx as i32);
        app.set_active_editor_tab(tab_type);
    }
}

/// Actualiza el título dinámico de la ventana de KRONOS Descode estilo VS Code
pub fn update_window_title(app: &AppWindow) {
    let folder = app.get_workspace_folder_name().to_string();
    let file = app.get_active_filename().to_string();
    let has_file = app.get_has_open_file() && !file.trim().is_empty();

    let title = match (!folder.is_empty(), has_file) {
        (true, true) => format!("{} - {} - KRONOS Descode", file, folder),
        (true, false) => format!("{} - KRONOS Descode", folder),
        (false, true) => format!("{} - KRONOS Descode", file),
        (false, false) => "KRONOS Descode".to_string(),
    };
    app.set_window_title(SharedString::from(title));
}

#[derive(Clone, Copy)]
pub struct CommandDef {
    pub id: i32,
    pub label: &'static str,
    pub shortcut: &'static str,
}

pub const ALL_COMMANDS: &[CommandDef] = &[
    CommandDef { id: 0, label: "Pipeline: Ejecutar Desofuscación AST Estática", shortcut: "F5" },
    CommandDef { id: 1, label: "Archivo: Nuevo Script Editable", shortcut: "Ctrl+N" },
    CommandDef { id: 2, label: "Archivo: Abrir Archivo...", shortcut: "Ctrl+O" },
    CommandDef { id: 3, label: "Archivo: Abrir Carpeta de Proyecto...", shortcut: "Ctrl+K Ctrl+O" },
    CommandDef { id: 4, label: "Archivo: Guardar Buffer Activo", shortcut: "Ctrl+S" },
    CommandDef { id: 5, label: "Archivo: Pegar y Desofuscar Portapapeles", shortcut: "Ctrl+V" },
    CommandDef { id: 6, label: "Edición: Alternar Modo Edición Directa", shortcut: "Ctrl+E" },
    CommandDef { id: 7, label: "Vista: Editor de Código (Principal)", shortcut: "Esc" },
    CommandDef { id: 8, label: "Vista: Explorador Visual AST Interactivo", shortcut: "AST" },
    CommandDef { id: 9, label: "Vista: Comparador Code Diff (Lado a Lado)", shortcut: "DIFF" },
    CommandDef { id: 10, label: "Vista: Alternar Vista Dividida (Split View)", shortcut: "Ctrl+\\" },
    CommandDef { id: 11, label: "Vista: Alternar Barra Lateral Izquierda", shortcut: "Ctrl+B" },
    CommandDef { id: 12, label: "Vista: Alternar Consola Inferior", shortcut: "Ctrl+J" },
    CommandDef { id: 13, label: "Vista: Alternar Asistente KRONOS AI", shortcut: "Ctrl+L" },
    CommandDef { id: 14, label: "KRONOS AI: Explicar Archivo Activo", shortcut: "AI" },
    CommandDef { id: 15, label: "KRONOS AI: Identificar Técnicas de Ofuscación", shortcut: "AI" },
    CommandDef { id: 16, label: "KRONOS AI: Proponer Reconstrucción Limpia", shortcut: "AI" },
    CommandDef { id: 17, label: "KRONOS AI: Limpiar Historial de Conversación", shortcut: "AI" },
];

pub fn update_filtered_commands(app: &AppWindow, query: &str) {
    let q = query.trim().to_lowercase();
    let items: Vec<PaletteCommandItem> = ALL_COMMANDS
        .iter()
        .filter(|cmd| q.is_empty() || cmd.label.to_lowercase().contains(&q) || cmd.shortcut.to_lowercase().contains(&q))
        .map(|cmd| PaletteCommandItem {
            id: cmd.id,
            label: SharedString::from(cmd.label),
            shortcut: SharedString::from(cmd.shortcut),
        })
        .collect();
    app.set_palette_commands(ModelRc::from(Rc::new(VecModel::from(items))));
}

/// Registers all Slint event callbacks on the AppWindow.
pub fn register_all_callbacks(app: &AppWindow) {
    // 1. Callback: Ejecutar Desofuscación (F5 o botón Play ▷)
    {
        let app_weak = app.as_weak();
        app.on_run_deobfuscation(move || {
            if let Some(app) = app_weak.upgrade() {
                ensure_tab_open(&app, 1, "ast_reconstruido.py", true);
                execute_and_sync_ui(&app);
            }
        });
    }

    // 2. Callback: Cerrar pestaña del editor
    {
        let app_weak = app.as_weak();
        app.on_close_editor_tab(move |close_idx_raw| {
            if let Some(app) = app_weak.upgrade() {
                let close_idx = close_idx_raw as usize;
                let current_model = app.get_open_tabs();
                let mut tabs: Vec<EditorTabItem> = (0..current_model.row_count())
                    .filter_map(|i| current_model.row_data(i))
                    .collect();

                if close_idx >= tabs.len() {
                    return;
                }

                let removed_tab = tabs.remove(close_idx);

                if tabs.is_empty() {
                    app.set_open_tabs(ModelRc::from(Rc::new(VecModel::from(Vec::<EditorTabItem>::new()))));
                    app.set_active_tab_index(-1);
                    app.set_active_editor_tab(-1);
                    app.set_has_open_file(false);
                    app.set_split_view_enabled(false);
                    app.set_active_filename("".into());
                    app.set_active_sample_index(-1);
                    app.set_input_code("".into());
                    app.set_output_code("".into());
                    app.set_input_lines(ModelRc::from(Rc::new(VecModel::from(Vec::<SyntaxLine>::new()))));
                    app.set_output_lines(ModelRc::from(Rc::new(VecModel::from(Vec::<SyntaxLine>::new()))));
                    app.set_input_editing_line(-1);
                    app.set_output_editing_line(-1);
                    update_window_title(&app);
                    app.set_status_text("Todos los editores cerrados.".into());
                    return;
                }

                let cur_active = app.get_active_tab_index().max(0) as usize;
                let new_active = calculate_adjacent_active_tab_index(close_idx, cur_active, tabs.len())
                    .unwrap_or(0);

                let active_tab_type = tabs[new_active].tab_type;
                let active_title = tabs[new_active].title.clone();
                app.set_active_editor_tab(active_tab_type);
                app.set_active_tab_index(new_active as i32);
                app.set_open_tabs(ModelRc::from(Rc::new(VecModel::from(tabs))));

                if active_tab_type == 0 {
                    app.set_active_filename(active_title);
                } else {
                    app.set_active_filename("ast_reconstruido.py".into());
                }
                update_window_title(&app);

                if app.get_open_tabs().row_count() <= 1 {
                    app.set_split_view_enabled(false);
                }

                app.set_status_text(SharedString::from(format!(
                    "Pestaña '{}' cerrada.",
                    removed_tab.title
                )));
            }
        });
    }

    // 3. Callback: Seleccionar pestaña del editor
    {
        let app_weak = app.as_weak();
        app.on_select_editor_tab(move |sel_idx_raw| {
            if let Some(app) = app_weak.upgrade() {
                let sel_idx = sel_idx_raw as usize;
                let current_model = app.get_open_tabs();
                if sel_idx < current_model.row_count() {
                    if let Some(tab) = current_model.row_data(sel_idx) {
                        app.set_active_tab_index(sel_idx as i32);
                        app.set_active_editor_tab(tab.tab_type);
                        app.set_input_editing_line(-1);
                        app.set_output_editing_line(-1);
                        if tab.tab_type == 0 {
                            app.set_active_filename(tab.title.clone());
                        } else {
                            app.set_active_filename("ast_reconstruido.py".into());
                        }
                        update_window_title(&app);
                    }
                }
            }
        });
    }

    // 4. Callback: Seleccionar otra línea de entrada
    {
        let app_weak = app.as_weak();
        app.on_select_input_line(move |line_idx| {
            if let Some(app) = app_weak.upgrade() {
                app.set_input_editing_line(line_idx);
                let input = app.get_input_code().to_string();
                let hide_comments = app.get_opt_hide_comments();
                let query = app.get_search_query().to_string();
                app.set_input_lines(build_syntax_lines_with_active(
                    &input,
                    false,
                    hide_comments,
                    &query,
                    line_idx,
                ));
            }
        });
    }

    // 5. Callback: Edición directa de una línea individual en la pestaña de entrada
    {
        let app_weak = app.as_weak();
        app.on_edit_input_line(move |line_idx, new_txt| {
            if let Some(app) = app_weak.upgrade() {
                let current = app.get_input_code().to_string();
                let updated = replace_single_line(&current, line_idx.max(0) as usize, &new_txt);
                app.set_input_code(SharedString::from(&updated));
                sync_ui_state(&app, false);
            }
        });
    }

    // 6. Callback: Pulsar Enter en una línea de entrada
    {
        let app_weak = app.as_weak();
        app.on_insert_input_newline(move |line_idx, curr_txt| {
            if let Some(app) = app_weak.upgrade() {
                let current = app.get_input_code().to_string();
                let next_idx = line_idx.max(0) + 1;
                let updated = insert_newline_after(&current, line_idx.max(0) as usize, &curr_txt);
                app.set_input_code(SharedString::from(&updated));
                app.set_input_editing_line(next_idx);
                execute_and_sync_ui(&app);
            }
        });
    }

    // 7. Callback: Seleccionar otra línea de salida AST
    {
        let app_weak = app.as_weak();
        app.on_select_output_line(move |line_idx| {
            if let Some(app) = app_weak.upgrade() {
                app.set_output_editing_line(line_idx);
                let current = app.get_output_code().to_string();
                let hide_comments = app.get_opt_hide_comments();
                let query = app.get_search_query().to_string();
                app.set_output_lines(build_syntax_lines_with_active(
                    &current,
                    true,
                    hide_comments,
                    &query,
                    line_idx,
                ));
            }
        });
    }

    // 8. Callback: Edición directa de una línea individual en la pestaña de salida AST
    {
        let app_weak = app.as_weak();
        app.on_edit_output_line(move |line_idx, new_txt| {
            if let Some(app) = app_weak.upgrade() {
                let current = app.get_output_code().to_string();
                let updated = replace_single_line(&current, line_idx.max(0) as usize, &new_txt);
                app.set_output_code(SharedString::from(&updated));
                app.set_output_line_numbers(build_line_numbers_text(&updated));
                app.set_status_text("Salida AST editada manualmente.".into());
            }
        });
    }

    // 9. Callback: Pulsar Enter en una línea de salida AST
    {
        let app_weak = app.as_weak();
        app.on_insert_output_newline(move |line_idx, curr_txt| {
            if let Some(app) = app_weak.upgrade() {
                let current = app.get_output_code().to_string();
                let next_idx = line_idx.max(0) + 1;
                let updated = insert_newline_after(&current, line_idx.max(0) as usize, &curr_txt);
                let hide_comments = app.get_opt_hide_comments();
                let query = app.get_search_query().to_string();
                app.set_output_code(SharedString::from(&updated));
                app.set_output_editing_line(next_idx);
                app.set_output_line_numbers(build_line_numbers_text(&updated));
                app.set_output_lines(build_syntax_lines_with_active(
                    &updated,
                    true,
                    hide_comments,
                    &query,
                    next_idx,
                ));
            }
        });
    }

    // 10. Callback: Cambio en el Editor Multilínea Completo (Entrada)
    {
        let app_weak = app.as_weak();
        app.on_input_code_changed(move |new_code| {
            if let Some(app) = app_weak.upgrade() {
                app.set_input_code(new_code);
                execute_and_sync_ui(&app);
            }
        });
    }

    // 11. Callback: Cambio en el Editor Multilínea Completo (Salida)
    {
        let app_weak = app.as_weak();
        app.on_output_code_changed(move |new_code| {
            if let Some(app) = app_weak.upgrade() {
                let s = new_code.to_string();
                let hide_comments = app.get_opt_hide_comments();
                let query = app.get_search_query().to_string();
                app.set_output_code(new_code);
                app.set_output_line_numbers(build_line_numbers_text(&s));
                app.set_output_lines(build_syntax_lines_with_active(
                    &s,
                    true,
                    hide_comments,
                    &query,
                    app.get_output_editing_line(),
                ));
                app.set_status_text("Editando código AST reconstruido.".into());
            }
        });
    }

    // 12. Callback: Cambio en la barra de búsqueda / filtro superior
    {
        let app_weak = app.as_weak();
        app.on_search_query_changed(move |_query| {
            if let Some(app) = app_weak.upgrade() {
                app.set_input_editing_line(-1);
                app.set_output_editing_line(-1);
                execute_and_sync_ui(&app);
            }
        });
    }

    // 13. Callback: Crear nuevo archivo en blanco editable
    {
        let app_weak = app.as_weak();
        app.on_new_blank_file(move || {
            if let Some(app) = app_weak.upgrade() {
                let template = "# Escribe tu código Python aquí (haz clic en cualquier línea o usa el modo Editor Completo)\nmensaje = \"Hola desde KRONOS AST\"\nprint(mensaje)\n";
                app.set_active_filename("nuevo_script_editable.py".into());
                app.set_active_sample_index(-2);
                app.set_input_code(SharedString::from(template));
                ensure_tab_open(&app, 0, "nuevo_script_editable.py", true);
                ensure_tab_open(&app, 1, "ast_reconstruido.py", false);
                app.set_active_tab_index(0);
                app.set_active_editor_tab(0);
                app.set_edit_mode(true);
                app.set_input_editing_line(-1);
                app.set_output_editing_line(-1);
                execute_and_sync_ui(&app);
                update_window_title(&app);
                app.set_status_text(
                    "Nuevo archivo editable creado — Puedes escribir libremente.".into(),
                );
            }
        });
    }

    // 14. Callback: Seleccionar modelo de Gemini
    {
        let app_weak = app.as_weak();
        app.on_select_model(move |model_name| {
            if let Some(app) = app_weak.upgrade() {
                app.set_selected_model(model_name.clone());
                persist_current_session(&app);
                app.set_status_text(SharedString::from(format!(
                    "Modelo KRONOS AI: {}",
                    model_name
                )));
            }
        });
    }

    // 14b. Callback: Seleccionar modo de trabajo de KRONOS AI
    {
        let app_weak = app.as_weak();
        app.on_select_ai_mode(move |mode_name| {
            if let Some(app) = app_weak.upgrade() {
                app.set_selected_ai_mode(mode_name.clone());
                persist_current_session(&app);
                app.set_status_text(SharedString::from(format!(
                    "Modo KRONOS AI: {}",
                    mode_name
                )));
            }
        });
    }

    // 14c. Callback: Persistir estado de la sesión actual
    {
        let app_weak = app.as_weak();
        app.on_save_session_state(move || {
            if let Some(app) = app_weak.upgrade() {
                persist_current_session(&app);
            }
        });
    }

    // 14d. Callback: Ejecutar acción contextual de KRONOS AI desde el editor
    {
        let app_weak = app.as_weak();
        app.on_trigger_contextual_ai_action(move |action_key| {
            if let Some(app) = app_weak.upgrade() {
                let key = action_key.as_str();
                let line_idx = app.get_input_editing_line();
                let input_code = app.get_input_code().to_string();
                let selected_line_snippet = if line_idx >= 0 {
                    input_code
                        .lines()
                        .nth(line_idx as usize)
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                } else {
                    None
                };

                let prompt = match (key, selected_line_snippet) {
                    ("explain", Some(line_txt)) => format!(
                        "Explica paso a paso el propósito y comportamiento de esta línea seleccionada (Línea {}): `{}` dentro del contexto del archivo.",
                        line_idx + 1,
                        line_txt
                    ),
                    ("explain", None) => {
                        "Explicar codigo activo paso a paso, detallando flujo de control, funciones y variables clave.".to_string()
                    }
                    ("detect_obfuscation", _) => {
                        "Identifica todas las técnicas de ofuscación presentes (codificación Base64/Hex/XOR, predicados opacos, renombrado hexadecimal) y evalúa riesgos.".to_string()
                    }
                    ("interpret_ast", _) => {
                        "Interpreta la estructura AST del archivo actual, jerarquía de nodos, asignaciones críticas y posibles sinks de ejecución.".to_string()
                    }
                    ("reconstruct", _) => {
                        "Desofuscar y limpiar codigo con IA proponiendo una reconstrucción idiomática, legible y segura.".to_string()
                    }
                    ("analyze_file", _) => {
                        "Auditar vulnerabilidades, sinks de seguridad y realizar un análisis integral de ingeniería inversa del archivo activo.".to_string()
                    }
                    _ => "Analizar el archivo activo en profundidad.".to_string(),
                };

                app.invoke_send_chat_prompt(SharedString::from(prompt));
            }
        });
    }

    // 15. Callback: Enviar consulta al Chat de Gemini AI (asíncrono sin bloquear la UI)
    {
        let app_weak = app.as_weak();
        app.on_send_chat_prompt(move |prompt_shared| {
            if let Some(app) = app_weak.upgrade() {
                let prompt = prompt_shared.to_string();
                if prompt.trim().is_empty() {
                    return;
                }

                app.set_chat_input_text("".into());
                app.set_ai_chat_visible(true);
                append_chat_message(&app, "TÚ", 1, &prompt, "ahora");

                let prompt_lower = prompt.to_lowercase();
                let is_project_batch = prompt_lower.contains("todo el proyecto")
                    || (prompt_lower.contains("desofusc") && prompt_lower.contains("proyecto"))
                    || (prompt_lower.contains("des ofusc") && prompt_lower.contains("proyecto"))
                    || (prompt_lower.contains("limpi") && prompt_lower.contains("proyecto"))
                    || prompt_lower.contains("desofuscar todo")
                    || prompt_lower.contains("des ofusca todo")
                    || prompt_lower.contains("desofusca todo")
                    || prompt_lower.contains("deobfuscate all")
                    || prompt_lower.contains("deobfuscate project");

                if is_project_batch {
                    let project_dir_str = app.get_workspace_folder_path().to_string();
                    let proj_path = if !project_dir_str.trim().is_empty() {
                        Some(PathBuf::from(project_dir_str))
                    } else if let Ok(guard) = LAST_ANALYSIS_REPORT.lock() {
                        guard.as_ref().map(|r| PathBuf::from(&r.project_path))
                    } else {
                        None
                    };

                    if let Some(target_dir) = proj_path {
                        if target_dir.exists() && target_dir.is_dir() {
                            app.set_ai_busy(true);
                            app.set_ai_status_badge("Desofuscando proyecto en local...".into());
                            app.set_status_text(SharedString::from(format!(
                                "Iniciando desofuscación por lotes de todo el proyecto en {}",
                                target_dir.display()
                            )));

                            let app_weak_bg = app.as_weak();
                            let dir_clone = target_dir.clone();
                            execute_batch_project_deobfuscate_async(target_dir, move |res| {
                                if let Some(app) = app_weak_bg.upgrade() {
                                    app.set_ai_busy(false);
                                    match res {
                                        Ok(report) => {
                                            app.set_ai_status_badge("Completado (Local Zero-Exec)".into());
                                            if !report.primary_filename.is_empty() && !report.primary_content.is_empty() {
                                                app.set_has_open_file(true);
                                                app.set_active_filename(SharedString::from(&report.primary_filename));
                                                app.set_input_code(SharedString::from(&report.primary_content));
                                                app.set_input_line_numbers(build_line_numbers_text(&report.primary_content));
                                                let hide_comments = app.get_opt_hide_comments();
                                                let query = app.get_search_query().to_string();
                                                app.set_input_lines(build_syntax_lines_with_active(&report.primary_content, false, hide_comments, &query, -1));
                                                ensure_tab_open(&app, 0, &report.primary_filename, true);
                                                ensure_tab_open(&app, 1, "ast_reconstruido.py", false);
                                                app.set_active_tab_index(0);
                                                app.set_active_editor_tab(0);
                                                execute_and_sync_ui(&app);
                                                app.set_last_ai_code_suggestion(SharedString::from(&report.primary_content));
                                            }

                                            // Refrescar el árbol de archivos del workspace para mostrar la carpeta 'desofuscado'
                                            if let Ok(mut set_guard) = EXPANDED_DIRS.lock() {
                                                let set = set_guard.get_or_insert_with(HashSet::new);
                                                let target_p = std::path::Path::new(&report.primary_filename);
                                                let visible = build_visible_tree(&dir_clone, set, Some(target_p));
                                                app.set_workspace_files(ModelRc::from(Rc::new(VecModel::from(visible))));
                                            }

                                            let summary = format!(
                                                "⚡ **DESOFUSCACIÓN COMPLETA DEL PROYECTO EJECUTADA (Motor Local Zero-Exec)**\n\n\
                                                Se ha procesado y desofuscado la totalidad del proyecto de forma determinista y segura en Rust sin dependencias de red externas:\n\n\
                                                - **Archivos escaneados**: {}\n\
                                                - **Archivos desofuscados**: {}\n\
                                                - **Constantes opacas Allatori plegadas**: {}\n\
                                                - **Comentarios y artefactos de bytecode purgados**: {}\n\
                                                - **Directorio de salida limpio**: `{}`\n\n\
                                                El archivo principal (`{}`) ha sido cargado directamente en el editor. Todas las clases desofuscadas conservan la estructura de paquetes original en la carpeta `desofuscado/`.",
                                                report.total_scanned,
                                                report.files_deobfuscated,
                                                report.constants_folded,
                                                report.comments_purged,
                                                report.output_dir,
                                                report.primary_filename
                                            );
                                            append_chat_message(&app, "KRONOS AI", 0, &summary, "ahora");
                                            app.set_status_text(SharedString::from(format!(
                                                "Desofuscación completa: {} archivos procesados en {}",
                                                report.files_deobfuscated, report.output_dir
                                            )));
                                        }
                                        Err(err) => {
                                            app.set_ai_status_badge("Error en desofuscación".into());
                                            append_chat_message(
                                                &app,
                                                "KRONOS AI",
                                                0,
                                                &format!("⚠️ Error al desofuscar el proyecto en local: {}", err),
                                                "ahora",
                                            );
                                        }
                                    }
                                }
                            });
                            return;
                        }
                    }

                    app.set_ai_status_badge("Sin proyecto abierto".into());
                    append_chat_message(
                        &app,
                        "KRONOS AI",
                        0,
                        "⚠️ No hay ninguna carpeta de proyecto abierta actualmente. Abra una carpeta de proyecto desde el explorador lateral o el Command Palette (Ctrl+P) para desofuscar el proyecto completo de forma automática.",
                        "ahora",
                    );
                    return;
                }

                app.set_ai_busy(true);
                app.set_ai_status_badge("Consultando Gemini...".into());

                let api_key = app.get_gemini_api_key().to_string();
                let selected_model = app.get_selected_model().to_string();
                let filename = app.get_active_filename().to_string();
                let mut input_code = app.get_input_code().to_string();
                let mut target_filename = filename.clone();

                if input_code.trim().is_empty() {
                    let out_fallback = app.get_output_code().to_string();
                    if !out_fallback.trim().is_empty()
                        && !out_fallback.starts_with("# Buffer vacío")
                    {
                        input_code = out_fallback;
                    } else if let Ok(guard) = LAST_ANALYSIS_REPORT.lock() {
                        if let Some(report) = guard.as_ref() {
                            // Si no hay archivo abierto en el editor, cargamos el primer archivo ofuscado detectado en el proyecto
                            if let Some(first_obf) = report.obfuscation_findings.first() {
                                let full_path = std::path::Path::new(&report.project_path).join(&first_obf.file_path);
                                if let Ok(bytes) = std::fs::read(&full_path) {
                                    let content = String::from_utf8_lossy(&bytes).to_string();
                                    input_code = content.clone();
                                    target_filename = first_obf.file_path.clone();
                                    app.set_has_open_file(true);
                                    app.set_active_filename(SharedString::from(&target_filename));
                                    app.set_input_code(SharedString::from(&content));
                                    app.set_input_line_numbers(build_line_numbers_text(&content));
                                    let hide_comments = app.get_opt_hide_comments();
                                    let query = app.get_search_query().to_string();
                                    app.set_input_lines(build_syntax_lines_with_active(&content, false, hide_comments, &query, -1));
                                    ensure_tab_open(&app, 0, &target_filename, true);
                                    app.set_status_text(SharedString::from(format!(
                                        "Cargado archivo ofuscado para análisis: {}",
                                        target_filename
                                    )));
                                }
                            }
                        }
                    }

                    if input_code.trim().is_empty() {
                        let folder_str = app.get_workspace_folder_path().to_string();
                        if !folder_str.trim().is_empty() {
                            let folder_p = PathBuf::from(&folder_str);
                            if folder_p.exists() {
                                let mut queue = vec![folder_p.clone()];
                                'find_file: while let Some(cur) = queue.pop() {
                                    if let Ok(entries) = std::fs::read_dir(&cur) {
                                        for entry in entries.flatten() {
                                            let ep = entry.path();
                                            if ep.is_dir() {
                                                let dname = ep.file_name().and_then(|n| n.to_str()).unwrap_or("");
                                                if dname != ".git" && dname != "target" && dname != "desofuscado" {
                                                    queue.push(ep);
                                                }
                                            } else if ep.is_file() {
                                                let ext = ep.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                                                if ext == "java" || ext == "py" {
                                                    if let Ok(bytes) = std::fs::read(&ep) {
                                                        let content = String::from_utf8_lossy(&bytes).to_string();
                                                        let rel = ep.strip_prefix(&folder_p).unwrap_or(&ep).to_string_lossy().to_string();
                                                        input_code = content.clone();
                                                        target_filename = rel.clone();
                                                        app.set_has_open_file(true);
                                                        app.set_active_filename(SharedString::from(&target_filename));
                                                        app.set_input_code(SharedString::from(&content));
                                                        app.set_input_line_numbers(build_line_numbers_text(&content));
                                                        let hide_comments = app.get_opt_hide_comments();
                                                        let query = app.get_search_query().to_string();
                                                        app.set_input_lines(build_syntax_lines_with_active(&content, false, hide_comments, &query, -1));
                                                        ensure_tab_open(&app, 0, &target_filename, true);
                                                        break 'find_file;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                let mode = app.get_selected_ai_mode().to_string();
                let is_deobf = mode == "Deobfuscate"
                    || prompt.to_lowercase().contains("desofusc")
                    || prompt.to_lowercase().contains("des ofusc")
                    || prompt.to_lowercase().contains("deobfusc")
                    || prompt.to_lowercase().contains("limpi")
                    || prompt.to_lowercase().contains("reconstru");

                let config = DeobfuscationConfig {
                    normalize_identifiers: app.get_opt_normalize_vars(),
                    prune_dead_code: app.get_opt_remove_dead_code(),
                    static_decoders: app.get_opt_decode_strings(),
                };

                let mut full_prompt = prompt.clone();
                if is_deobf {
                    full_prompt.push_str("\n\n[PROTOCOLO DE DESOFUSCACIÓN KRONOS ACTIVO]\n\
                    Instrucciones obligatorias:\n\
                    1. Normalización Léxica: Renombra TODOS los identificadores en alfabeto griego (α, β, γ...) o nombres crípticos a identificadores semánticos en inglés según su función.\n\
                    2. Plegado Aritmético: Resuelve trampas como Integer.reverse(-2147483648) -> 1, Integer.reverse(-1) -> -1, etc.\n\
                    3. Purga de Artefactos: Elimina comentarios de descompilador (/* 20 */, // Byte code:).\n\
                    4. Devuelve la versión final COMPLETA, SINTÁCTICAMENTE VÁLIDA Y SIN TRUNCAR dentro de un bloque Markdown.");
                }

                if let Ok(guard) = LAST_ANALYSIS_REPORT.lock() {
                    if let Some(report) = guard.as_ref() {
                        let project_ctx = report.format_for_ai_prompt();
                        full_prompt = format!("{}\n\n{}", full_prompt, project_ctx);
                    }
                }

                let app_weak_bg = app.as_weak();
                execute_ai_query_async(
                    api_key,
                    selected_model,
                    full_prompt,
                    target_filename,
                    input_code,
                    config,
                    move |ai_res| {
                        if let Some(app) = app_weak_bg.upgrade() {
                            app.set_ai_busy(false);
                            app.set_ai_status_badge(SharedString::from(&ai_res.mode_badge));
                            if let Some(code_sug) = &ai_res.suggested_code {
                                app.set_last_ai_code_suggestion(SharedString::from(code_sug));
                            }
                            append_chat_message(
                                &app,
                                "KRONOS AI",
                                0,
                                &ai_res.reply_text,
                                "ahora",
                            );
                            app.set_status_text(
                                "Respuesta de KRONOS AI recibida.".into(),
                            );
                        }
                    },
                );
            }
        });
    }

    // 16. Callback: Aplicar el código limpio de Gemini AI directamente al editor
    {
        let app_weak = app.as_weak();
        app.on_apply_ai_code_to_editor(move || {
            if let Some(app) = app_weak.upgrade() {
                let mut suggestion = app.get_last_ai_code_suggestion().to_string();
                if suggestion.trim().is_empty() {
                    let input_code = app.get_input_code().to_string();
                    let filename = app.get_active_filename().to_string();
                    if input_code.trim().is_empty() {
                        app.set_status_text("No hay sugerencia de Gemini ni código activo en el editor.".into());
                        return;
                    } else {
                        let cfg = DeobfuscationConfig {
                            normalize_identifiers: app.get_opt_normalize_vars(),
                            prune_dead_code: app.get_opt_remove_dead_code(),
                            static_decoders: app.get_opt_decode_strings(),
                        };
                        let res = DeobfuscatorEngine::process(&input_code, &cfg);
                        suggestion = build_semantic_clean_code(&filename, &res.raw_transformed);
                    }
                    app.set_last_ai_code_suggestion(SharedString::from(&suggestion));
                }

                let active_fname = app.get_active_filename().to_string();
                let tab_title = if active_fname.to_lowercase().ends_with(".java") {
                    let name = std::path::Path::new(&active_fname)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("codigo.java");
                    format!("desofuscado_{}", name)
                } else if active_fname.to_lowercase().ends_with(".py") {
                    let name = std::path::Path::new(&active_fname)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("codigo.py");
                    format!("desofuscado_{}", name)
                } else if !active_fname.is_empty() {
                    let name = std::path::Path::new(&active_fname)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("reconstruido");
                    format!("desofuscado_{}", name)
                } else {
                    "codigo_desofuscado.java".to_string()
                };

                ensure_tab_open(&app, 1, &tab_title, true);
                app.set_output_code(SharedString::from(&suggestion));
                app.set_output_editing_line(-1);
                app.set_active_editor_tab(1);
                update_window_title(&app);

                let hide_comments = app.get_opt_hide_comments();
                let query = app.get_search_query().to_string();
                app.set_output_line_numbers(build_line_numbers_text(&suggestion));
                app.set_output_lines(build_syntax_lines_with_active(
                    &suggestion,
                    true,
                    hide_comments,
                    &query,
                    -1,
                ));
                app.set_status_text("Código sugerido por KRONOS AI aplicado a la pestaña de salida.".into());
            }
        });
    }

    // 16B. Callback: Copiar código del chat al portapapeles
    {
        app.on_copy_chat_code(move |code_shared| {
            copy_to_clipboard(code_shared.as_ref());
        });
    }

    // 17. Callback: Limpiar historial de conversación del Chat IA
    {
        let app_weak = app.as_weak();
        app.on_clear_chat_history(move || {
            if let Some(app) = app_weak.upgrade() {
                let empty: Vec<ChatMessage> = Vec::new();
                app.set_chat_messages(ModelRc::from(Rc::new(VecModel::from(empty))));
            }
        });
    }

    // 18. Callback: Abrir archivo .py desde el disco
    {
        let app_weak = app.as_weak();
        app.on_open_file(move || {
            if let Some(app) = app_weak.upgrade() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Python Script", &["py", "txt"])
                    .set_title("Abrir script Python para auditoría AST")
                    .pick_file()
                {
                    match fs::read_to_string(&path) {
                        Ok(content) => {
                            let fname = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("script_externo.py");
                            app.set_active_filename(SharedString::from(fname));
                            app.set_active_sample_index(-1);
                            app.set_input_code(SharedString::from(&content));
                            app.set_input_editing_line(-1);
                            app.set_output_editing_line(-1);
                            ensure_tab_open(&app, 0, fname, true);
                            ensure_tab_open(&app, 1, "ast_reconstruido.py", false);
                            app.set_active_tab_index(0);
                            app.set_active_editor_tab(0);
                            execute_and_sync_ui(&app);
                            update_window_title(&app);
                        }
                        Err(e) => {
                            app.set_status_text(SharedString::from(format!(
                                "Error al leer archivo: {}",
                                e
                            )));
                        }
                    }
                }
            }
        });
    }

    // 19. Callback: Abrir carpeta completa de proyecto con Pipeline Automático de 7 Fases
    {
        let app_weak = app.as_weak();
        app.on_open_folder(move || {
            if let Some(app) = app_weak.upgrade() {
                if let Some(folder_path) = rfd::FileDialog::new()
                    .set_title("Abrir Carpeta de Proyecto - KRONOS Descode")
                    .pick_folder()
                {
                    let folder_name = folder_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("WORKSPACE")
                        .to_string();
                    let full_folder_str = folder_path.to_string_lossy().to_string();

                    // Inicializar árbol de archivos con la raíz expandida
                    let mut expanded = HashSet::new();
                    expanded.insert(folder_path.clone());
                    let visible_items = build_visible_tree(&folder_path, &expanded, None);
                    *EXPANDED_DIRS.lock().unwrap() = Some(expanded);

                    app.set_workspace_folder_name(SharedString::from(&folder_name));
                    app.set_workspace_folder_path(SharedString::from(&full_folder_str));
                    app.set_workspace_files(ModelRc::from(Rc::new(VecModel::from(visible_items))));
                    app.set_sidebar_visible(true);
                    app.set_sec_workspace_open(true);
                    update_window_title(&app);

                    // Activar Pantalla de Análisis Automático (center_view_mode = 4)
                    app.set_center_view_mode(4);
                    let initial_phases: Vec<AnalysisPhaseItem> = ProjectAnalysisPipeline::initial_phases()
                        .into_iter()
                        .map(|p| AnalysisPhaseItem {
                            phase_id: p.index as i32,
                            name: SharedString::from(&p.name),
                            status_str: SharedString::from(p.status.as_str()),
                            status_code: match p.status {
                                PhaseStatus::Pending => 0,
                                PhaseStatus::Running => 1,
                                PhaseStatus::Completed => 2,
                                PhaseStatus::Skipped => 3,
                                PhaseStatus::Warning => 4,
                                PhaseStatus::Error => 5,
                            },
                            elapsed_ms: p.elapsed_ms as i32,
                            detail: SharedString::from(&p.detail),
                        })
                        .collect();

                    app.set_analysis_phases(ModelRc::from(Rc::new(VecModel::from(initial_phases))));
                    app.set_analysis_progress(0.0);
                    app.set_analysis_current_op("Inicializando motor de análisis determinista...".into());
                    app.set_analysis_live_log(format!("[INICIO] Pipeline KRONOS DESCODE en: {}\n", full_folder_str).into());
                    app.set_analysis_is_running(true);
                    app.set_analysis_is_finished(false);
                    app.set_analysis_verdict_case(0);
                    app.set_analysis_verdict_title("".into());
                    app.set_analysis_verdict_summary("".into());

                    // Despachar el pipeline asíncrono
                    let app_weak_prog = app.as_weak();
                    let on_progress = move |update: crate::analysis::AnalysisProgressUpdate| {
                        if let Some(app) = app_weak_prog.upgrade() {
                            app.set_analysis_progress(update.progress_percent);
                            app.set_analysis_current_op(SharedString::from(&update.current_operation));
                            app.set_analysis_live_log(SharedString::from(&update.live_log));
                            let phases_ui: Vec<AnalysisPhaseItem> = update.phases.into_iter().map(|p| {
                                AnalysisPhaseItem {
                                    phase_id: p.index as i32,
                                    name: SharedString::from(&p.name),
                                    status_str: SharedString::from(p.status.as_str()),
                                    status_code: match p.status {
                                        PhaseStatus::Pending => 0,
                                        PhaseStatus::Running => 1,
                                        PhaseStatus::Completed => 2,
                                        PhaseStatus::Skipped => 3,
                                        PhaseStatus::Warning => 4,
                                        PhaseStatus::Error => 5,
                                    },
                                    elapsed_ms: p.elapsed_ms as i32,
                                    detail: SharedString::from(&p.detail),
                                }
                            }).collect();
                            app.set_analysis_phases(ModelRc::from(Rc::new(VecModel::from(phases_ui))));
                        }
                    };

                    let app_weak_done = app.as_weak();
                    let on_complete = move |report: ProjectAnalysisReport| {
                        if let Some(app) = app_weak_done.upgrade() {
                            app.set_analysis_is_running(false);
                            app.set_analysis_is_finished(true);
                            app.set_analysis_progress(100.0);
                            app.set_analysis_current_op("Análisis completado.".into());
                            app.set_analysis_verdict_title(SharedString::from(&report.verdict_headline));
                            app.set_analysis_verdict_summary(SharedString::from(&report.verdict_summary));
                            let verdict_num = match report.verdict_case {
                                AnalysisVerdictCase::CaseA_Obfuscated => 1,
                                AnalysisVerdictCase::CaseB_CleanProject => 2,
                                AnalysisVerdictCase::CaseC_IncompatibleFiles => 3,
                                AnalysisVerdictCase::CaseD_SecurityThreat => 4,
                            };
                            app.set_analysis_verdict_case(verdict_num);
                            app.set_status_text(SharedString::from(format!(
                                "Análisis completado: {} ({} archivos examinados)",
                                report.verdict_headline, report.total_files
                            )));

                            let is_obfuscated = report.verdict_case == AnalysisVerdictCase::CaseA_Obfuscated
                                || !report.obfuscation_findings.is_empty();

                            // Transición directa al workspace central sin bloquear al usuario
                            app.set_center_view_mode(0);

                            if is_obfuscated {
                                if let Some(first_obf) = report.obfuscation_findings.first() {
                                    let full_path = std::path::Path::new(&report.project_path).join(&first_obf.file_path);
                                    if let Ok(bytes) = std::fs::read(&full_path) {
                                        let content = String::from_utf8_lossy(&bytes).to_string();
                                        let target_filename = first_obf.file_path.clone();
                                        app.set_has_open_file(true);
                                        app.set_active_filename(SharedString::from(&target_filename));
                                        app.set_input_code(SharedString::from(&content));
                                        app.set_input_line_numbers(build_line_numbers_text(&content));
                                        let hide_comments = app.get_opt_hide_comments();
                                        let query = app.get_search_query().to_string();
                                        app.set_input_lines(build_syntax_lines_with_active(&content, false, hide_comments, &query, -1));
                                        ensure_tab_open(&app, 0, &target_filename, true);
                                        ensure_tab_open(&app, 1, "ast_reconstruido.py", false);
                                        app.set_active_tab_index(0);
                                        app.set_active_editor_tab(0);

                                        // Ejecutar de inmediato el motor de desofuscación
                                        execute_and_sync_ui(&app);

                                        app.set_status_text(SharedString::from(format!(
                                            "Ofuscación detectada: {} cargado y desofuscado de inmediato.",
                                            target_filename
                                        )));
                                    }
                                }
                            }

                            if !app.get_has_open_file() {
                                let mut queue = vec![std::path::PathBuf::from(&report.project_path)];
                                'load_first_code: while let Some(cur) = queue.pop() {
                                    if let Ok(entries) = std::fs::read_dir(&cur) {
                                        for entry in entries.flatten() {
                                            let ep = entry.path();
                                            if ep.is_dir() {
                                                let dname = ep.file_name().and_then(|n| n.to_str()).unwrap_or("");
                                                if dname != ".git" && dname != "target" && dname != "desofuscado" {
                                                    queue.push(ep);
                                                }
                                            } else if ep.is_file() {
                                                let ext = ep.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                                                if ext == "java" || ext == "py" {
                                                    if let Ok(bytes) = std::fs::read(&ep) {
                                                        let content = String::from_utf8_lossy(&bytes).to_string();
                                                        let rel = ep.strip_prefix(&report.project_path).unwrap_or(&ep).to_string_lossy().to_string();
                                                        app.set_has_open_file(true);
                                                        app.set_active_filename(SharedString::from(&rel));
                                                        app.set_input_code(SharedString::from(&content));
                                                        app.set_input_line_numbers(build_line_numbers_text(&content));
                                                        let hide_comments = app.get_opt_hide_comments();
                                                        let query = app.get_search_query().to_string();
                                                        app.set_input_lines(build_syntax_lines_with_active(&content, false, hide_comments, &query, -1));
                                                        ensure_tab_open(&app, 0, &rel, true);
                                                        ensure_tab_open(&app, 1, "ast_reconstruido.py", false);
                                                        app.set_active_tab_index(0);
                                                        app.set_active_editor_tab(0);
                                                        execute_and_sync_ui(&app);
                                                        break 'load_first_code;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            *LAST_ANALYSIS_REPORT.lock().unwrap() = Some(report);
                        }
                    };

                    execute_project_analysis_async(folder_path, on_progress, on_complete);
                }
            }
        });
    }

    // 19b. Callback: Alternar expansión de carpeta individual
    {
        let app_weak = app.as_weak();
        app.on_toggle_folder_expand(move |folder_path_str| {
            if let Some(app) = app_weak.upgrade() {
                let target = PathBuf::from(folder_path_str.as_str());
                let root_str = app.get_workspace_folder_path().to_string();
                if root_str.is_empty() {
                    return;
                }
                let root = PathBuf::from(&root_str);

                let mut guard = EXPANDED_DIRS.lock().unwrap();
                let set = guard.get_or_insert_with(|| {
                    let mut s = HashSet::new();
                    s.insert(root.clone());
                    s
                });
                toggle_folder_expanded(set, &target);

                let active_file = app.get_active_filename().to_string();
                let active_path = if !active_file.is_empty() {
                    Some(root.join(&active_file))
                } else {
                    None
                };

                let visible = build_visible_tree(&root, set, active_path.as_deref());
                app.set_workspace_files(ModelRc::from(Rc::new(VecModel::from(visible))));
            }
        });
    }

    // 19c. Callback: Entrar al Workspace desde la pantalla de análisis
    {
        let app_weak = app.as_weak();
        app.on_enter_workspace(move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_center_view_mode(0);
            }
        });
    }

    // 19d. Callback: Cancelar análisis o minimizar a segundo plano
    {
        let app_weak = app.as_weak();
        app.on_cancel_analysis(move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_center_view_mode(0);
            }
        });
    }

    // 20. Callback: Seleccionar y abrir un archivo del árbol del workspace
    {
        let app_weak = app.as_weak();
        app.on_select_workspace_file(move |file_idx_raw| {
            if let Some(app) = app_weak.upgrade() {
                let idx = file_idx_raw as usize;
                let current_model = app.get_workspace_files();
                if idx >= current_model.row_count() {
                    return;
                }
                if let Some(item) = current_model.row_data(idx) {
                    if item.is_dir {
                        return;
                    }
                    let full_path_str = item.full_path.to_string();
                    let fname = item.name.to_string();

                    let content = match fs::read_to_string(&full_path_str) {
                        Ok(c) => c,
                        Err(_) => {
                            let size = fs::metadata(&full_path_str).map(|m| m.len()).unwrap_or(0);
                            format!(
                                "# =========================================================================\n\
                                # ARCHIVO BINARIO / COMPRIMIDO: {}\n\
                                # Ruta: {}\n\
                                # Tamaño: {} bytes ({})\n\
                                # =========================================================================\n\n\
                                # Este artefacto contiene datos binarios o formato empaquetado.\n\
                                # KRONOS DESCODE protege su integridad preservando el contenido en disco.\n\
                                # Las fases 03 y 04 del pipeline automático auditan su estructura interna.\n",
                                fname, full_path_str, size, crate::fs::format_file_size(size)
                            )
                        }
                    };

                    let mut items: Vec<WorkspaceFileItem> = (0..current_model.row_count())
                        .filter_map(|i| current_model.row_data(i))
                        .collect();
                    for (i, it) in items.iter_mut().enumerate() {
                        it.is_active = i == idx;
                    }
                    app.set_workspace_files(ModelRc::from(Rc::new(VecModel::from(items))));

                    app.set_active_filename(SharedString::from(&fname));
                    app.set_active_sample_index(-1);
                    app.set_input_code(SharedString::from(&content));
                    app.set_input_editing_line(-1);
                    app.set_output_editing_line(-1);
                    app.set_has_open_file(true);

                    ensure_tab_open(&app, 0, &fname, true);
                    app.set_active_tab_index(0);
                    app.set_active_editor_tab(0);
                    execute_and_sync_ui(&app);
                    update_window_title(&app);
                    app.set_status_text(SharedString::from(format!(
                        "Abierto: {}",
                        fname
                    )));
                }
            }
        });
    }

    // 21. Callback: Guardar archivo actual o salida AST
    {
        let app_weak = app.as_weak();
        app.on_save_file(move || {
            if let Some(app) = app_weak.upgrade() {
                let is_input = app.get_active_editor_tab() == 0;
                let (code_to_save, default_name) = if is_input {
                    let name = app.get_active_filename().to_string();
                    let f = if name.trim().is_empty() {
                        "script.py".to_string()
                    } else {
                        name
                    };
                    (app.get_input_code().to_string(), f)
                } else {
                    (
                        app.get_output_code().to_string(),
                        "ast_reconstruido.py".to_string(),
                    )
                };

                if code_to_save.trim().is_empty() {
                    app.set_status_text("No hay código para guardar.".into());
                    return;
                }
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Python Script", &["py", "txt"])
                    .set_file_name(&default_name)
                    .set_title("Guardar código")
                    .save_file()
                {
                    match fs::write(&path, code_to_save) {
                        Ok(_) => {
                            let fname = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(&default_name);
                            if is_input {
                                app.set_active_filename(SharedString::from(fname));
                                ensure_tab_open(&app, 0, fname, true);
                                update_window_title(&app);
                            }
                            app.set_status_text(SharedString::from(format!(
                                "Archivo guardado en: {}",
                                path.display()
                            )));
                        }
                        Err(e) => {
                            app.set_status_text(SharedString::from(format!(
                                "Error al guardar: {}",
                                e
                            )));
                        }
                    }
                }
            }
        });
    }

    // 22. Callback: Copiar resultado al portapapeles
    {
        let app_weak = app.as_weak();
        app.on_copy_output(move || {
            if let Some(app) = app_weak.upgrade() {
                let out = if app.get_active_editor_tab() == 0 && !app.get_split_view_enabled() {
                    app.get_input_code().to_string()
                } else {
                    app.get_output_code().to_string()
                };
                if !out.trim().is_empty() {
                    copy_to_clipboard(&out);
                    app.set_status_text(
                        "Código copiado al portapapeles de Windows.".into(),
                    );
                }
            }
        });
    }

    // 23. Callback: Pegar código desde el portapapeles y desofuscar
    {
        let app_weak = app.as_weak();
        app.on_paste_clipboard(move || {
            if let Some(app) = app_weak.upgrade() {
                if let Some(pasted) = paste_from_clipboard() {
                    app.set_active_filename("portapapeles.py".into());
                    app.set_active_sample_index(-1);
                    app.set_input_code(SharedString::from(&pasted));
                    app.set_input_editing_line(-1);
                    app.set_output_editing_line(-1);
                    ensure_tab_open(&app, 0, "portapapeles.py", true);
                    ensure_tab_open(&app, 1, "ast_reconstruido.py", false);
                    app.set_active_tab_index(0);
                    app.set_active_editor_tab(0);
                    execute_and_sync_ui(&app);
                    update_window_title(&app);
                    app.set_status_text(
                        "Código pegado desde el portapapeles y analizado con éxito.".into(),
                    );
                }
            }
        });
    }

    // 24. Callback: Limpiar buffers
    {
        let app_weak = app.as_weak();
        app.on_clear_all(move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_input_code("".into());
                app.set_output_code("".into());
                app.set_input_editing_line(-1);
                app.set_output_editing_line(-1);
                execute_and_sync_ui(&app);
                app.set_status_text("Espacio de trabajo limpio.".into());
            }
        });
    }

    // 25. Callback: Filtrar comandos de Command Palette
    {
        let app_weak = app.as_weak();
        app.on_command_palette_query_changed(move |query| {
            if let Some(app) = app_weak.upgrade() {
                update_filtered_commands(&app, query.as_str());
            }
        });
    }

    // 26. Callback: Ejecutar comando de Command Palette
    {
        let app_weak = app.as_weak();
        app.on_execute_palette_command(move |cmd_id| {
            if let Some(app) = app_weak.upgrade() {
                match cmd_id {
                    0 => {
                        app.invoke_run_deobfuscation();
                        app.set_active_editor_tab(1);
                    }
                    1 => app.invoke_new_blank_file(),
                    2 => app.invoke_open_file(),
                    3 => app.invoke_open_folder(),
                    4 => app.invoke_save_file(),
                    5 => app.invoke_paste_clipboard(),
                    6 => {
                        if app.get_open_tabs().row_count() > 0 {
                            app.set_input_editing_line(-1);
                            app.set_output_editing_line(-1);
                            app.set_edit_mode(!app.get_edit_mode());
                        }
                    }
                    7 => app.set_center_view_mode(0),
                    8 => app.set_center_view_mode(1),
                    9 => app.set_center_view_mode(2),
                    10 => {
                        if app.get_open_tabs().row_count() > 0 {
                            app.set_split_view_enabled(!app.get_split_view_enabled());
                            persist_current_session(&app);
                        }
                    }
                    11 => {
                        app.set_sidebar_visible(!app.get_sidebar_visible());
                        persist_current_session(&app);
                    }
                    12 => {
                        app.set_bottom_panel_visible(!app.get_bottom_panel_visible());
                        persist_current_session(&app);
                    }
                    13 => {
                        app.set_ai_chat_visible(!app.get_ai_chat_visible());
                        persist_current_session(&app);
                    }
                    14 => {
                        app.set_ai_chat_visible(true);
                        app.invoke_trigger_contextual_ai_action("explain".into());
                    }
                    15 => {
                        app.set_ai_chat_visible(true);
                        app.invoke_trigger_contextual_ai_action("detect_obfuscation".into());
                    }
                    16 => {
                        app.set_ai_chat_visible(true);
                        app.invoke_trigger_contextual_ai_action("reconstruct".into());
                    }
                    17 => app.invoke_clear_chat_history(),
                    _ => {}
                }
            }
        });
    }
}
