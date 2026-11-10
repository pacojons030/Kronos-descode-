use std::fs;
use std::path::{Path, PathBuf};
use slint::SharedString;
use crate::WorkspaceFileItem;

/// Splits code into lines, preserving empty trailing lines.
pub fn split_code_lines(code: &str) -> Vec<&str> {
    if code.is_empty() {
        vec![""]
    } else {
        code.split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l))
            .collect()
    }
}

/// Maximum number of lines permitted in editor buffer operations to prevent unbounded allocation and OOM.
pub const MAX_BUFFER_LINES: usize = 500_000;

/// Replaces a single line in a multi-line buffer.
/// Guarded against OOM: rejects line indices >= MAX_BUFFER_LINES and buffers exceeding MAX_BUFFER_LINES.
pub fn replace_single_line(code: &str, line_idx: usize, new_line: &str) -> String {
    if line_idx >= MAX_BUFFER_LINES {
        return code.to_string();
    }
    let mut lines: Vec<String> = split_code_lines(code)
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    if lines.len() > MAX_BUFFER_LINES {
        return code.to_string();
    }
    if lines.len() <= line_idx {
        lines.reserve(line_idx + 1 - lines.len());
        while lines.len() <= line_idx {
            lines.push(String::new());
        }
    }
    lines[line_idx] = new_line.to_string();
    lines.join("\n")
}

/// Inserts a newline after line_idx preserving indentation from current_line_text.
/// Guarded against OOM: rejects line indices >= MAX_BUFFER_LINES and buffers exceeding MAX_BUFFER_LINES.
pub fn insert_newline_after(code: &str, line_idx: usize, current_line_text: &str) -> String {
    if line_idx >= MAX_BUFFER_LINES {
        return code.to_string();
    }
    let mut lines: Vec<String> = split_code_lines(code)
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    if lines.len() >= MAX_BUFFER_LINES {
        return code.to_string();
    }
    if lines.len() <= line_idx {
        lines.reserve(line_idx + 2 - lines.len());
        while lines.len() <= line_idx {
            lines.push(String::new());
        }
    }
    lines[line_idx] = current_line_text.to_string();
    let indent: String = current_line_text
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();
    lines.insert(line_idx + 1, indent);
    lines.join("\n")
}

use std::collections::HashSet;

/// Classifies a file or folder into an icon type category.
/// 0 = folder, 1 = java, 2 = python, 3 = archive, 4 = config, 5 = text/doc, 6 = binary/class, 7 = generic
pub fn detect_icon_type(path: &Path, is_dir: bool) -> i32 {
    if is_dir {
        return 0;
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "java" => 1,
        "py" | "pyw" => 2,
        "jar" | "zip" | "war" | "tar" | "gz" | "7z" | "rar" => 3,
        "yml" | "yaml" | "json" | "toml" | "xml" | "properties" | "cfg" | "ini" => 4,
        "txt" | "md" | "markdown" | "rst" | "log" => 5,
        "class" | "bin" | "exe" | "dll" | "so" | "dylib" | "o" => 6,
        _ => 7,
    }
}

/// Formats byte counts into human-readable strings (e.g. "4.2 KB", "1.1 MB").
pub fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Toggles expansion of a folder in the expanded directories set.
pub fn toggle_folder_expanded(expanded_dirs: &mut HashSet<PathBuf>, folder_path: &Path) -> bool {
    if expanded_dirs.contains(folder_path) {
        expanded_dirs.remove(folder_path);
        false
    } else {
        expanded_dirs.insert(folder_path.to_path_buf());
        true
    }
}

/// Recursively builds the visible file tree respecting individual folder expansion state.
pub fn build_visible_tree(
    root: &Path,
    expanded_dirs: &HashSet<PathBuf>,
    active_path: Option<&Path>,
) -> Vec<WorkspaceFileItem> {
    let mut items = Vec::new();
    walk_visible_dir(root, root, 0, expanded_dirs, active_path, &mut items);
    items
}

