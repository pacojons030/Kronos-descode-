#![allow(clippy::useless_vec)]

#[path = "../src/core/mod.rs"]
pub mod core;

use core::decoders::*;
use core::normalizer::*;
use core::pipeline::*;

#[test]
fn test_fuzz_chr_extreme_inputs() {
    // 1. Extreme numbers
    let cases = vec![
        "chr(0)",
        "chr(0x0)",
        "chr(0x10FFFF)",
        "chr(0x110000)", // Out of range for char (> 0x10FFFF)
        "chr(9999999999999999999999999999999999)", // u32 overflow
        "chr(0xFFFFFFFFFFFFFFFFFFFFFF)", // u32 overflow hex
        "chr(0xD800)", // UTF-16 surrogate half
        "chr(0xDFFF)", // UTF-16 surrogate half
        "chr(0x20)", // space
        "chr(10)", // newline
        "chr(13)", // carriage return
        "chr()", // empty
        "chr(   )", // spaces
        "chr(-1)", // negative (should not match regex)
        "chr(0xG)", // invalid hex
    ];

    for case in cases {
        let (res, _count) = decode_chr_calls(case);
        assert!(!res.is_empty(), "Result should not be empty for input: {}", case);
    }
}

#[test]
fn test_fuzz_xor_comprehensions_adversarial() {
    let cases = vec![
        // Normal XOR
        r#"" " .join([chr(x ^ 42) for x in [105, 70, 75]])"#,
        // Single quotes
        r#"'' .join([chr(x ^ 0x2A) for x in [105, 70, 75]])"#,
        // Invalid chars (> 0x10FFFF after XOR)
        r#"" " .join([chr(x ^ 0) for x in [0x110000, 100]])"#,
        // u32 overflow in key
        r#"" " .join([chr(x ^ 99999999999999999999) for x in [100, 200]])"#,
        // u32 overflow in list items
        r#"" " .join([chr(x ^ 42) for x in [99999999999999999999, 100]])"#,
        // Empty list
        r#"" " .join([chr(x ^ 42) for x in []])"#,
        // Surrogates after XOR
        r#"" " .join([chr(x ^ 0) for x in [0xD800, 0xDFFF]])"#,
        // Trailing comma
        r#"" " .join([chr(x ^ 42) for x in [105, 70, ]])"#,
    ];

    for case in cases {
        let (res, _count) = decode_xor_comprehensions(case);
        assert!(!res.is_empty());
    }
}

#[test]
fn test_fuzz_fold_string_concatenation_adversarial() {
    // Test 1: High depth chaining
    let mut chained = String::from("\"a\"");
    for _ in 0..1000 {
        chained.push_str(" + \"b\"");
    }
    let (res, count) = fold_string_concatenations(&chained);
    assert_eq!(count, 1000);
    assert!(res.starts_with("\"ab"));

    // Test 2: Single quotes chaining
    let mut single_chained = String::from("'x'");
    for _ in 0..500 {
        single_chained.push_str(" + 'y'");
    }
    let (res2, count2) = fold_string_concatenations(&single_chained);
    assert_eq!(count2, 500);
    assert!(res2.starts_with("'xy"));

    // Test 3: Pathological combinations
    let adversarial = vec![
        "\"\" + \"\"",
        "\"\" + \"\" + \"\"",
        "\"a\" + \"\" + \"b\"",
        "\"\"\" + \"\"\"",
        "'' + ''",
        "\"'\" + \"'\"",
        "'+' + '+'",
    ];
    for case in adversarial {
        let (res, _) = fold_string_concatenations(case);
        assert!(!res.is_empty());
    }
}

#[test]
fn test_fuzz_hex_bytes_adversarial() {
    let cases = vec![
        r#"bytes.fromhex("").decode("utf-8")"#,
        r#"bytes.fromhex("41").decode("utf-8")"#, // odd length: 1 char
        r#"bytes.fromhex("41424").decode("utf-8")"#, // odd length: 5 chars
        r#"bytes.fromhex("GG").decode("utf-8")"#, // not hex
        r#"bytes.fromhex("FF FE").decode("utf-8")"#, // invalid UTF-8 sequence
        r#"bytes.fromhex("00").decode("utf-8")"#, // null byte
        r#"bytes.fromhex("   48   65   6c   6c   6f   ").decode("utf-8")"#, // valid with spaces
        r#"bytes.fromhex("f09f918d").decode("utf-8")"#, // 4-byte UTF-8 emoji 👍
        r#"bytes.fromhex("d0b0d0b1d0b2").decode("utf-8")"#, // 2-byte Cyrillic абв
    ];

    for case in cases {
        let (res, _) = decode_hex_bytes(case);
        assert!(!res.is_empty());
    }
}

