#![allow(
    clippy::field_reassign_with_default,
    clippy::useless_vec,
    clippy::manual_range_contains,
    clippy::vec_init_then_push,
    clippy::const_is_empty,
    clippy::unnecessary_literal_unwrap,
    clippy::collapsible_if
)]

#[path = "../src/core/mod.rs"]
pub mod core;

#[path = "../src/ai/mod.rs"]
pub mod ai;

use ai::budget::{budget_code_context, estimate_tokens, ContextBudget};
use ai::parser::{ConversationHistory, MarkdownCodeParser, MessageRole};
use ai::prompts::{
    compute_shannon_entropy, prompt_analyze_file, prompt_detect_obfuscation, prompt_explain_code,
    prompt_interpret_ast, prompt_suggest_reconstruction,
};

#[test]
fn test_challenge_conversation_history_limits_and_serialization() {
    let mut history = ConversationHistory::new(3); // 3 turns = 6 messages
    for i in 1..=10 {
        history.push(MessageRole::User, format!("Query {}", i), "12:00:00".into());
        history.push(MessageRole::Model, format!("Answer {}", i), "12:00:01".into());
    }
    assert_eq!(history.messages.len(), 6, "History must be pruned to exactly max_turns * 2 messages");
    assert_eq!(history.messages[0].content, "Query 8");
    assert_eq!(history.messages[5].content, "Answer 10");

    let gemini_json = history.to_gemini_contents();
    assert_eq!(gemini_json.len(), 6);
    assert_eq!(gemini_json[0]["role"], "user");
    assert_eq!(gemini_json[0]["parts"][0]["text"], "Query 8");
}

#[test]
fn test_challenge_budget_utf8_multibyte_boundary_safety() {
    let emoji_code = "🦀".repeat(50);
    // If lines.len() <= 200, cut_idx = max_tokens * 7 / 2.
    // Testing if budget_code_context panics or survives on multi-byte boundaries
    // Let's check with multiple token budgets: 1, 2, 5, 9, 10, 11, 15, 25
    for max_tok in [1, 2, 5, 9, 10, 11, 15, 25] {
        let (budgeted, truncated) = budget_code_context(&emoji_code, max_tok);
        assert!(truncated);
        assert!(!budgeted.is_empty());
    }
}

#[test]
fn test_challenge_mask_api_key_non_ascii() {
    use ai::key::mask_api_key;
    // An adversarial user or test could pass non-ASCII string as API key
    let non_ascii_key = "12345🦀7890123";
    let masked = mask_api_key(non_ascii_key);
    assert!(masked.contains("..."));
}




// =========================================================================
// CHALLENGE 1: MarkdownCodeParser
// =========================================================================

#[test]
fn test_challenge_markdown_multiblock_prefers_python() {
    let markdown_payload = r#"
Para iniciar el análisis, primero instala los paquetes auxiliares:
```bash
pip install capstone unicorn ropper
```

A continuación se presenta el payload de desofuscación completamente recuperado:
```python
import base64

def unpack_payload(obf_str: str) -> bytes:
    raw = base64.b64decode(obf_str)
    return bytes([b ^ 0x5A for b in raw])
```

Y la configuración estructurada resultante del análisis:
```json
{
  "target": "loader.exe",
  "arch": "x86_64",
  "status": "unpacked"
}
```
"#;

    // Verify all 3 blocks extracted
    let all_blocks = MarkdownCodeParser::extract_all_blocks(markdown_payload);
    assert_eq!(all_blocks.len(), 3, "Must extract exactly 3 code blocks");
    assert_eq!(all_blocks[0].language, "bash");
    assert_eq!(all_blocks[1].language, "python");
    assert_eq!(all_blocks[2].language, "json");

    // Verify select_primary_code_block explicitly selects the Python block
    let primary = MarkdownCodeParser::select_primary_code_block(markdown_payload);
    assert!(primary.is_some(), "Primary code block must not be None");
    let code = primary.unwrap();
    assert!(code.contains("def unpack_payload"), "Primary block must contain Python code");
    assert!(code.contains("import base64"), "Primary block must contain Python imports");
    assert!(!code.contains("pip install"), "Primary block must NOT be bash");
    assert!(!code.contains("\"target\": \"loader.exe\""), "Primary block must NOT be json");
}

#[test]
fn test_challenge_markdown_unclosed_block_safe_extraction() {
    // Simulating token exhaustion truncation by LLM
    let truncated_markdown = "Análisis en progreso:\n```python\ndef payload_decrypt(buf):\n    key = 0xAA\n    result = bytearray()";

    let blocks = MarkdownCodeParser::extract_all_blocks(truncated_markdown);
    assert_eq!(blocks.len(), 1, "Must extract the single unclosed block");
    assert_eq!(blocks[0].language, "python");
    assert!(!blocks[0].is_closed, "Block must be marked is_closed = false");
    assert!(blocks[0].code.contains("def payload_decrypt(buf):"));
    assert!(blocks[0].code.contains("result = bytearray()"));

    // Ensure select_primary_code_block extracts it safely without hanging or panic
    let primary = MarkdownCodeParser::select_primary_code_block(truncated_markdown);
    assert!(primary.is_some(), "Must extract primary block even if unclosed");
    let code = primary.unwrap();
    assert_eq!(code, "def payload_decrypt(buf):\n    key = 0xAA\n    result = bytearray()");
}

