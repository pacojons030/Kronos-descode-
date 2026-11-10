use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Classification of open editor tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TabType {
    #[default]
    SourceFile,         // 0: Standard source code file on disk
    DeobfuscatedResult, // 1: In-memory reconstructed output (e.g. ast_reconstruido.py)
    AstVisualizer,      // 2: AST node graph tab
    DiffComparison,     // 3: Code difference comparison
    Custom,             // 4: Extensible tab type
}

impl TabType {
    pub fn to_raw(self) -> i32 {
        match self {
            Self::SourceFile => 0,
            Self::DeobfuscatedResult => 1,
            Self::AstVisualizer => 2,
            Self::DiffComparison => 3,
            Self::Custom => 4,
        }
    }

    pub fn from_raw(raw: i32) -> Self {
        match raw {
            0 => Self::SourceFile,
            1 => Self::DeobfuscatedResult,
            2 => Self::AstVisualizer,
            3 => Self::DiffComparison,
            _ => Self::SourceFile,
        }
    }
}

/// Primary tool navigation modes in the Activity Bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NavigationMode {
    #[default]
    Explorer,             // 0: Workspace file tree & project explorer
    ReverseEngineering,   // 1: Deobfuscation engine & string decoders
    AstExplorer,          // 2: AST visual tree hierarchy
    CodeDiff,             // 3: Side-by-side / inline diff viewer
    Pipeline,             // 4: Multi-pass transformation pipeline
    KronosAi,             // 5: Dedicated AI assistant interface
    History,              // 6: Analysis operations & security audit log
}

impl NavigationMode {
    pub fn to_index(self) -> i32 {
        match self {
            Self::Explorer => 0,
            Self::ReverseEngineering => 1,
            Self::AstExplorer => 2,
            Self::CodeDiff => 3,
            Self::Pipeline => 4,
            Self::KronosAi => 5,
            Self::History => 6,
        }
    }

    pub fn from_index(index: i32) -> Self {
        match index {
            0 => Self::Explorer,
            1 => Self::ReverseEngineering,
            2 => Self::AstExplorer,
            3 => Self::CodeDiff,
            4 => Self::Pipeline,
            5 => Self::KronosAi,
            6 => Self::History,
            _ => Self::Explorer,
        }
    }
}

/// Persistent record of an open editor tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TabSessionItem {
    pub id: i32,
    pub title: String,
    pub file_path: Option<String>,
    pub tab_type: i32,
    pub is_dirty: bool,
}

pub type TabSessionInfo = TabSessionItem;

/// Complete persistent session state for KRONOS DESCODE.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionState {
    pub window_width: u32,
    pub window_height: u32,
    pub window_maximized: bool,
    pub sidebar_width: f32,
    pub sidebar_visible: bool,
    pub context_panel_width: f32,
    pub context_panel_visible: bool,
    pub terminal_height: f32,
    pub terminal_visible: bool,
    pub ai_panel_width: f32,
    pub ai_panel_visible: bool,
    pub split_view_enabled: bool,
    pub active_tool: String,
    pub active_tab_index: i32,
    pub open_tabs: Vec<TabSessionItem>,
    pub recent_projects: Vec<String>,
    pub active_project_path: Option<String>,
    pub active_model: String,
    pub active_mode: String,
    pub gemini_api_key: String,
    pub opt_normalize_vars: bool,
    pub opt_remove_dead_code: bool,
    pub opt_decode_strings: bool,
    pub opt_hide_comments: bool,
    pub workspace_folder_path: Option<String>,
    pub workspace_folder_name: Option<String>,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            window_width: 1280,
            window_height: 800,
            window_maximized: false,
            sidebar_width: 50.0,
            sidebar_visible: true,
            context_panel_width: 260.0,
            context_panel_visible: true,
            terminal_height: 220.0,
            terminal_visible: false,
            ai_panel_width: 340.0,
            ai_panel_visible: true,
            split_view_enabled: false,
            active_tool: "explorer".to_string(),
            active_tab_index: -1,
            open_tabs: Vec::new(),
            recent_projects: Vec::new(),
            active_project_path: None,
            active_model: "gemini-3.1-flash-lite".to_string(),
            active_mode: "deobfuscate".to_string(),
            gemini_api_key: crate::core::DEFAULT_GEMINI_API_KEY.to_string(),
            opt_normalize_vars: true,
            opt_remove_dead_code: true,
            opt_decode_strings: true,
            opt_hide_comments: false,
            workspace_folder_path: None,
            workspace_folder_name: None,
        }
    }
}