fn walk_visible_dir(
    root: &Path,
    current_dir: &Path,
    depth: i32,
    expanded_dirs: &HashSet<PathBuf>,
    active_path: Option<&Path>,
    out: &mut Vec<WorkspaceFileItem>,
) {
    if depth > 12 || out.len() >= 1000 {
        return;
    }

    let entries = match fs::read_dir(current_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let fname = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        if fname.starts_with('.')
            || fname.eq_ignore_ascii_case("node_modules")
            || fname.eq_ignore_ascii_case("target")
            || fname.eq_ignore_ascii_case("__pycache__")
            || fname.eq_ignore_ascii_case("venv")
            || fname.eq_ignore_ascii_case(".env")
            || fname.eq_ignore_ascii_case("dist")
            || fname.eq_ignore_ascii_case("build")
        {
            continue;
        }

        if path.is_dir() {
            dirs.push(path);
        } else if path.is_file() {
            files.push(path);
        }
    }

    dirs.sort_by(|a, b| {
        let an = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let bn = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
        an.to_lowercase().cmp(&bn.to_lowercase())
    });

    files.sort_by(|a, b| {
        let an = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let bn = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
        an.to_lowercase().cmp(&bn.to_lowercase())
    });

    // Directories first
    for dir in dirs {
        if out.len() >= 1000 {
            break;
        }
        let dirname = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("dir")
            .to_string();
        let rel_path = dir
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| dirname.clone());

        let is_expanded = expanded_dirs.contains(&dir);
        let has_children = fs::read_dir(&dir)
            .map(|mut r| r.next().is_some())
            .unwrap_or(false);

        out.push(WorkspaceFileItem {
            name: SharedString::from(&dirname),
            rel_path: SharedString::from(&rel_path),
            full_path: SharedString::from(dir.to_string_lossy().as_ref()),
            is_dir: true,
            is_python: false,
            depth,
            is_active: false,
            is_expanded,
            has_children,
            icon_type: 0,
            size_str: SharedString::default(),
        });

        // Only traverse children if explicitly expanded
        if is_expanded {
            walk_visible_dir(root, &dir, depth + 1, expanded_dirs, active_path, out);
        }
    }

    // Files next
    for file in files {
        if out.len() >= 1000 {
            break;
        }
        let fname = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();
        let rel_path = file
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| fname.clone());
        let is_py = fname.to_lowercase().ends_with(".py");
        let icon_type = detect_icon_type(&file, false);
        let is_active = active_path.map(|ap| ap == file.as_path()).unwrap_or(false);
        let size_str = file
            .metadata()
            .map(|m| format_file_size(m.len()))
            .unwrap_or_default();

        out.push(WorkspaceFileItem {
            name: SharedString::from(&fname),
            rel_path: SharedString::from(&rel_path),
            full_path: SharedString::from(file.to_string_lossy().as_ref()),
            is_dir: false,
            is_python: is_py,
            depth,
            is_active,
            is_expanded: false,
            has_children: false,
            icon_type,
            size_str: SharedString::from(size_str),
        });
    }
}

/// Scans a folder recursively and returns Slint WorkspaceFileItem models.
pub fn scan_folder_recursive(root: &Path) -> Vec<WorkspaceFileItem> {
    let mut items = Vec::new();
    scan_dir(root, root, 0, &mut items);
    items
}

