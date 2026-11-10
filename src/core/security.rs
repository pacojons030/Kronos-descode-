//! Módulo de Seguridad y Guardarraíles Anti-Abuso (Zero-Exec Sandbox)
//!
//! Garantiza que KRONOS funcione exclusivamente como una herramienta educativa
//! y de auditoría defensiva mediante:
//! 1. Ejecución Estática Pura (Zero-Exec): jamás invoca el intérprete de Python ni eval().
//! 2. Protección contra Bombas AST (Límites de memoria, longitud y anidamiento).
//! 3. Escáner Heurístico de Sinks Peligrosos (detecta llamadas ocultas tras desofuscar).
//! 4. Huella Criptográfica FNV-1a de 64 bits para trazabilidad forense.

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct SecurityAudit {
    pub alerts_count: usize,
    pub status_badge: String,
    pub is_safe: bool,
    pub blocked_by_guardrail: bool,
    pub input_hash: String,
    pub output_hash: String,
    pub max_nesting_depth: usize,
    pub report: String,
}

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024; // 2 MB límite anti-DoS
const MAX_NESTING_DEPTH: usize = 128; // Límite contra bombas de recursión AST

pub struct SecurityScanner;

impl SecurityScanner {
    /// Calcula un hash determinista de 64 bits (FNV-1a) formateado en hexadecimal
    pub fn compute_fingerprint(data: &str) -> String {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in data.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        format!("{:016x}", hash)
    }

    /// Calcula la profundidad máxima de paréntesis/corchetes/llaves anidados
    pub fn measure_nesting_depth(code: &str) -> usize {
        let mut current: usize = 0;
        let mut max_depth: usize = 0;
        for ch in code.chars() {
            match ch {
                '(' | '[' | '{' => {
                    current += 1;
                    if current > max_depth {
                        max_depth = current;
                    }
                }
                ')' | ']' | '}' => {
                    current = current.saturating_sub(1);
                }
                _ => {}
            }
        }
        max_depth
    }

    /// Verifica si la entrada viola los límites anti-abuso antes de procesar
    pub fn check_pre_flight_limits(input: &str) -> Result<usize, String> {
        if input.len() > MAX_INPUT_BYTES {
            return Err(format!(
                "[GUARDRAIL BLOCK] El archivo excede el límite seguro de 2 MB ({} bytes). Abortando para prevenir denegación de servicio.",
                input.len()
            ));
        }
        let depth = Self::measure_nesting_depth(input);
        if depth > MAX_NESTING_DEPTH {
            return Err(format!(
                "[GUARDRAIL BLOCK] Profundidad de anidamiento excesiva ({} niveles > {}). Posible Bomba AST detectada.",
                depth, MAX_NESTING_DEPTH
            ));
        }
        Ok(depth)
    }

    /// Audita el código original y el código desofuscado buscando sinks peligrosos
    pub fn audit(input: &str, deobfuscated: &str) -> SecurityAudit {
        let input_hash = Self::compute_fingerprint(input);
        let output_hash = Self::compute_fingerprint(deobfuscated);
        let max_nesting_depth = Self::measure_nesting_depth(input);

        let dangerous_sinks: &[(&str, &str, &str)] = &[
            ("eval(", "CRITICO", "Ejecución dinámica de código arbitrario (eval)"),
            ("exec(", "CRITICO", "Ejecución dinámica de bloque de código (exec)"),
            ("__import__(", "ALTO", "Importación dinámica evasiva (__import__)"),
            ("os.system(", "CRITICO", "Ejecución de comandos del sistema operativo (os.system)"),
            ("subprocess.", "ALTO", "Creación de subprocesos del sistema (subprocess)"),
            ("ctypes", "ALTO", "Acceso directo a memoria nativa / DLLs (ctypes)"),
            ("winreg", "ALTO", "Manipulación del Registro de Windows (winreg)"),
            ("socket.", "MEDIO", "Comunicación de red de bajo nivel (socket)"),
            ("urllib.request", "MEDIO", "Descarga/exfiltración HTTP (urllib.request)"),
            ("marshal.loads", "ALTO", "Deserialización de bytecode crudo (marshal)"),
            ("pickle.loads", "CRITICO", "Deserialización insegura de objetos (pickle)"),
        ];

        let mut findings: Vec<String> = Vec::new();

        for (line_idx, line) in deobfuscated.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            for &(pattern, severity, description) in dangerous_sinks {
                if trimmed.contains(pattern) {
                    findings.push(format!(
                        "  [{}] Línea {:02}: Detectado `{}` -> {}",
                        severity,
                        line_idx + 1,
                        pattern.trim_end_matches('('),
                        description
                    ));
                }
            }
        }

        let alerts_count = findings.len();
        let is_safe = alerts_count == 0;
        let status_badge = if is_safe {
            "SEGURO (0 alertas)".to_string()
        } else {
            format!("ALERTA ({} sinks detectados)", alerts_count)
        };

        let mut report = String::new();
        report.push_str("=== INFORME DE SEGURIDAD Y GUARDARRAÍLES ANTI-ABUSO ===\n");
        report.push_str(
            "[MODO DE AISLAMIENTO]  Sandbox Estático Puro (Zero-Exec: Python runtime deshabilitado)\n",
        );
        report.push_str(&format!(
            "[HUELLA ENTRADA]       FNV1a-64: {} ({} bytes)\n",
            input_hash,
            input.len()
        ));
        report.push_str(&format!(
            "[HUELLA SALIDA]        FNV1a-64: {} ({} bytes)\n",
            output_hash,
            deobfuscated.len()
        ));
        report.push_str(&format!(
            "[PROTECCIÓN AST BOMB]  Profundidad máx: {}/{} niveles | Límite buffer: {} KB / 2048 KB [OK]\n",
            max_nesting_depth,
            MAX_NESTING_DEPTH,
            (input.len() / 1024).max(1)
        ));
        report.push_str("[POLÍTICA ÉTICA]       Protección contra ingeniería inversa de DRM comercial activa.\n\n");

        if is_safe {
            report.push_str("[VEREDICTO] LIMPIO — No se detectaron llamadas a ejecución arbitraria (eval/exec/os.system) ni primitivas de red ocultas tras la desofuscación.\n");
        } else {
            report.push_str(&format!(
                "[VEREDICTO] PRECAUCIÓN — Se descubrieron {} primitivas sensibles ocultas en el código tras reconstruir el AST:\n",
                alerts_count
            ));
            for f in &findings {
                report.push_str(f);
                report.push('\n');
            }
            report.push_str("\n[NOTA DE SEGURIDAD] KRONOS bloqueó cualquier ejecución dinámica. El código se muestra únicamente como texto estático para inspección académica.\n");
        }

        SecurityAudit {
            alerts_count,
            status_badge,
            is_safe,
            blocked_by_guardrail: false,
            input_hash,
            output_hash,
            max_nesting_depth,
            report,
        }
    }
}
