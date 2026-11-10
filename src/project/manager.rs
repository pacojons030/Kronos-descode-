use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Metadata describing an opened project workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub name: String,
    pub path: PathBuf,
    pub project_type: Option<String>,
    pub file_count: usize,
}

/// Detects the type of project based on configuration and build marker files.
pub fn detect_project_type(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    if path.join("requirements.txt").exists()
        || path.join("setup.py").exists()
        || path.join("pyproject.toml").exists()
    {
        return Some("Python".to_string());
    }
    if path.join("Cargo.toml").exists() {
        return Some("Rust".to_string());
    }
    if path.is_dir() {
        return Some("Generic".to_string());
    }
    None
}

/// Project lifecycle and recent workspace manager.
#[derive(Debug, Clone, Default)]
pub struct ProjectManager {
    pub active_project: Option<ProjectMetadata>,
    pub recent_projects: Vec<ProjectMetadata>,
    pub max_recent: usize,
}

impl ProjectManager {
    pub fn new(max_recent: usize) -> Self {
        Self {
            active_project: None,
            recent_projects: Vec::new(),
            max_recent: if max_recent == 0 { 10 } else { max_recent },
        }
    }

    pub fn open_project(&mut self, path: &Path) -> Result<ProjectMetadata, std::io::Error> {
        if !path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Path does not exist: {:?}", path),
            ));
        }

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("WORKSPACE")
            .to_string();
        let project_type = detect_project_type(path);
        let meta = ProjectMetadata {
            name,
            path: path.to_path_buf(),
            project_type,
            file_count: 0,
        };

        self.active_project = Some(meta.clone());
        self.recent_projects.retain(|p| p.path != meta.path);
        self.recent_projects.insert(0, meta.clone());
        if self.recent_projects.len() > self.max_recent {
            self.recent_projects.truncate(self.max_recent);
        }

        Ok(meta)
    }

    pub fn close_active_project(&mut self) {
        self.active_project = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_project_manager_open_and_active_state() {
        let temp_dir = std::env::temp_dir().join("kronos_test_project_open");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        fs::write(temp_dir.join("app.py"), "print('hello')").unwrap();

        let mut pm = ProjectManager::new(5);
        assert!(pm.active_project.is_none());

        let project = pm.open_project(&temp_dir).expect("Open project must succeed");
        assert_eq!(project.name, "kronos_test_project_open");
        assert_eq!(project.path, temp_dir);
        assert!(pm.active_project.is_some());
        assert_eq!(pm.recent_projects.len(), 1);
        assert_eq!(pm.recent_projects[0].path, temp_dir);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_project_manager_recent_projects_mru_and_dedup() {
        let temp_base = std::env::temp_dir().join("kronos_test_pm_mru");
        let _ = fs::remove_dir_all(&temp_base);

        let p1 = temp_base.join("project_1");
        let p2 = temp_base.join("project_2");
        let p3 = temp_base.join("project_3");
        fs::create_dir_all(&p1).unwrap();
        fs::create_dir_all(&p2).unwrap();
        fs::create_dir_all(&p3).unwrap();

        let mut pm = ProjectManager::new(5);
        pm.open_project(&p1).unwrap();
        pm.open_project(&p2).unwrap();
        pm.open_project(&p3).unwrap();

        // Currently MRU order: [p3, p2, p1]
        assert_eq!(pm.recent_projects[0].path, p3);
        assert_eq!(pm.recent_projects[1].path, p2);
        assert_eq!(pm.recent_projects[2].path, p1);

        // Re-open p1 -> must move to index 0 without duplicates
        pm.open_project(&p1).unwrap();
        assert_eq!(pm.recent_projects.len(), 3);
        assert_eq!(pm.recent_projects[0].path, p1);
        assert_eq!(pm.recent_projects[1].path, p3);
        assert_eq!(pm.recent_projects[2].path, p2);

        let _ = fs::remove_dir_all(&temp_base);
    }

    #[test]
    fn test_project_manager_recent_projects_capacity_limit() {
        let temp_base = std::env::temp_dir().join("kronos_test_pm_cap");
        let _ = fs::remove_dir_all(&temp_base);

        let mut pm = ProjectManager::new(3); // Cap at 3 recent projects

        for i in 0..5 {
            let p = temp_base.join(format!("proj_{}", i));
            fs::create_dir_all(&p).unwrap();
            pm.open_project(&p).unwrap();
        }

        // Cap must be strictly respected
        assert_eq!(pm.recent_projects.len(), 3);
        assert_eq!(pm.recent_projects[0].name, "proj_4");
        assert_eq!(pm.recent_projects[1].name, "proj_3");
        assert_eq!(pm.recent_projects[2].name, "proj_2");

        let _ = fs::remove_dir_all(&temp_base);
    }

    #[test]
    fn test_project_manager_detect_project_type() {
        let temp_dir = std::env::temp_dir().join("kronos_test_detect_type");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        // 1. Python project with requirements.txt
        fs::write(temp_dir.join("requirements.txt"), "requests\n").unwrap();
        assert_eq!(detect_project_type(&temp_dir), Some("Python".to_string()));

        // 2. Python project with setup.py
        fs::remove_file(temp_dir.join("requirements.txt")).unwrap();
        fs::write(temp_dir.join("setup.py"), "# setup").unwrap();
        assert_eq!(detect_project_type(&temp_dir), Some("Python".to_string()));

        // 3. Rust project with Cargo.toml
        fs::remove_file(temp_dir.join("setup.py")).unwrap();
        fs::write(temp_dir.join("Cargo.toml"), "[package]").unwrap();
        assert_eq!(detect_project_type(&temp_dir), Some("Rust".to_string()));

        // 4. Generic empty folder
        fs::remove_file(temp_dir.join("Cargo.toml")).unwrap();
        assert_eq!(detect_project_type(&temp_dir), Some("Generic".to_string()));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_project_manager_close_active_project() {
        let temp_dir = std::env::temp_dir().join("kronos_test_close_project");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let mut pm = ProjectManager::new(5);
        pm.open_project(&temp_dir).unwrap();
        assert!(pm.active_project.is_some());

        pm.close_active_project();
        assert!(pm.active_project.is_none());
        // Recent history is preserved
        assert_eq!(pm.recent_projects.len(), 1);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_project_manager_open_nonexistent_path_fails() {
        let mut pm = ProjectManager::new(5);
        let fake_path = PathBuf::from("C:\\kronos_fake_path_99999");

        let res = pm.open_project(&fake_path);
        assert!(res.is_err());
        assert!(pm.active_project.is_none());
        assert!(pm.recent_projects.is_empty());
    }
}
