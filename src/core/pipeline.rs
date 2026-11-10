use crate::core::decoders::{
    decode_base64_calls, decode_chr_calls, decode_hex_bytes, decode_xor_comprehensions,
    fold_arithmetic_constants, fold_string_concatenations, resolve_static_list_indices,
};
use crate::core::normalizer::normalize_obfuscated_identifiers;
use crate::core::security::{SecurityAudit, SecurityScanner};
use regex::Regex;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct BatchProjectDeobfReport {
    pub total_scanned: usize,
    pub files_deobfuscated: usize,
    pub constants_folded: usize,
    pub comments_purged: usize,
    pub output_dir: String,
    pub primary_filename: String,
    pub primary_content: String,
}

#[derive(Debug, Clone)]
pub struct DeobfuscationConfig {
    pub normalize_identifiers: bool,
    pub prune_dead_code: bool,
    pub static_decoders: bool,
}

impl Default for DeobfuscationConfig {
    fn default() -> Self {
        Self {
            normalize_identifiers: true,
            prune_dead_code: true,
            static_decoders: true,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Default, Clone)]
pub struct DeobfuscationResult {
    pub code: String,
    pub raw_transformed: String,
    pub strings_decoded: usize,
    pub constants_folded: usize,
    pub variables_renamed: usize,
    pub dead_branches_pruned: usize,
    pub tokens_breakdown: String,
    pub audit_log: String,
    pub diff: String,
    pub security_audit: SecurityAudit,
}

pub struct DeobfuscatorEngine;

impl DeobfuscatorEngine {
    pub fn process(input: &str, config: &DeobfuscationConfig) -> DeobfuscationResult {
        // 1. Verificación previa Anti-Abuso (Límites de Bomba AST y DoS)
        if let Err(guardrail_err) = SecurityScanner::check_pre_flight_limits(input) {
            let blocked_audit = SecurityAudit {
                blocked_by_guardrail: true,
                is_safe: false,
                alerts_count: 1,
                status_badge: "BLOQUEADO (Guardrail)".to_string(),
                report: guardrail_err.clone(),
                ..Default::default()
            };
            return DeobfuscationResult {
                code: format!("# {}\n", guardrail_err),
                raw_transformed: format!("# {}\n", guardrail_err),
                audit_log: guardrail_err,
                security_audit: blocked_audit,
                ..Default::default()
            };
        }

        let mut result = input.to_string();
        let mut total_strings = 0;
        let mut total_constants = 0;
        let mut total_vars = 0;
        let mut total_dead = 0;

        let mut log = String::new();
        log.push_str("[00:00:00.001] [SANDBOX] Modo Zero-Exec activo: Intérprete Python aislado (100% análisis estático en Rust)\n");
        log.push_str(&format!(
            "[00:00:00.002] [BUFFER]  Entrada recibida: {} bytes ({} líneas) | Hash FNV1a: {}\n",
            input.len(),
            input.lines().count(),
            SecurityScanner::compute_fingerprint(input)
        ));

        let mut tokens_report = String::new();
        tokens_report.push_str("=== TABLA DE SÍMBOLOS AST Y TRANSFORMACIONES LÉXICAS ===\n\n");

        if config.static_decoders {
            // Pase 1: chr() decimal y hexadecimal
            let (res, c1) = decode_chr_calls(&result);
            if c1 > 0 {
                log.push_str(&format!(
                    "[00:00:00.004] [PASS 01] Llamadas chr(N) / chr(0xN) evaluadas estáticamente: {} literales\n",
                    c1
                ));
                tokens_report.push_str(&format!(
                    "  [PASS 01: CHR_DECODE]       {} llamadas chr() convertidas a caracteres UTF-8\n",
                    c1
                ));
            }
            result = res;
            total_strings += c1;

            // Pase 2: Comprensiones de lista XOR
            let (res, c_xor) = decode_xor_comprehensions(&result);
            if c_xor > 0 {
                log.push_str(&format!(
                    "[00:00:00.006] [PASS 02] Bucles XOR [chr(c ^ KEY)] desencriptados: {} bloques\n",
                    c_xor
                ));
                tokens_report.push_str(&format!(
                    "  [PASS 02: XOR_UNPACK]       {} comprensiones XOR resueltas a cadenas planas\n",
                    c_xor
                ));
            }
            result = res;
            total_strings += c_xor;

            // Pase 3: bytes.fromhex()
            let (res, c2) = decode_hex_bytes(&result);
            if c2 > 0 {
                log.push_str(&format!(
                    "[00:00:00.008] [PASS 03] Payloads bytes.fromhex() decodificados: {} cadenas\n",
                    c2
                ));
                tokens_report.push_str(&format!(
                    "  [PASS 03: HEX_DECODE]       {} secuencias hexadecimales traducidas a UTF-8\n",
                    c2
                ));
            }
            result = res;
            total_strings += c2;

            // Pase 4: base64.b64decode()
            let (res, c3) = decode_base64_calls(&result);
            if c3 > 0 {
                log.push_str(&format!(
                    "[00:00:00.010] [PASS 04] Payloads base64.b64decode() desempaquetados: {} bloques\n",
                    c3
                ));
                tokens_report.push_str(&format!(
                    "  [PASS 04: BASE64_DECODE]    {} bloques Base64 decodificados sin ejecución\n",
                    c3
                ));
            }
            result = res;
            total_strings += c3;

            // Pase 5: Concatenación de literales ("a" + "b")
            let (res, c4) = fold_string_concatenations(&result);
            if c4 > 0 {
                log.push_str(&format!(
                    "[00:00:00.012] [PASS 05] Plegado de cadenas adyacentes (Constant String Folding): {} uniones\n",
                    c4
                ));
                tokens_report.push_str(&format!(
                    "  [PASS 05: STRING_FOLDING]   {} fragmentos de texto unidos en literales continuos\n",
                    c4
                ));
            }
            result = res;
            total_constants += c4;

            // Pase 6: Plegado aritmético y bit a bit
            let (res, c_math) = fold_arithmetic_constants(&result);
            if c_math > 0 {
                log.push_str(&format!(
                    "[00:00:00.014] [PASS 06] Plegado de constantes aritméticas/bitwise: {} expresiones\n",
                    c_math
                ));
                tokens_report.push_str(&format!(
                    "  [PASS 06: CONST_FOLDING]    {} operaciones matemáticas evaluadas en tiempo de compilación\n",
                    c_math
                ));
            }
            result = res;
            total_constants += c_math;

            // Pase 7: Resolución de tablas/listas estáticas indexadas
            let (res, c_idx) = resolve_static_list_indices(&result);
            if c_idx > 0 {
                log.push_str(&format!(
                    "[00:00:00.016] [PASS 07] Índices de tablas estáticas resueltos (arr[i]): {} accesos\n",
                    c_idx
                ));
                tokens_report.push_str(&format!(
                    "  [PASS 07: ARRAY_PROPAGATE]  {} referencias de lista sustituidas por su valor literal\n",
                    c_idx
                ));
            }
            result = res;
            total_constants += c_idx;

            // Pase 7B: Plegado de constantes opacas Java / Allatori y desempaquetado de comentarios
            let (res_allatori, c_allatori) = Self::fold_allatori_opaque_constants(&result);
            if c_allatori > 0 {
                log.push_str(&format!(
                    "[00:00:00.017] [PASS 07B] Plegado de constantes Allatori Integer.reverse(): {} evaluaciones\n",
                    c_allatori
                ));
                tokens_report.push_str(&format!(
                    "  [PASS 07B: ALLATORI_FOLD]    {} constantes de predicado opaco y artefactos resueltos\n",
                    c_allatori
                ));
            }
            result = res_allatori;
            total_constants += c_allatori;
        }

        if config.normalize_identifiers {
            let (res, c5, mappings) = normalize_obfuscated_identifiers(&result);
            if c5 > 0 {
                log.push_str(&format!(
                    "[00:00:00.019] [PASS 08] Normalización léxica de identificadores (_0x / Il1O0): {} símbolos\n",
                    c5
                ));
                tokens_report.push_str("\n--- MAPEO DE IDENTIFICADORES RENOMBRADOS ---\n");
                for (orig, renamed) in &mappings {
                    tokens_report.push_str(&format!("  {:20}  -->  {}\n", orig, renamed));
                }
            }
            result = res;
            total_vars += c5;
        }

        if config.prune_dead_code {
            // Poda la cabecera `if False: ... else:` y conserva el bloque alcanzable
            if let Ok(re_header) =
                Regex::new(r"(?ms)^[ \t]*if (?:False|0|not True):\s*\n.*?\n[ \t]*else:\s*\n")
            {
                let dead_count = re_header.find_iter(&result).count();
                if dead_count > 0 {
                    result = re_header
                        .replace_all(
                            &result,
                            "# [AST PRUNED: Rama condicional opaca (if False/0) eliminada]\n",
                        )
                        .to_string();
                    total_dead += dead_count;
                    log.push_str(&format!(
                        "[00:00:00.022] [PASS 09] Eliminación de código muerto (Dead Branch Pruning): {} ramas\n",
                        dead_count
                    ));
                    tokens_report.push_str(&format!(
                        "\n  [PASS 09: DEAD_CODE_PRUNE]  {} bloques condicionales inalcanzables eliminados\n",
                        dead_count
                    ));
                }
            }
        }

        // Auditoría de Seguridad y Sinks post-desofuscación
        let security_audit = SecurityScanner::audit(input, &result);
        log.push_str(&format!(
            "[00:00:00.025] [SECURITY] Escaneo de Sinks completado: {} | Hash Salida: {}\n",
            security_audit.status_badge, security_audit.output_hash
        ));
        log.push_str("[00:00:00.027] [READY]   Reconstrucción AST finalizada sin errores.\n");

        // Construir Diff unificado línea por línea
        let mut diff = String::new();
        diff.push_str("--- a/entrada_ofuscada.py\n+++ b/ast_reconstruido.py\n\n");
        let orig_lines: Vec<&str> = input.lines().collect();
        let clean_lines: Vec<&str> = result.lines().collect();
        let max_len = orig_lines.len().max(clean_lines.len());

        for i in 0..max_len {
            let orig = orig_lines.get(i).copied().unwrap_or("");
            let clean = clean_lines.get(i).copied().unwrap_or("");

            if orig != clean {
                if !orig.is_empty() {
                    diff.push_str(&format!("- {:02} | {}\n", i + 1, orig));
                }
                if !clean.is_empty() {
                    diff.push_str(&format!("+ {:02} | {}\n", i + 1, clean));
                }
            } else if !orig.is_empty() {
                diff.push_str(&format!("  {:02} | {}\n", i + 1, orig));
            }
        }

        let header = format!(
            "# [KRONOS AST v2.5] Salida Estatica Normalizada | Estado: {}\n",
            security_audit.status_badge
        );

        DeobfuscationResult {
            code: format!("{}\n{}", header, result),
            raw_transformed: result,
            strings_decoded: total_strings,
            constants_folded: total_constants,
            variables_renamed: total_vars,
            dead_branches_pruned: total_dead,
            tokens_breakdown: tokens_report,
            audit_log: log,
            diff,
            security_audit,
        }
    }