/// Recursive directory walker for Slint UI workspace tree.
pub fn scan_dir(root: &Path, current_dir: &Path, depth: i32, out: &mut Vec<WorkspaceFileItem>) {
    if depth > 4 || out.len() >= 600 {
        return;
    }

    let entries = match fs::read_dir(current_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let fname = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        // Ignorar carpetas ocultas y dependencias masivas
        if fname.starts_with('.')
            || fname.eq_ignore_ascii_case("node_modules")
            || fname.eq_ignore_ascii_case("target")
            || fname.eq_ignore_ascii_case("__pycache__")
            || fname.eq_ignore_ascii_case("venv")
            || fname.eq_ignore_ascii_case(".env")
            || fname.eq_ignore_ascii_case("dist")
            || fname.eq_ignore_ascii_case("build")
        {
            continue;
        }

        if path.is_dir() {
            dirs.push(path);
        } else if path.is_file() {
            files.push(path);
        }
    }

    dirs.sort_by(|a, b| {
        let an = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let bn = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
        an.to_lowercase().cmp(&bn.to_lowercase())
    });

    files.sort_by(|a, b| {
        let an = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let bn = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
        an.to_lowercase().cmp(&bn.to_lowercase())
    });

    for dir in dirs {
        if out.len() >= 600 {
            break;
        }
        let dirname = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("dir")
            .to_string();
        let rel_path = dir
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| dirname.clone());

        out.push(WorkspaceFileItem {
            name: SharedString::from(&dirname),
            rel_path: SharedString::from(&rel_path),
            full_path: SharedString::from(dir.to_string_lossy().as_ref()),
            is_dir: true,
            is_python: false,
            depth,
            is_active: false,
            is_expanded: true,
            has_children: true,
            icon_type: 0,
            size_str: SharedString::default(),
        });

        scan_dir(root, &dir, depth + 1, out);
    }

    for file in files {
        if out.len() >= 600 {
            break;
        }
        let fname = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();
        let rel_path = file
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| fname.clone());
        let is_py = fname.to_lowercase().ends_with(".py");
        let icon_type = detect_icon_type(&file, false);
        let size_str = file
            .metadata()
            .map(|m| format_file_size(m.len()))
            .unwrap_or_default();

        out.push(WorkspaceFileItem {
            name: SharedString::from(&fname),
            rel_path: SharedString::from(&rel_path),
            full_path: SharedString::from(file.to_string_lossy().as_ref()),
            is_dir: false,
            is_python: is_py,
            depth,
            is_active: false,
            is_expanded: false,
            has_children: false,
            icon_type,
            size_str: SharedString::from(size_str),
        });
    }
}

/// Representation of a workspace entry for filesystem inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_python: bool,
    pub depth: i32,
    pub extension: Option<String>,
}

/// Recursively scans folder entries with explicit depth and count limits, filtering out blacklisted directories.
pub fn scan_folder_entries(root: &Path, max_depth: i32, max_count: usize) -> Vec<WorkspaceEntry> {
    let mut entries = Vec::new();
    scan_entries_recursive(root, root, 0, max_depth, max_count, &mut entries);
    entries
}

fn scan_entries_recursive(
    _root: &Path,
    current_dir: &Path,
    depth: i32,
    max_depth: i32,
    max_count: usize,
    out: &mut Vec<WorkspaceEntry>,
) {
    if depth > max_depth || out.len() >= max_count {
        return;
    }

    let read_res = match fs::read_dir(current_dir) {
        Ok(r) => r,
        Err(_) => return,
    };

    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();

    for item in read_res.flatten() {
        let path = item.path();
        let fname = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        if fname.starts_with('.')
            || fname.eq_ignore_ascii_case("node_modules")
            || fname.eq_ignore_ascii_case("target")
            || fname.eq_ignore_ascii_case("__pycache__")
            || fname.eq_ignore_ascii_case("venv")
            || fname.eq_ignore_ascii_case(".env")
            || fname.eq_ignore_ascii_case("dist")
            || fname.eq_ignore_ascii_case("build")
        {
            continue;
        }

        if path.is_dir() {
            dirs.push(path);
        } else if path.is_file() {
            files.push(path);
        }
    }

    dirs.sort_by(|a, b| {
        let an = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let bn = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
        an.cmp(bn)
    });

    files.sort_by(|a, b| {
        let an = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let bn = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
        an.cmp(bn)
    });

    for dir in dirs {
        if out.len() >= max_count {
            break;
        }
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("dir")
            .to_string();

        out.push(WorkspaceEntry {
            name,
            path: dir.clone(),
            is_dir: true,
            is_python: false,
            depth,
            extension: None,
        });

        if depth < max_depth {
            scan_entries_recursive(_root, &dir, depth + 1, max_depth, max_count, out);
        }
    }

    for file in files {
        if out.len() >= max_count {
            break;
        }
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();
        let ext = file.extension().and_then(|e| e.to_str()).map(|s| s.to_string());
        let is_py = matches!(ext.as_deref(), Some("py") | Some("pyw"));

        out.push(WorkspaceEntry {
            name,
            path: file,
            is_dir: false,
            is_python: is_py,
            depth,
            extension: ext,
        });
    }
}