#[test]
fn test_challenge_markdown_no_code_blocks_returns_none() {
    let plain_responses = vec![
        "No hay ningún bloque de código en esta respuesta técnica.",
        "",
        "   \n\t  \n  ",
        "El archivo parece ser benigno. No se requiere desofuscación.",
        "`print('inline code only')`", // Inline code with single backtick is NOT a code block
    ];

    for text in plain_responses {
        let extracted = MarkdownCodeParser::select_primary_code_block(text);
        assert_eq!(extracted, None, "Plain text with no triple backticks must return None");

        let all_blocks = MarkdownCodeParser::extract_all_blocks(text);
        assert!(all_blocks.is_empty(), "extract_all_blocks must return empty vec for plain text");
    }
}

// =========================================================================
// CHALLENGE 2: budget_code_context
// =========================================================================

#[test]
fn test_challenge_budget_50000_line_script() {
    // Generate an enormous 50,000-line script
    let mut large_script = String::with_capacity(50_000 * 30);
    for i in 0..50_000 {
        large_script.push_str(&format!("var_{:05} = calculate_offset({})\n", i, i));
    }

    let raw_tokens = estimate_tokens(&large_script);
    assert!(raw_tokens > 200_000, "50k lines should exceed 200k estimated tokens");

    // Default code token budget is 24,000
    let max_budget_tokens = ContextBudget::default().max_code_tokens; // 24_000
    let (budgeted, was_truncated) = budget_code_context(&large_script, max_budget_tokens);

    assert!(was_truncated, "50,000-line script MUST be marked as truncated");
    assert!(budgeted.contains("KRONOS AST TRUNCATION"), "Must contain explicit KRONOS AST TRUNCATION banner");
    assert!(budgeted.contains("líneas intermedias omitidas por presupuesto de tokens"), "Banner must state omitted lines");

    // Verify head lines are present
    assert!(budgeted.contains("var_00000 = calculate_offset(0)"), "Must contain initial head code");
    assert!(budgeted.contains("var_00010 = calculate_offset(10)"), "Must contain head code");

    // Verify tail lines are present
    assert!(budgeted.contains("var_49999 = calculate_offset(49999)"), "Must contain final tail code");
    assert!(budgeted.contains("var_49990 = calculate_offset(49990)"), "Must contain tail code");

    // Verify token estimation of budgeted output stays within bounded limit
    let budgeted_tokens = estimate_tokens(&budgeted);
    // Budget target was 24,000 tokens; windowing heuristic target_lines = (max_tokens * 7 / 2) / 45
    // with 1866 lines generated. Check that budgeted tokens is close to budget (<= 1.2 * max_budget_tokens)
    assert!(
        budgeted_tokens <= max_budget_tokens * 12 / 10,
        "Budgeted tokens ({}) must stay within reasonable bounds of max_budget_tokens ({})",
        budgeted_tokens,
        max_budget_tokens
    );
}

// =========================================================================
// CHALLENGE 3: Shannon Entropy
// =========================================================================