impl SessionState {
    /// Sanitizes and clamps all panel dimensions and bounds, neutralizing NaN and Infinite floats.
    pub fn clamp(&mut self) {
        self.window_width = self.window_width.clamp(1100, 7680);
        self.window_height = self.window_height.clamp(700, 4320);

        // Sidebar width: default 50.0, bounds [40.0, 120.0]
        if !self.sidebar_width.is_finite() || self.sidebar_width < 40.0 {
            self.sidebar_width = 50.0;
        } else if self.sidebar_width > 120.0 {
            self.sidebar_width = 120.0;
        }

        // Context panel width: default 260.0, bounds [160.0, 600.0]
        if !self.context_panel_width.is_finite() || self.context_panel_width < 160.0 {
            self.context_panel_width = 160.0;
        } else if self.context_panel_width > 600.0 {
            self.context_panel_width = 600.0;
        }

        // Terminal height: default 220.0, bounds [100.0, 800.0]
        if !self.terminal_height.is_finite() || self.terminal_height < 100.0 {
            self.terminal_height = 220.0;
        } else if self.terminal_height > 800.0 {
            self.terminal_height = 800.0;
        }

        // AI panel width: default 340.0, bounds [240.0, 700.0]
        if !self.ai_panel_width.is_finite() || self.ai_panel_width < 240.0 {
            self.ai_panel_width = 240.0;
        } else if self.ai_panel_width > 700.0 {
            self.ai_panel_width = 700.0;
        }

        if self.active_model.trim().is_empty() {
            self.active_model = "gemini-3.1-flash-lite".to_string();
        }

        if self.active_mode.trim().is_empty() {
            self.active_mode = "deobfuscate".to_string();
        }

        if self.active_tool.trim().is_empty() {
            self.active_tool = "explorer".to_string();
        }

        if self.open_tabs.is_empty() {
            self.active_tab_index = -1;
        } else if self.active_tab_index < 0 || (self.active_tab_index as usize) >= self.open_tabs.len() {
            self.active_tab_index = 0;
        }
    }