pub fn read_file_to_string(path: &Path) -> std::io::Result<String> {
    fs::read_to_string(path)
}

pub fn write_file_string(path: &Path, content: &str) -> std::io::Result<()> {
    fs::write(path, content)
}

pub fn write_string_to_file(path: &Path, content: &str) -> std::io::Result<()> {
    fs::write(path, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_scan_ignores_blacklisted_directories() {
        let temp_dir = std::env::temp_dir().join("kronos_test_fs_blacklist");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join(".git")).unwrap();
        fs::create_dir_all(temp_dir.join("__pycache__")).unwrap();
        fs::create_dir_all(temp_dir.join("node_modules")).unwrap();
        fs::create_dir_all(temp_dir.join("target")).unwrap();
        fs::create_dir_all(temp_dir.join("venv")).unwrap();
        fs::create_dir_all(temp_dir.join("src")).unwrap();

        fs::write(temp_dir.join(".git").join("HEAD"), "ref").unwrap();
        fs::write(temp_dir.join("__pycache__").join("cache.pyc"), "bytecode").unwrap();
        fs::write(temp_dir.join("node_modules").join("pkg.json"), "{}").unwrap();
        fs::write(temp_dir.join("src").join("main.py"), "print(1)").unwrap();

        let entries = scan_folder_entries(&temp_dir, 4, 500);

        // Verify blacklist rejection
        assert!(!entries.iter().any(|e| e.name == ".git"));
        assert!(!entries.iter().any(|e| e.name == "__pycache__"));
        assert!(!entries.iter().any(|e| e.name == "node_modules"));
        assert!(!entries.iter().any(|e| e.name == "target"));
        assert!(!entries.iter().any(|e| e.name == "venv"));

        // Verify legitimate directory and file included
        assert!(entries.iter().any(|e| e.name == "src" && e.is_dir));
        assert!(entries.iter().any(|e| e.name == "main.py" && !e.is_dir));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_workspace_hierarchy_depth_and_sort_order() {
        let temp_dir = std::env::temp_dir().join("kronos_test_fs_hierarchy");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("b_folder").join("nested")).unwrap();
        fs::create_dir_all(temp_dir.join("a_folder")).unwrap();

        fs::write(temp_dir.join("z_script.py"), "print('z')").unwrap();
        fs::write(temp_dir.join("a_script.py"), "print('a')").unwrap();
        fs::write(temp_dir.join("b_folder").join("child.py"), "print('child')").unwrap();
        fs::write(temp_dir.join("b_folder").join("nested").join("deep.py"), "print('deep')").unwrap();

        let entries = scan_folder_entries(&temp_dir, 4, 500);

        // Root items (depth 0): directories first ('a_folder', 'b_folder'), then files ('a_script.py', 'z_script.py')
        let root_entries: Vec<_> = entries.iter().filter(|e| e.depth == 0).collect();
        assert_eq!(root_entries[0].name, "a_folder");
        assert!(root_entries[0].is_dir);
        assert_eq!(root_entries[1].name, "b_folder");
        assert!(root_entries[1].is_dir);
        assert_eq!(root_entries[2].name, "a_script.py");
        assert!(!root_entries[2].is_dir);
        assert_eq!(root_entries[3].name, "z_script.py");
        assert!(!root_entries[3].is_dir);

        // Deep child depth verification
        let deep = entries.iter().find(|e| e.name == "deep.py").expect("deep.py must exist");
        assert_eq!(deep.depth, 2);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_workspace_file_attributes_and_python_detection() {
        let temp_dir = std::env::temp_dir().join("kronos_test_fs_attrs");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        fs::write(temp_dir.join("module.py"), "x = 42\n").unwrap();
        fs::write(temp_dir.join("script.pyw"), "y = 84\n").unwrap();
        fs::write(temp_dir.join("README.md"), "# Intro").unwrap();
        fs::write(temp_dir.join("data.json"), "{}").unwrap();

        let entries = scan_folder_entries(&temp_dir, 2, 100);

        let py = entries.iter().find(|e| e.name == "module.py").unwrap();
        assert!(py.is_python);
        assert_eq!(py.extension.as_deref(), Some("py"));

        let pyw = entries.iter().find(|e| e.name == "script.pyw").unwrap();
        assert!(pyw.is_python);

        let md = entries.iter().find(|e| e.name == "README.md").unwrap();
        assert!(!md.is_python);
        assert_eq!(md.extension.as_deref(), Some("md"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_workspace_read_and_write_file_io() {
        let temp_dir = std::env::temp_dir().join("kronos_test_fs_io");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        let target = temp_dir.join("sample.py");

        let content = "def hello():\n    return 'kronos'\n";
        write_file_string(&target, content).expect("Write must succeed");

        let read_back = read_file_to_string(&target).expect("Read must succeed");
        assert_eq!(read_back, content);

        // Non-existent file error handling
        let non_existent = temp_dir.join("does_not_exist.py");
        assert!(read_file_to_string(&non_existent).is_err());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_workspace_scan_recursion_depth_limit() {
        let temp_dir = std::env::temp_dir().join("kronos_test_fs_depth_limit");
        let _ = fs::remove_dir_all(&temp_dir);

        // Build a 6-deep folder tree: d0/d1/d2/d3/d4/d5/leaf.txt
        let deep_path = temp_dir.join("d0").join("d1").join("d2").join("d3").join("d4").join("d5");
        fs::create_dir_all(&deep_path).unwrap();
        fs::write(deep_path.join("leaf.txt"), "too deep").unwrap();

        // Scan with max_depth = 3
        let entries = scan_folder_entries(&temp_dir, 3, 500);

        // Ensure depth does not exceed 3
        assert!(!entries.iter().any(|e| e.depth > 3));
        assert!(!entries.iter().any(|e| e.name == "leaf.txt"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_replace_single_line_upper_bound_guard() {
        let code = "line1\nline2\nline3";
        let res_max = replace_single_line(code, MAX_BUFFER_LINES, "overflow");
        assert_eq!(res_max, code);

        let res_huge = replace_single_line(code, 100_000_000, "overflow");
        assert_eq!(res_huge, code);

        let res_usize_max = replace_single_line(code, usize::MAX, "overflow");
        assert_eq!(res_usize_max, code);
    }

    #[test]
    fn test_insert_newline_after_upper_bound_guard() {
        let code = "line1\nline2\nline3";
        let res_max = insert_newline_after(code, MAX_BUFFER_LINES, "line3");
        assert_eq!(res_max, code);

        let res_huge = insert_newline_after(code, 100_000_000, "line3");
        assert_eq!(res_huge, code);

        let res_usize_max = insert_newline_after(code, usize::MAX, "line3");
        assert_eq!(res_usize_max, code);
    }

    #[test]
    fn test_line_manipulation_within_safe_bounds() {
        let code = "a\nb";
        let res = replace_single_line(code, 5, "c");
        assert_eq!(res, "a\nb\n\n\n\nc");

        let nl = insert_newline_after("    indented()", 0, "    indented()");
        assert_eq!(nl, "    indented()\n    ");
    }
}
