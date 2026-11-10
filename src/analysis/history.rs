use serde::{Deserialize, Serialize};

/// Detailed log entry of a reverse engineering / deobfuscation operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisEntry {
    pub id: u64,
    pub timestamp: String,
    pub filename: String,
    pub input_hash: String,
    pub output_hash: String,
    pub strings_decoded: usize,
    pub constants_folded: usize,
    pub variables_renamed: usize,
    pub dead_branches_pruned: usize,
    pub security_alerts: usize,
    pub is_safe: bool,
    pub status_badge: String,
}

/// In-memory ring buffer recording analysis history.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalysisHistory {
    pub entries: Vec<AnalysisEntry>,
    pub max_entries: usize,
}

impl AnalysisHistory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries: if max_entries == 0 { 50 } else { max_entries },
        }
    }

    pub fn record(&mut self, entry: AnalysisEntry) {
        self.entries.insert(0, entry);
        if self.entries.len() > self.max_entries {
            self.entries.truncate(self.max_entries);
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Line metrics comparing original and transformed code.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiffMetrics {
    pub total_lines_original: usize,
    pub total_lines_transformed: usize,
    pub changed_lines: usize,
}

/// Computes basic line difference metrics between original and transformed code.
pub fn compute_diff_metrics(orig: &str, transformed: &str) -> DiffMetrics {
    let orig_lines: Vec<&str> = orig.lines().collect();
    let trans_lines: Vec<&str> = transformed.lines().collect();

    let mut changed = 0;
    let min_len = orig_lines.len().min(trans_lines.len());
    for i in 0..min_len {
        if orig_lines[i] != trans_lines[i] {
            changed += 1;
        }
    }
    changed += orig_lines.len().abs_diff(trans_lines.len());

    DiffMetrics {
        total_lines_original: orig_lines.len(),
        total_lines_transformed: trans_lines.len(),
        changed_lines: changed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_history_record_and_limit() {
        let mut history = AnalysisHistory::new(2);
        let entry1 = AnalysisEntry {
            id: 1,
            timestamp: "2026-11-09T19:00:00Z".to_string(),
            filename: "sample1.py".to_string(),
            input_hash: "hash1".to_string(),
            output_hash: "hash1_out".to_string(),
            strings_decoded: 3,
            constants_folded: 1,
            variables_renamed: 4,
            dead_branches_pruned: 1,
            security_alerts: 0,
            is_safe: true,
            status_badge: "SEGURO".to_string(),
        };
        let mut entry2 = entry1.clone();
        entry2.id = 2;
        let mut entry3 = entry1.clone();
        entry3.id = 3;

        history.record(entry1);
        history.record(entry2);
        assert_eq!(history.entries.len(), 2);
        assert_eq!(history.entries[0].id, 2);

        history.record(entry3);
        assert_eq!(history.entries.len(), 2);
        assert_eq!(history.entries[0].id, 3);
        assert_eq!(history.entries[1].id, 2);

        history.clear();
        assert!(history.entries.is_empty());
    }

    #[test]
    fn test_compute_diff_metrics() {
        let orig = "line 1\nline 2\nline 3";
        let trans = "line 1\nmodified 2\nline 3\nline 4";
        let metrics = compute_diff_metrics(orig, trans);
        assert_eq!(metrics.total_lines_original, 3);
        assert_eq!(metrics.total_lines_transformed, 4);
        assert_eq!(metrics.changed_lines, 2); // 1 modified + 1 added
    }
}