#[test]
fn test_fuzz_base64_adversarial() {
    let cases = vec![
        r#"base64.b64decode(b"").decode("utf-8")"#,
        r#"base64.b64decode(b"===").decode("utf-8")"#, // corrupt padding
        r#"base64.b64decode(b"!!!!").decode("utf-8")"#, // invalid base64 chars
        r#"base64.b64decode(b"//5/").decode("utf-8")"#, // invalid UTF-8
        r#"base64.b64decode(b"AA==").decode("utf-8")"#, // null byte \0
        r#"base64.b64decode(b"8J+RjQ==").decode("utf-8")"#, // emoji 👍
        r#"base64.standard_b64decode(b"SGVsbG8=").decode("utf-8")"#, // standard_b64decode
        r#"base64.b64decode("SGVsbG8=").decode("utf-8")"#, // without 'b' prefix
        r#"base64.b64decode(b"SGVsbG8=")"#, // without .decode()
    ];

    for case in cases {
        let (res, _) = decode_base64_calls(case);
        assert!(!res.is_empty());
    }
}

#[test]
fn test_fuzz_arithmetic_constants_parentheses() {
    let cases = vec![
        "(10 + 20)",
        "(100 - 50)",
        "(50 * 2)",
        "(100 // 2)",
        "(100 // 0)", // Division by zero guard
        "(255 ^ 170)",
        "(255 & 15)",
        "(240 | 15)",
        "(0 * 99999999)",
        "(9223372036854775807 + 0)", // i64::MAX
        "(9223372036854775807 * 2)", // i64 overflow in multiplication (should wrap)
        "(9223372036854775807 + 1)", // i64 overflow in addition (should wrap)
    ];

    for case in cases {
        let (res, _) = fold_arithmetic_constants(case);
        assert!(!res.is_empty(), "Failed on case: {}", case);
    }
}

#[test]
fn test_fuzz_arithmetic_constants_re_chain_integer_overflow() {
    // Testing the '=' assignment chain:
    // Regex: r"=\s*(\d+)\s*(\+|\-)\s*(\d+)(?:\s*(\+|\-)\s*(\d+))?"
    let cases = vec![
        "x = 10 + 20",
        "x = 100 - 40",
        "x = 10 + 20 - 5",
        "x = 10 - 20 + 5",
        "x = 0 + 0",
        // Extreme values near i64::MAX
        "x = 9223372036854775806 + 1",
    ];

    for case in cases {
        let (res, count) = fold_arithmetic_constants(case);
        assert!(!res.is_empty());
        assert!(count > 0, "Expected folded count > 0 for {}", case);
    }

    // Now test that i64::MAX + 1 wraps safely without panic in DeobfuscatorEngine
    let panic_engine_result = std::panic::catch_unwind(|| {
        let config = DeobfuscationConfig::default();
        DeobfuscatorEngine::process("x = 9223372036854775807 + 1\n", &config)
    });
    
    assert!(
        panic_engine_result.is_ok(),
        "Expected DeobfuscatorEngine to succeed safely without panic on integer overflow bomb!"
    );
    let deob_res = panic_engine_result.unwrap();
    assert!(
        deob_res.code.contains("-9223372036854775808"),
        "i64::MAX + 1 must wrap cleanly to i64::MIN"
    );
}

#[test]
fn test_fuzz_static_list_indices_adversarial() {
    let cases = vec![
        // Normal list
        "arr = [\"alpha\", \"beta\", \"gamma\"]\nx = arr[1]",
        // Out of bounds index
        "arr = [\"alpha\"]\nx = arr[5]",
        // Huge index that might overflow usize or exceed
        "arr = [\"alpha\"]\nx = arr[999999999999999999999999999999999]",
        // Empty list
        "arr = []\nx = arr[0]",
        // Single item
        "arr = [\"single\"]\nx = arr[0]",
        // List with special characters
        "arr = [\"line\\nbreak\", \"tab\\tchar\", \"quotes\\\"inside\"]\nx = arr[0]",
    ];

    for case in cases {
        let (res, _) = resolve_static_list_indices(case);
        assert!(!res.is_empty());
    }
}

