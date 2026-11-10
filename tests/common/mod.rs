use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// =========================================================================
// 1. Sovereign Obsidian & Crimson Theme Tokens (ORIGINAL_REQUEST §R2)
// =========================================================================
pub const THEME_BACKGROUND: &str = "#0B0B0D";
pub const THEME_SIDEBAR: &str = "#101013";
pub const THEME_SECONDARY_SURFACE: &str = "#151518";
pub const THEME_EDITOR_BG: &str = "#0D0D10";
pub const THEME_BORDERS: &str = "#26262B";
pub const THEME_PRIMARY_TEXT: &str = "#E8E8EB";
pub const THEME_SECONDARY_TEXT: &str = "#85858F";
pub const THEME_PRIMARY_ACCENT: &str = "#D9283D";
pub const THEME_ACCENT_HOVER: &str = "#F13B50";
pub const THEME_SELECTION: &str = "rgba(217, 40, 61, 0.14)";

// =========================================================================
// 2. Models & AI Constants (ORIGINAL_REQUEST §R4)
// =========================================================================
pub const MODEL_GEMINI_3_1_FLASH_LITE: &str = "gemini-3.1-flash-lite";
pub const MODEL_GEMINI_3_8_FLASH: &str = "gemini-3.8-flash";
pub const GEMINI_API_ENDPOINT_BASE: &str =
    "https://generativelanguage.googleapis.com/v1beta/models";

// =========================================================================
// 3. Session & Configuration Models (PROJECT.md § Interface Contracts)
// =========================================================================
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorTabItem {
    pub id: i32,
    pub title: String,
    pub tab_type: i32, // 0 = standard/python, 1 = diff, 2 = ast
    pub dot_color: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionState {
    pub version: String,
    pub window_width: u32,
    pub window_height: u32,
    pub sidebar_width: u32,
    pub ai_panel_width: u32,
    pub active_activity_tab: i32, // 0=Explorer, 1=ReverseEng, 2=AST, 3=Pipeline, 4=History, 5=AI
    pub active_tab_index: i32,
    pub open_tabs: Vec<EditorTabItem>,
    pub active_file_path: Option<String>,
    pub recent_projects: Vec<String>,
    pub split_view_enabled: bool,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            window_width: 1400,
            window_height: 900,
            sidebar_width: 260,
            ai_panel_width: 380,
            active_activity_tab: 0,
            active_tab_index: -1,
            open_tabs: Vec::new(),
            active_file_path: None,
            recent_projects: Vec::new(),
            split_view_enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub gemini_api_key: String,
    pub default_model: String,
    pub current_mode: String,
    pub auto_deobfuscate_on_open: bool,
    pub max_history_entries: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            gemini_api_key: String::new(),
            default_model: MODEL_GEMINI_3_1_FLASH_LITE.to_string(),
            current_mode: "Deobfuscate".to_string(),
            auto_deobfuscate_on_open: false,
            max_history_entries: 50,
        }
    }
}

// =========================================================================
// 4. Extensible AI Provider Trait & Contracts (PROJECT.md § Interface Contracts)
// =========================================================================
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiAction {
    ExplainCode,
    AnalyzeFile,
    InterpretAst,
    DetectObfuscation,
    SuggestReconstruction,
    CustomQuery,
}

#[derive(Debug, Clone)]
pub struct AiQueryRequest {
    pub prompt: String,
    pub model: String,
    pub context_code: String,
    pub action: AiAction,
    pub api_key: String,
}

