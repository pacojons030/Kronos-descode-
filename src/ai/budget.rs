//! Token Budgeting, Heuristic Estimation, and Context Windowing Engine.

/// Heuristic token estimator for source code and AST dumps (~3.5 characters per token).
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    (text.len() * 2) / 7 + 1
}

/// Truncation configuration for prompt context budgets.
#[derive(Debug, Clone)]
pub struct ContextBudget {
    pub max_total_tokens: usize,
    pub max_code_tokens: usize,
    pub max_ast_summary_tokens: usize,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            max_total_tokens: 32_000,
            max_code_tokens: 24_000,
            max_ast_summary_tokens: 4_000,
        }
    }
}

/// Smart head-and-tail code windowing with explicit AST truncation notice.
pub fn budget_code_context(code: &str, max_tokens: usize) -> (String, bool) {
    let estimated = estimate_tokens(code);
    if estimated <= max_tokens {
        return (code.to_string(), false);
    }

    let lines: Vec<&str> = code.lines().collect();
    if lines.len() <= 200 {
        let max_chars = max_tokens * 7 / 2;
        let cut_idx = match code.char_indices().nth(max_chars) {
            Some((idx, _)) => idx,
            None => code.len(),
        };
        let truncated = format!(
            "{}\n# [KRONOS AST TRUNCATION: Código truncado a {} caracteres para respetar límite de contexto]",
            &code[..cut_idx], max_chars
        );
        return (truncated, true);
    }

    // Head-and-tail windowing: 70% head, 30% tail
    let target_lines = (max_tokens * 7 / 2) / 45; // Assuming avg 45 chars per line
    let head_count = (target_lines * 7) / 10;
    let tail_count = target_lines.saturating_sub(head_count);

    let head = &lines[..head_count.min(lines.len())];
    let tail_start = lines.len().saturating_sub(tail_count);
    let tail = &lines[tail_start..];

    let omitted_count = tail_start.saturating_sub(head_count);

    let mut budgeted = head.join("\n");
    budgeted.push_str(&format!(
        "\n\n# =========================================================================\n\
         # [KRONOS AST TRUNCATION: {} líneas intermedias omitidas por presupuesto de tokens]\n\
         # =========================================================================\n\n",
        omitted_count
    ));
    budgeted.push_str(&tail.join("\n"));

    (budgeted, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        let sample = "def calculate_hash(data):\n    return sha256(data)";
        let tokens = estimate_tokens(sample);
        assert!(tokens > 10 && tokens < 30);
    }

    #[test]
    fn test_budget_code_context_within_limit() {
        let code = "print('hello world')";
        let (res, truncated) = budget_code_context(code, 1000);
        assert!(!truncated);
        assert_eq!(res, code);
    }

    #[test]
    fn test_budget_code_context_large_head_and_tail() {
        let mut lines = Vec::new();
        for i in 0..500 {
            lines.push(format!("x_{} = {}", i, i));
        }
        let code = lines.join("\n");
        let (res, truncated) = budget_code_context(&code, 50);
        assert!(truncated);
        assert!(res.contains("KRONOS AST TRUNCATION"));
        assert!(res.contains("x_0 = 0"));
        assert!(res.contains("x_499 = 499"));
    }
}