    /// Serializes session state to pretty-printed JSON after clamping bounds.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let mut clamped = self.clone();
        clamped.clamp();
        serde_json::to_string_pretty(&clamped)
    }

    /// Deserializes session state from JSON string with default fallbacks and clamps bounds.
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        let mut state: Self = serde_json::from_str(json_str)?;
        state.clamp();
        Ok(state)
    }

    /// Saves session state atomically to the specified file path with unique temp file.
    pub fn save_to_path(&self, path: &Path) -> std::io::Result<()> {
        let mut clamped = self.clone();
        clamped.clamp();

        let json = clamped
            .to_json()
            .map_err(std::io::Error::other)?;

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        // Unique temporary filename to prevent collisions in concurrent saves
        static SAVE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let counter = SAVE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let pid = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("session");
        let tmp_filename = format!("{}.{}.{}.{}.tmp", file_stem, pid, counter, nanos);
        let tmp_path = match path.parent() {
            Some(parent) => parent.join(tmp_filename),
            None => PathBuf::from(tmp_filename),
        };

        fs::write(&tmp_path, json.as_bytes())?;

        let mut rename_result = fs::rename(&tmp_path, path);
        #[cfg(target_os = "windows")]
        if rename_result.is_err() {
            for attempt in 1..=3 {
                std::thread::sleep(std::time::Duration::from_millis(attempt * 2));
                rename_result = fs::rename(&tmp_path, path);
                if rename_result.is_ok() {
                    break;
                }
            }
        }

        if let Err(e) = rename_result {
            let _ = fs::remove_file(&tmp_path);
            return Err(e);
        }

        Ok(())
    }

    /// Loads session state from the specified file path.
    pub fn load_from_path(path: &Path) -> std::io::Result<Self> {
        let content = fs::read_to_string(path)?;
        Self::from_json(&content).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Loads session state from path or falls back to defaults, quarantining corrupted JSON.
    pub fn load_or_default_from_path(path: &Path) -> Self {
        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(path) {
            Ok(content) => match Self::from_json(&content) {
                Ok(mut state) => {
                    state.clamp();
                    state
                }
                Err(err) => {
                    eprintln!(
                        "[KRONOS WARN] Corrupt session at {:?}: {}. Quarantining and resetting defaults.",
                        path, err
                    );
                    quarantine_corrupt_file(path);
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    /// Saves state to the default resolved session path.
    pub fn save(&self) -> std::io::Result<()> {
        let path = resolve_session_path();
        self.save_to_path(&path)
    }

    /// Loads state from the default resolved session path.
    pub fn load() -> Self {
        let path = resolve_session_path();
        Self::load_or_default_from_path(&path)
    }
}

/// Quarantines a malformed session file with a timestamp and removes the corrupt original from disk.
pub fn quarantine_corrupt_file(path: &Path) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let backup_path = path.with_extension(format!("corrupt.{}.json", timestamp));

    if fs::rename(path, &backup_path).is_err() && fs::copy(path, &backup_path).is_ok() {
        let _ = fs::remove_file(path);
    }
}

/// Resolves the application data directory for KRONOS DESCODE.
pub fn get_appdata_dir() -> PathBuf {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        PathBuf::from(appdata).join("kronos_descode")
    } else if let Some(userprofile) = std::env::var_os("USERPROFILE") {
        PathBuf::from(userprofile)
            .join("AppData")
            .join("Roaming")
            .join("kronos_descode")
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".config").join("kronos_descode")
    } else {
        std::env::temp_dir().join("kronos_descode")
    }
}

/// Returns the canonical path to the session persistence file.
pub fn get_session_file_path() -> PathBuf {
    get_appdata_dir().join("session.json")
}

/// Resolves session path checking KRONOS_SESSION_PATH env var override first.
pub fn resolve_session_path() -> PathBuf {
    if let Ok(override_val) = std::env::var("KRONOS_SESSION_PATH") {
        let trimmed = override_val.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    get_session_file_path()
}

pub fn load_session() -> SessionState {
    SessionState::load()
}

pub fn save_session(state: &SessionState) -> std::io::Result<()> {
    state.save()
}

/// User application configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    pub gemini_api_key: String,
    pub selected_model: String,
    pub opt_normalize_vars: bool,
    pub opt_remove_dead_code: bool,
    pub opt_decode_strings: bool,
    pub opt_hide_comments: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            gemini_api_key: crate::core::DEFAULT_GEMINI_API_KEY.to_string(),
            selected_model: "gemini-3.1-flash-lite".to_string(),
            opt_normalize_vars: true,
            opt_remove_dead_code: true,
            opt_decode_strings: true,
            opt_hide_comments: false,
        }
    }
}

pub fn load_config() -> AppConfig {
    let path = get_appdata_dir().join("config.json");
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(cfg) = serde_json::from_str(&content) {
            return cfg;
        }
    }
    AppConfig::default()
}

pub fn save_config(cfg: &AppConfig) -> std::io::Result<()> {
    let path = get_appdata_dir().join("config.json");
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let content = serde_json::to_string_pretty(cfg)
        .map_err(std::io::Error::other)?;
    fs::write(path, content)
}

