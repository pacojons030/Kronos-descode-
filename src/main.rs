#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod ai;
pub mod analysis;
pub mod ast;
pub mod config;
pub mod core;
pub mod fs;
pub mod project;
pub mod ui;

use slint::{ModelRc, SharedString, VecModel};
use std::rc::Rc;

slint::include_modules!();

// Backwards-compatible public re-exports
pub use ast::tree::{
    build_line_numbers_text, build_syntax_lines, build_syntax_lines_with_active,
    tokenize_python_line,
};
pub use fs::workspace::{
    insert_newline_after, replace_single_line, scan_dir, scan_folder_recursive,
    split_code_lines,
};
pub use ui::controller::{
    append_chat_message, ensure_tab_open, execute_and_sync_ui, sync_ui_state,
    update_window_title,
};

fn main() -> Result<(), slint::PlatformError> {
    let app = AppWindow::new()?;

    // Cargar configuración y sesión persistida
    let session = config::load_session();
    let cfg = config::load_config();

    // Preconfigurar estado inicial / restaurado
    app.set_gemini_api_key(SharedString::from(&cfg.gemini_api_key));
    app.set_selected_model(SharedString::from(&session.active_model));
    app.set_selected_ai_mode(SharedString::from(&session.active_mode));
    app.set_has_open_file(false);
    app.set_active_tab_index(-1);
    app.set_active_editor_tab(-1);
    app.set_open_tabs(ModelRc::from(Rc::new(VecModel::from(Vec::<EditorTabItem>::new()))));
    app.set_active_filename("".into());
    app.set_active_sample_index(-1);
    app.set_input_code("".into());
    app.set_output_code("".into());
    app.set_sidebar_visible(session.sidebar_visible);
    app.set_context_panel_width(session.context_panel_width);
    app.set_ai_chat_visible(session.ai_panel_visible);
    app.set_ai_panel_width(session.ai_panel_width);
    app.set_bottom_panel_visible(session.terminal_visible);
    app.set_bottom_panel_height(session.terminal_height);
    app.set_split_view_enabled(session.split_view_enabled);
    app.set_opt_normalize_vars(session.opt_normalize_vars);
    app.set_opt_remove_dead_code(session.opt_remove_dead_code);
    app.set_opt_decode_strings(session.opt_decode_strings);
    app.set_opt_hide_comments(session.opt_hide_comments);

    if let Some(folder_path_str) = &session.workspace_folder_path {
        let path = std::path::Path::new(folder_path_str);
        if path.exists() && path.is_dir() {
            let folder_name = session
                .workspace_folder_name
                .clone()
                .unwrap_or_else(|| "WORKSPACE".to_string());
            let scanned = scan_folder_recursive(path);
            app.set_workspace_folder_name(SharedString::from(folder_name));
            app.set_workspace_folder_path(SharedString::from(folder_path_str));
            app.set_workspace_files(ModelRc::from(Rc::new(VecModel::from(scanned))));
        }
    }

    update_window_title(&app);
    app.set_status_text("Ready".into());

    // Registrar todos los callbacks de la interfaz
    ui::register_all_callbacks(&app);

    // Inicializar lista de comandos de la Command Palette
    ui::controller::update_filtered_commands(&app, "");

    app.run()
}

#[cfg(test)]
mod main_tests {
    use super::*;
    use slint::{Color, Model};
    use std::fs;

    #[test]
    fn test_replace_and_insert_lines_including_last_line_and_empty_buffer() {
        let orig = "linea1\nlinea2\nlinea3";
        let edited = replace_single_line(orig, 1, "linea2_modificada");
        assert_eq!(edited, "linea1\nlinea2_modificada\nlinea3");

        let with_nl = insert_newline_after("    indented = 1\nnext = 2", 0, "    indented = 1");
        assert_eq!(with_nl, "    indented = 1\n    \nnext = 2");

        // Caso crítico: pulsar Enter en la última línea sin sangría debe crear una 4ª línea visible
        let after_last = insert_newline_after(orig, 2, "linea3");
        assert_eq!(after_last, "linea1\nlinea2\nlinea3\n");
        let model = build_syntax_lines_with_active(&after_last, false, false, "", 3);
        assert_eq!(model.row_count(), 4);
        assert_eq!(model.row_data(3).unwrap().line_index, 3);
        assert_eq!(build_line_numbers_text(&after_last).as_str(), "1\n2\n3\n4");

        // Caso crítico: pulsar Enter en un buffer vacío debe pasar de 1 a 2 líneas
        let empty_nl = insert_newline_after("", 0, "");
        assert_eq!(empty_nl, "\n");
        let empty_model = build_syntax_lines_with_active(&empty_nl, false, false, "", 1);
        assert_eq!(empty_model.row_count(), 2);
        assert_eq!(build_line_numbers_text(&empty_nl).as_str(), "1\n2");
    }

