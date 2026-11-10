pub mod workspace;

pub use workspace::{
    build_visible_tree, detect_icon_type, format_file_size, insert_newline_after,
    read_file_to_string, replace_single_line, scan_dir, scan_folder_entries,
    scan_folder_recursive, split_code_lines, toggle_folder_expanded, write_file_string,
    write_string_to_file, WorkspaceEntry, MAX_BUFFER_LINES,
};