#[test]
fn test_challenge_shannon_entropy_bounds_and_distributions() {
    // 1. Boundary: Empty slice -> 0.0
    let empty: &[u8] = &[];
    let h_empty = compute_shannon_entropy(empty);
    assert_eq!(h_empty, 0.0, "Empty data must have exactly 0.0 entropy");

    // 2. Boundary: Monotonous repeating single byte -> 0.0
    let zeroes = vec![0x00u8; 10000];
    let h_zeroes = compute_shannon_entropy(&zeroes);
    assert_eq!(h_zeroes, 0.0, "Monotonous 0x00 must have 0.0 entropy");

    let repeated_a = vec![b'A'; 5000];
    let h_a = compute_shannon_entropy(&repeated_a);
    assert_eq!(h_a, 0.0, "Repeating single character must have 0.0 entropy");

    // 3. Boundary: Maximum entropy uniform 256-byte distribution -> exactly 8.0
    let mut uniform = Vec::with_capacity(256 * 100);
    for _ in 0..100 {
        for b in 0..=255u8 {
            uniform.push(b);
        }
    }
    let h_uniform = compute_shannon_entropy(&uniform);
    assert!(
        (h_uniform - 8.0).abs() < 1e-6,
        "Uniform 256-byte distribution must equal 8.0 bits/byte, got {}",
        h_uniform
    );

    // 4. Plaintext English / Python code: typical range 3.0 .. 4.8
    let python_plaintext = b"import os\nimport sys\n\ndef main():\n    for i in range(100):\n        print(f'Processing line {i}')\n\nif __name__ == '__main__':\n    main()\n";
    let h_plain = compute_shannon_entropy(python_plaintext);
    assert!(
        h_plain >= 3.0 && h_plain <= 4.8,
        "Plaintext Python code entropy should be between 3.0 and 4.8, got {}",
        h_plain
    );

    // 5. Hex payload: alphabet of 16 characters -> theoretical max log2(16) = 4.0
    let hex_payload = b"48656c6c6f20576f726c6420546869732069732061206c6f6e672068657820737472696e6720666f7220656e74726f70792074657374696e6720646174612030313233343536373839616263646566";
    let h_hex = compute_shannon_entropy(hex_payload);
    assert!(
        h_hex <= 4.0 && h_hex > 2.5,
        "Hex payload entropy must be <= 4.0 and > 2.5, got {}",
        h_hex
    );

    // 6. High-entropy Base64 payload: alphabet of 64 characters -> theoretical max log2(64) = 6.0
    // Real obfuscated base64 payload
    let b64_payload = b"aW1wb3J0IG9zLCBzeXMsIHNvY2tldCwgdGhyZWFkaW5nLCBzdWJwcm9jZXNzCnMgPSBzb2NrZXQuc29ja2V0KHNvY2tldC5BRl9JTkVULCBzb2NrZXQuU09DS19TVFJFQU0pCnMuY29ubmVjdCgoJzE5Mi4xNjguMS4xMDAnLCA0NDQ0KSk=";
    let h_b64 = compute_shannon_entropy(b64_payload);
    assert!(
        h_b64 > 5.0 && h_b64 <= 6.0,
        "Base64 payload entropy should be in range (5.0, 6.0], got {}",
        h_b64
    );

    // 7. Pseudo-random / encrypted high entropy payload -> should be near 7.5 .. 8.0
    // LCX PRNG generating high entropy bytes
    let mut high_entropy = Vec::with_capacity(4096);
    let mut state: u64 = 0x123456789ABCDEF0;
    for _ in 0..4096 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        high_entropy.push((state >> 32) as u8);
    }
    let h_high = compute_shannon_entropy(&high_entropy);
    assert!(
        h_high > 7.8 && h_high <= 8.0,
        "Encrypted/PRNG high entropy payload should be > 7.8 and <= 8.0, got {}",
        h_high
    );

    // 8. Invariant test: across all cases, H must strictly be in [0.0, 8.0]
    let all_entropies = [h_empty, h_zeroes, h_a, h_uniform, h_plain, h_hex, h_b64, h_high];
    for &h in &all_entropies {
        assert!(h >= 0.0 && h <= 8.0, "Entropy {} outside invariant range [0.0, 8.0]", h);
        assert!(!h.is_nan(), "Entropy must not be NaN");
        assert!(!h.is_infinite(), "Entropy must not be infinite");
    }
}

// =========================================================================
// CHALLENGE 4: Contextual Prompt Templates
// =========================================================================

#[test]
fn test_challenge_contextual_prompt_templates() {
    // 1. Explain Code
    let prompt1 = prompt_explain_code("def xor(a, b): return [x ^ y for x, y in zip(a, b)]", "crypto.py", Some(12), Some(25));
    assert!(prompt1.contains("crypto.py (Líneas 12-25)"));
    assert!(prompt1.contains("```python\ndef xor"));
    assert!(prompt1.contains("Instrucciones para el análisis:"));

    // 2. Analyze File
    let prompt2 = prompt_analyze_file("import subprocess\nsubprocess.Popen(['powershell'])", "dropper.py", Some("SINK ALERT"));
    assert!(prompt2.contains("dropper.py"));
    assert!(prompt2.contains("Pre-auditoría estática KRONOS:\nSINK ALERT"));
    assert!(prompt2.contains("Tamaño: "));

    // 3. Interpret AST
    let prompt3 = prompt_interpret_ast("Module(body=[Expr(value=Call(...))])", "orig_code()", "main.py");
    assert!(prompt3.contains("main.py"));
    assert!(prompt3.contains("```text\nModule(body="));
    assert!(prompt3.contains("```python\norig_code()"));

    // 4. Detect Obfuscation
    let prompt4 = prompt_detect_obfuscation("a = 'x'", 5.92345, 3, 14, 55, "obf.py");
    assert!(prompt4.contains("5.923 bits/byte"));
    assert!(prompt4.contains("Cadenas codificadas detectadas: 3"));
    assert!(prompt4.contains("Constantes numéricas/lógicas plegadas: 14"));
    assert!(prompt4.contains("Variables con nombres anómalos: 55"));

    // 5. Suggest Reconstruction
    let prompt5 = prompt_suggest_reconstruction("raw_code", "pre_transformed", "packed.py");
    assert!(prompt5.contains("raw_code"));
    assert!(prompt5.contains("pre_transformed"));
    assert!(prompt5.contains("```python ... ```"));
}