/// Helper calculating adjacent tab index when closing a tab.
pub fn calculate_adjacent_active_tab_index(
    close_idx: usize,
    cur_active: usize,
    remaining_len: usize,
) -> Option<usize> {
    if remaining_len == 0 {
        return None;
    }
    let new_active = if close_idx < cur_active {
        cur_active.saturating_sub(1)
    } else if close_idx == cur_active {
        if close_idx >= remaining_len {
            remaining_len.saturating_sub(1)
        } else {
            close_idx
        }
    } else {
        cur_active
    };
    Some(new_active.min(remaining_len - 1))
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_default_values() {
        let default_state = SessionState::default();
        assert_eq!(default_state.window_width, 1280);
        assert_eq!(default_state.window_height, 800);
        assert!(!default_state.window_maximized);
        assert_eq!(default_state.context_panel_width, 260.0);
        assert_eq!(default_state.ai_panel_width, 340.0);
        assert_eq!(default_state.active_tool, "explorer");
        assert!(default_state.context_panel_visible);
        assert!(default_state.ai_panel_visible);
        assert_eq!(default_state.active_tab_index, -1);
        assert_eq!(default_state.active_model, "gemini-3.1-flash-lite");
        assert_eq!(default_state.active_mode, "deobfuscate");
        assert!(default_state.recent_projects.is_empty());
    }

    #[test]
    fn test_session_state_json_roundtrip() {
        let mut original = SessionState::default();
        original.window_width = 1920;
        original.window_height = 1080;
        original.window_maximized = true;
        original.context_panel_width = 300.0;
        original.ai_panel_width = 400.0;
        original.active_tool = "reverse".to_string();
        original.active_tab_index = 1;
        original.open_tabs = vec![
            TabSessionItem {
                id: 1,
                title: "script.py".to_string(),
                file_path: Some("C:\\project\\script.py".to_string()),
                tab_type: 0,
                is_dirty: false,
            },
            TabSessionItem {
                id: 2,
                title: "diff_view".to_string(),
                file_path: None,
                tab_type: 1,
                is_dirty: true,
            },
        ];
        original.recent_projects = vec!["C:\\proj_a".to_string(), "C:\\proj_b".to_string()];
        original.active_project_path = Some("C:\\proj_a".to_string());
        original.active_model = "gemini-3.8-flash".to_string();

        let json = original.to_json().expect("Serialization must succeed");
        let restored = SessionState::from_json(&json).expect("Deserialization must succeed");

        assert_eq!(original, restored);
    }

    #[test]
    fn test_session_state_save_and_load_file() {
        let temp_dir = std::env::temp_dir().join("kronos_test_session_io");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        let session_file = temp_dir.join("test_session.json");

        let mut state = SessionState::default();
        state.active_tool = "ast".to_string();
        state.context_panel_width = 310.0;

        // Save
        state.save_to_path(&session_file).expect("Save must succeed");
        assert!(session_file.exists());

        // Load
        let loaded = SessionState::load_from_path(&session_file).expect("Load must succeed");
        assert_eq!(loaded.active_tool, "ast");
        assert_eq!(loaded.context_panel_width, 310.0);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_session_state_corrupt_json_fallback() {
        let temp_dir = std::env::temp_dir().join("kronos_test_corrupt_session");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        let session_file = temp_dir.join("corrupt_session.json");

        // Write corrupt JSON
        fs::write(&session_file, "{ malformed: json, missing closing").unwrap();

        // Safe loader must fallback to default without panicking
        let loaded = SessionState::load_or_default_from_path(&session_file);
        assert_eq!(loaded.window_width, 1280);
        assert_eq!(loaded.active_tool, "explorer");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_session_state_partial_fields_backward_compatibility() {
        // Simulates an older session file missing newly added fields
        let partial_json = r#"{
            "window_width": 1600,
            "window_height": 900
        }"#;

        let state = SessionState::from_json(partial_json)
            .expect("Should deserialize partial JSON with serde defaults");
        assert_eq!(state.window_width, 1600);
        assert_eq!(state.window_height, 900);
        assert_eq!(state.context_panel_width, 260.0); // Defaulted
        assert_eq!(state.active_tool, "explorer");   // Defaulted
        assert_eq!(state.active_model, "gemini-3.1-flash-lite"); // Defaulted
    }

    #[test]
    fn test_navigation_mode_indexing() {
        for idx in 0..=6 {
            let mode = NavigationMode::from_index(idx);
            assert_eq!(mode.to_index(), idx);
        }
        assert_eq!(NavigationMode::from_index(99), NavigationMode::Explorer);
        assert_eq!(NavigationMode::from_index(-1), NavigationMode::Explorer);
    }

    #[test]
    fn test_tab_type_indexing() {
        for raw in 0..=3 {
            let t = TabType::from_raw(raw);
            assert_eq!(t.to_raw(), raw);
        }
        assert_eq!(TabType::from_raw(999), TabType::SourceFile);
    }

    #[test]
    fn test_env_path_override() {
        let custom_test_path = std::env::temp_dir().join("kronos_custom_env_override.json");
        std::env::set_var("KRONOS_SESSION_PATH", custom_test_path.to_string_lossy().as_ref());

        let resolved = resolve_session_path();
        assert_eq!(resolved, custom_test_path);

        std::env::remove_var("KRONOS_SESSION_PATH");
    }

    #[test]
    fn test_calculate_adjacent_active_tab_index() {
        // Closing tab before active -> active shifts left
        assert_eq!(calculate_adjacent_active_tab_index(0, 2, 4), Some(1));
        // Closing tab after active -> active stays same
        assert_eq!(calculate_adjacent_active_tab_index(3, 1, 4), Some(1));
        // Closing active tab at end -> active shifts to new last tab
        assert_eq!(calculate_adjacent_active_tab_index(2, 2, 2), Some(1));
        // Closing only tab -> None
        assert_eq!(calculate_adjacent_active_tab_index(0, 0, 0), None);
    }
}