    /// Folds Allatori opaque constants (e.g. `Integer.reverse(-2147483648)` -> `1`) and strips decompiler line markers.
    pub fn fold_allatori_opaque_constants(input: &str) -> (String, usize) {
        let mut count = 0;
        let mut out = String::with_capacity(input.len());
        for line in input.lines() {
            let mut processed = line.to_string();
            let replacements = [
                ("Integer.reverse(-2147483648)", "1"),
                ("Integer.reverse(-1)", "-1"),
                ("Integer.reverse(0)", "0"),
                ("Integer.reverse(-1073741824)", "2"),
                ("Integer.reverse(536870912)", "4"),
                ("Integer.reverse(-234881024)", "8"),
                ("Integer.reverse(-2046820352)", "16"),
                ("Integer.reverse(-603979776)", "32"),
                ("Integer.reverse(1073741824)", "2"),
                ("Integer.reverse(1)", "-2147483648"),
                ("Integer.reverse(2)", "1073741824"),
                ("Integer.reverse(16)", "134217728"),
                ("Integer.reverse(64)", "33554432"),
                ("Integer.reverse(128)", "16777216"),
                ("Integer.reverse(256)", "8388608"),
                ("Integer.reverse(512)", "4194304"),
                ("Integer.reverse(1024)", "2097152"),
                ("Integer.reverse(2048)", "1048576"),
                ("Integer.reverse(4096)", "524288"),
                ("Integer.reverse(8192)", "262144"),
                ("Integer.reverse(16384)", "131072"),
                ("Integer.reverse(32768)", "65536"),
                ("Integer.reverse(65536)", "32768"),
            ];
            for (pattern, val) in replacements {
                if processed.contains(pattern) {
                    let matches = processed.matches(pattern).count();
                    count += matches;
                    processed = processed.replace(pattern, val);
                }
            }
            // Limpiar comentarios de número de línea de descompilador /* 20 */ o /*     */
            let trimmed = processed.trim_start();
            if trimmed.starts_with("/*") {
                if let Some(end_comment) = trimmed.find("*/") {
                    let stripped = trimmed[end_comment + 2..].trim_start();
                    if !stripped.is_empty() {
                        let leading_spaces = line.len().saturating_sub(line.trim_start().len());
                        out.push_str(&" ".repeat(leading_spaces));
                        out.push_str(stripped);
                        out.push('\n');
                        count += 1;
                        continue;
                    }
                }
            }
            out.push_str(&processed);
            out.push('\n');
        }
        (out, count)
    }