#[test]
fn test_fuzz_normalizer_adversarial() {
    let cases = vec![
        // Normal obfuscated IDs
        "_0x1234 = 1\n_Il1Il1 = 2\nIl1Il1Il1 = 3",
        // Identifiers that should NOT match (clean identifiers)
        "var_1 = 1\nnormal_variable = 2\n_private = 3\n__dunder__ = 4",
        // Repetitions
        "_0x1 + _0x1 + _0x1",
        // Unicode mixed
        "_0x1234_abc = 1",
        // Empty
        "",
        // Only special chars
        "!@#$%^&*()",
    ];

    for case in cases {
        let (res, _count, _pairs) = normalize_obfuscated_identifiers(case);
        assert_eq!(res.is_empty(), case.is_empty());
    }
}

#[test]
fn test_fuzz_pipeline_adversarial_syntax_bombs() {
    let config = DeobfuscationConfig::default();

    // 1. Guardrail test: Exceed 2 MB
    let huge_input = "a".repeat(2 * 1024 * 1024 + 10);
    let res_huge = DeobfuscatorEngine::process(&huge_input, &config);
    assert!(res_huge.security_audit.blocked_by_guardrail);
    assert!(res_huge.code.contains("[GUARDRAIL BLOCK]"));

    // 2. Guardrail test: Nesting depth > 128
    let mut bomb_nesting = String::new();
    for _ in 0..130 {
        bomb_nesting.push('(');
    }
    bomb_nesting.push_str("1 + 1");
    for _ in 0..130 {
        bomb_nesting.push(')');
    }
    let res_nesting = DeobfuscatorEngine::process(&bomb_nesting, &config);
    assert!(res_nesting.security_audit.blocked_by_guardrail);
    assert!(res_nesting.code.contains("[GUARDRAIL BLOCK]"));

    // 3. Unbalanced brackets (should NOT block or underflow)
    let unbalanced = "))))]]]]}}}}";
    let res_unbalanced = DeobfuscatorEngine::process(unbalanced, &config);
    assert!(!res_unbalanced.security_audit.blocked_by_guardrail);

    // 4. Extreme Unicode / UTF-8
    let unicode_input = "# \u{1F600} \u{0000} \u{FFFF} \u{10FFFF} \n_0x1234 = chr(65)";
    let res_unicode = DeobfuscatorEngine::process(unicode_input, &config);
    assert!(!res_unicode.security_audit.blocked_by_guardrail);
    assert!(res_unicode.code.contains("\"A\""));

    // 5. Zero-length input
    let res_empty = DeobfuscatorEngine::process("", &config);
    assert!(!res_empty.security_audit.blocked_by_guardrail);
}

#[test]
fn test_pipeline_determinism_multi_pass() {
    let sample = r#"
_0x1234 = chr(72) + chr(101) + chr(108) + chr(108) + chr(111)
_0x5678 = bytes.fromhex("576f726c64").decode("utf-8")
_0x9abc = base64.b64decode(b"VGVzdA==").decode("utf-8")
_Il1Il1Il = (100 * 2) + (50 // 2)
arr = ["apple", "banana"]
selected = arr[0]
if False:
    print("dead")
else:
    print(_0x1234 + " " + _0x5678)
"#;

    let config = DeobfuscationConfig::default();
    let baseline = DeobfuscatorEngine::process(sample, &config);

    // Run 50 times and verify bit-for-bit identical output
    for run in 1..=50 {
        let candidate = DeobfuscatorEngine::process(sample, &config);
        assert_eq!(
            baseline.code, candidate.code,
            "Non-deterministic code output on run {}",
            run
        );
        assert_eq!(
            baseline.strings_decoded, candidate.strings_decoded,
            "Non-deterministic strings_decoded on run {}",
            run
        );
        assert_eq!(
            baseline.constants_folded, candidate.constants_folded,
            "Non-deterministic constants_folded on run {}",
            run
        );
        assert_eq!(
            baseline.variables_renamed, candidate.variables_renamed,
            "Non-deterministic variables_renamed on run {}",
            run
        );
        assert_eq!(
            baseline.dead_branches_pruned, candidate.dead_branches_pruned,
            "Non-deterministic dead_branches_pruned on run {}",
            run
        );
        assert_eq!(
            baseline.security_audit.input_hash, candidate.security_audit.input_hash,
            "Non-deterministic input hash on run {}",
            run
        );
        assert_eq!(
            baseline.security_audit.output_hash, candidate.security_audit.output_hash,
            "Non-deterministic output hash on run {}",
            run
        );
    }
}