#[derive(Debug, Clone)]
pub struct AiQueryResponse {
    pub reply_text: String,
    pub suggested_code: Option<String>,
    pub model_used: String,
    pub token_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiError {
    EmptyApiKey,
    InvalidApiKey(String),
    NetworkError(String),
    RateLimitExceeded,
    ServiceUnavailable(String),
    MalformedResponse(String),
}

pub trait AiProvider: Send + Sync {
    fn query(&self, req: &AiQueryRequest) -> Result<AiQueryResponse, AiError>;
    fn name(&self) -> &str;
}

// =========================================================================
// 5. Analysis History Model (PROJECT.md § Module Boundaries)
// =========================================================================
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryRecord {
    pub id: u64,
    pub timestamp: u64,
    pub filename: String,
    pub original_code: String,
    pub transformed_code: String,
    pub alerts_count: usize,
    pub diff_lines_changed: usize,
    pub execution_ms: u128,
}

// =========================================================================
// 6. Workspace File System & Directory Scanner
// =========================================================================
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceFileItem {
    pub name: String,
    pub rel_path: String,
    pub full_path: String,
    pub is_dir: bool,
    pub is_python: bool,
    pub depth: usize,
    pub is_active: bool,
}

pub fn scan_folder_recursive(root: &Path) -> Vec<WorkspaceFileItem> {
    let mut items = Vec::new();
    scan_dir_inner(root, root, 0, &mut items);
    items
}

fn scan_dir_inner(
    base: &Path,
    current: &Path,
    depth: usize,
    out: &mut Vec<WorkspaceFileItem>,
) {
    let read_res = match fs::read_dir(current) {
        Ok(r) => r,
        Err(_) => return,
    };

    let mut entries: Vec<fs::DirEntry> = read_res.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let name_os = entry.file_name();
        let name = name_os.to_string_lossy().to_string();

        // Ignorar directorios de control de versiones y caché
        if name == ".git" || name == "__pycache__" || name == ".idea" || name == "target" {
            continue;
        }

        let full_path = entry.path();
        let rel_path = match full_path.strip_prefix(base) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => name.clone(),
        };

        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let is_python = !is_dir && (name.ends_with(".py") || name.ends_with(".pyw"));

        out.push(WorkspaceFileItem {
            name: name.clone(),
            rel_path,
            full_path: full_path.to_string_lossy().to_string(),
            is_dir,
            is_python,
            depth,
            is_active: false,
        });

        if is_dir {
            scan_dir_inner(base, &full_path, depth + 1, out);
        }
    }
}

// =========================================================================
// 7. Test Workspace RAII Fixture
// =========================================================================
pub struct TempWorkspace {
    pub root: PathBuf,
}

impl TempWorkspace {
    pub fn new(prefix: &str) -> Self {
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("kronos_e2e_{}_{}", prefix, epoch));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self { root: dir }
    }

    pub fn write_file(&self, rel_path: &str, content: &str) -> PathBuf {
        let path = self.root.join(rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

// =========================================================================
// 8. AST & Diff Helpers
// =========================================================================
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLineType {
    Unmodified,
    Added,
    Deleted,
    Modified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub line_index: usize,
    pub original_text: String,
    pub modified_text: String,
    pub diff_type: DiffLineType,
}

pub fn compute_simple_diff(original: &str, modified: &str) -> Vec<DiffLine> {
    let orig_lines: Vec<&str> = original.lines().collect();
    let mod_lines: Vec<&str> = modified.lines().collect();
    let max_len = orig_lines.len().max(mod_lines.len());
    let mut diff = Vec::new();

    for i in 0..max_len {
        let orig = orig_lines.get(i).copied().unwrap_or("");
        let modi = mod_lines.get(i).copied().unwrap_or("");

        let diff_type = if i >= orig_lines.len() {
            DiffLineType::Added
        } else if i >= mod_lines.len() {
            DiffLineType::Deleted
        } else if orig == modi {
            DiffLineType::Unmodified
        } else {
            DiffLineType::Modified
        };

        diff.push(DiffLine {
            line_index: i + 1,
            original_text: orig.to_string(),
            modified_text: modi.to_string(),
            diff_type,
        });
    }

    diff
}

pub fn build_line_numbers_text(code: &str) -> String {
    let count = if code.is_empty() {
        1
    } else {
        code.lines().count().max(1)
    };
    (1..=count)
        .map(|n| n.to_string())
        .collect::<Vec<String>>()
        .join("\n")
}