    #[test]
    fn test_hide_comments_and_search_filter_with_active_editing_line() {
        let code = "# comentario 1\nx = 10\n# comentario 2\ny = 20";
        let all_lines = build_syntax_lines(code, false, false, "");
        assert_eq!(all_lines.row_count(), 4);

        let no_comments = build_syntax_lines(code, false, true, "");
        assert_eq!(no_comments.row_count(), 2);
        assert_eq!(no_comments.row_data(0).unwrap().line_index, 1);
        assert_eq!(no_comments.row_data(1).unwrap().line_index, 3);

        let filtered = build_syntax_lines(code, false, false, "y = 20");
        assert_eq!(filtered.row_count(), 1);
        assert_eq!(filtered.row_data(0).unwrap().line_index, 3);

        // Caso crítico: cuando el usuario edita la línea 1 convirtiéndola en comentario o borrando
        // el término buscado mientras hide_comments=true y search_query="y = 20" están activos,
        // la línea en edición (active_editing_line = 1) NO debe desaparecer ni desalinear line_index.
        let edited_as_comment = replace_single_line(code, 1, "# ahora es comentario");
        let simultaneous =
            build_syntax_lines_with_active(&edited_as_comment, false, true, "y = 20", 1);
        assert_eq!(simultaneous.row_count(), 2);
        assert_eq!(simultaneous.row_data(0).unwrap().line_index, 1);
        assert_eq!(
            simultaneous.row_data(0).unwrap().raw_text.as_str(),
            "# ahora es comentario"
        );
        assert_eq!(simultaneous.row_data(1).unwrap().line_index, 3);
    }

    #[test]
    fn test_tab_closing_adjacent_algorithm_and_bounds_safety() {
        // Simular lista de pestañas [Tab 0, Tab 1, Tab 2]
        let mut tabs = vec![
            EditorTabItem { id: 0, title: "file_a.py".into(), tab_type: 0, dot_color: Color::from_rgb_u8(0, 0, 0) },
            EditorTabItem { id: 1, title: "file_b.py".into(), tab_type: 1, dot_color: Color::from_rgb_u8(0, 0, 0) },
            EditorTabItem { id: 2, title: "file_c.py".into(), tab_type: 0, dot_color: Color::from_rgb_u8(0, 0, 0) },
        ];

        // Caso 1: Cerrar pestaña activa intermedia (índice 1 de 3) -> transfiere a la derecha (nuevo índice 1 = file_c.py)
        let close_idx = 1;
        let cur_active = 1;
        tabs.remove(close_idx);
        let new_active = if close_idx < cur_active {
            cur_active.saturating_sub(1)
        } else if close_idx == cur_active {
            if close_idx >= tabs.len() { tabs.len().saturating_sub(1) } else { close_idx }
        } else {
            cur_active
        };
        assert_eq!(tabs.len(), 2);
        assert_eq!(new_active, 1);
        assert_eq!(tabs[new_active].title.as_str(), "file_c.py");

        // Caso 2: Cerrar pestaña activa final (índice 1 de 2) -> transfiere a la izquierda (nuevo índice 0 = file_a.py)
        let close_idx_end = 1;
        let cur_active_end = 1;
        tabs.remove(close_idx_end);
        let new_active_end = if close_idx_end < cur_active_end {
            cur_active_end.saturating_sub(1)
        } else if close_idx_end == cur_active_end {
            if close_idx_end >= tabs.len() { tabs.len().saturating_sub(1) } else { close_idx_end }
        } else {
            cur_active_end
        };
        assert_eq!(tabs.len(), 1);
        assert_eq!(new_active_end, 0);
        assert_eq!(tabs[new_active_end].title.as_str(), "file_a.py");

        // Caso 3: Cerrar la única pestaña restante (índice 0 de 1) -> lista vacía (0 tabs), activa -1
        let close_idx_last = 0;
        tabs.remove(close_idx_last);
        assert!(tabs.is_empty());

        // Caso 4: Índice fuera de límites -> debe ser protegido sin pánico
        let out_of_bounds_idx = 99;
        assert!(out_of_bounds_idx >= tabs.len());
    }

    #[test]
    fn test_scan_folder_recursive_and_workspace_hierarchy() {
        let temp_dir = std::env::temp_dir().join("kronos_test_workspace_tree");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("subfolder")).unwrap();
        fs::create_dir_all(temp_dir.join(".git")).unwrap();
        fs::create_dir_all(temp_dir.join("__pycache__")).unwrap();

        fs::write(temp_dir.join("root_script.py"), "print(1)").unwrap();
        fs::write(temp_dir.join("README.txt"), "hello").unwrap();
        fs::write(temp_dir.join("subfolder").join("child.py"), "print(2)").unwrap();
        fs::write(temp_dir.join(".git").join("config"), "secret").unwrap();
        fs::write(temp_dir.join("__pycache__").join("cached.pyc"), "bytecode").unwrap();

        let items = scan_folder_recursive(&temp_dir);

        // Verificar que .git y __pycache__ fueron ignorados
        assert!(!items.iter().any(|i| i.name.as_str() == ".git"));
        assert!(!items.iter().any(|i| i.name.as_str() == "__pycache__"));

        // Verificar subcarpeta y profundidad
        let subfolder = items.iter().find(|i| i.name.as_str() == "subfolder").expect("subfolder must exist");
        assert!(subfolder.is_dir);
        assert_eq!(subfolder.depth, 0);

        let child = items.iter().find(|i| i.name.as_str() == "child.py").expect("child.py must exist");
        assert!(!child.is_dir);
        assert!(child.is_python);
        assert_eq!(child.depth, 1);

        let root_py = items.iter().find(|i| i.name.as_str() == "root_script.py").expect("root_script.py must exist");
        assert!(!root_py.is_dir);
        assert!(root_py.is_python);
        assert_eq!(root_py.depth, 0);

        let txt = items.iter().find(|i| i.name.as_str() == "README.txt").expect("README.txt must exist");
        assert!(!txt.is_dir);
        assert!(!txt.is_python);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