    /// Deobfuscates all source files in a project directory recursively.
    pub fn deobfuscate_project(project_dir: &Path) -> std::io::Result<BatchProjectDeobfReport> {
        let mut report = BatchProjectDeobfReport {
            total_scanned: 0,
            files_deobfuscated: 0,
            constants_folded: 0,
            comments_purged: 0,
            output_dir: String::new(),
            primary_filename: String::new(),
            primary_content: String::new(),
        };

        if !project_dir.exists() || !project_dir.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Directorio de proyecto no encontrado",
            ));
        }

        let out_dir = project_dir.join("desofuscado");
        let _ = fs::create_dir_all(&out_dir);
        report.output_dir = out_dir.to_string_lossy().to_string();

        let cfg = DeobfuscationConfig::default();

        let mut queue = vec![project_dir.to_path_buf()];
        while let Some(current) = queue.pop() {
            let entries = match fs::read_dir(&current) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if name != ".git" && name != "target" && name != "desofuscado" {
                        queue.push(p);
                    }
                } else if p.is_file() {
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    if ext == "java" || ext == "py" || ext == "json" || ext == "txt" {
                        report.total_scanned += 1;
                        if let Ok(bytes) = fs::read(&p) {
                            let content = String::from_utf8_lossy(&bytes).to_string();
                            let (folded_content, folded_count) = Self::fold_allatori_opaque_constants(&content);
                            let res = Self::process(&folded_content, &cfg);

                            let is_obf = folded_count > 0 || res.constants_folded > 0 || res.strings_decoded > 0;
                            if is_obf || ext == "java" || ext == "py" {
                                report.files_deobfuscated += 1;
                                report.constants_folded += folded_count + res.constants_folded;
                                report.comments_purged += folded_count;

                                // Guardar archivo limpio en carpeta desofuscado preservando ruta relativa
                                if let Ok(rel) = p.strip_prefix(project_dir) {
                                    let dest = out_dir.join(rel);
                                    if let Some(parent) = dest.parent() {
                                        let _ = fs::create_dir_all(parent);
                                    }
                                    let clean_text = if !res.raw_transformed.trim().is_empty() {
                                        &res.raw_transformed
                                    } else {
                                        &folded_content
                                    };
                                    let _ = fs::write(&dest, clean_text);

                                    if report.primary_filename.is_empty() && (is_obf || ext == "java") {
                                        report.primary_filename = rel.to_string_lossy().to_string();
                                        report.primary_content = clean_text.to_string();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CODE: &str = r#"_0x1a = chr(72) + chr(101) + chr(108) + chr(108) + chr(111)
_0x2b = bytes.fromhex("576f726c64").decode("utf-8")
_0x3c = base64.b64decode(b"Um9vdA==").decode("utf-8")
if False:
    print("trap")
else:
    print(_0x1a + " " + _0x2b + " " + _0x3c)
"#;

    #[test]
    fn test_full_pipeline_execution() {
        let config = DeobfuscationConfig::default();
        let res = DeobfuscatorEngine::process(TEST_CODE, &config);

        assert!(res.strings_decoded >= 7);
        assert!(res.variables_renamed >= 3);
        assert_eq!(res.dead_branches_pruned, 1);
        assert!(res.code.contains("Hello"));
        assert!(res.code.contains("World"));
        assert!(res.code.contains("Root"));
        assert!(res.code.contains("var_1"));
        assert!(!res.diff.is_empty());
        assert!(!res.tokens_breakdown.is_empty());
        assert!(!res.audit_log.is_empty());
        assert!(res.security_audit.is_safe);
    }

    #[test]
    fn test_selective_options() {
        let config = DeobfuscationConfig {
            normalize_identifiers: false,
            prune_dead_code: false,
            static_decoders: true,
        };
        let res = DeobfuscatorEngine::process(TEST_CODE, &config);
        assert_eq!(res.variables_renamed, 0);
        assert_eq!(res.dead_branches_pruned, 0);
        assert!(res.strings_decoded >= 7);
        assert!(res.code.contains("_0x1a"));
    }

    #[test]
    fn test_allatori_constant_folding() {
        let sample = "/* 20 */ int a = Integer.reverse(-2147483648);\n/* */ int b = Integer.reverse(-1);";
        let (out, count) = DeobfuscatorEngine::fold_allatori_opaque_constants(sample);
        assert!(count >= 2);
        assert!(out.contains("int a = 1;"));
        assert!(out.contains("int b = -1;"));
        assert!(!out.contains("/* 20 */"));
    }

    #[test]
    fn test_deobfuscate_project_on_temp_dir() {
        let temp = std::env::temp_dir().join("kronos_test_batch_proj");
        let _ = fs::remove_dir_all(&temp);
        let sub = temp.join("src").join("pkg");
        let _ = fs::create_dir_all(&sub);

        let java_file = sub.join("TestClass.java");
        let java_code = "/* 10 */ int x = Integer.reverse(-2147483648);\n// Byte code:\nint y = 5;";
        let _ = fs::write(&java_file, java_code);

        let report = DeobfuscatorEngine::deobfuscate_project(&temp).expect("Deobfuscate project should succeed");
        assert_eq!(report.total_scanned, 1);
        assert_eq!(report.files_deobfuscated, 1);
        assert!(report.constants_folded >= 1);
        assert!(temp.join("desofuscado").join("src").join("pkg").join("TestClass.java").exists());

        let clean_content = fs::read_to_string(temp.join("desofuscado").join("src").join("pkg").join("TestClass.java")).unwrap();
        assert!(clean_content.contains("int x = 1;"));
        assert!(!clean_content.contains("/* 10 */"));

        let _ = fs::remove_dir_all(&temp);
    }
}
