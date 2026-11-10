//! Specialized Prompt Engineering Templates and Shannon Entropy Calculation for KRONOS AI.

pub const KRONOS_SYSTEM_PROMPT: &str =
    "Eres KRONOS AI, el motor de inteligencia artificial de élite especializado en ingeniería inversa, \
    desofuscación de bytecode, análisis de AST y auditoría de seguridad en KRONOS DESCODE.\n\
    \n\
    REGLAS DE OPERACIÓN:\n\
    1. Responde de forma directa, rigurosa, profesional y concisa en español. Sin introducciones superfluas ni cortesías.\n\
    2. PROTOCOLOS DE DESOFUSCACIÓN (OBLIGATORIO CUANDO SE PIDA DESOFUSCAR O LIMPIAR CÓDIGO):\n\
       - Normalización Semántica: Renombra TODO identificador en alfabeto griego (α, β, γ, etc.), caracteres no-ASCII u ofuscados (a, b, _0x...) a nombres en inglés estándar descriptivos y coherentes con su propósito (ej. LoginSessionManager, AuthPacket, TokenHolder).\n\
       - Plegado Aritmético y de Predicados Opacos: Resuelve constantes de Allatori/Zelix como `Integer.reverse(-2147483648)` -> 1, `Integer.reverse(-1)` -> -1, `Integer.reverse(0)` -> 0, etc. Elimina ramas muertas (`if (false)`).\n\
       - Purga de Descompilación: Elimina artefactos de descompilador (`/* 20 */`, `/*     */`, `// Byte code:`, `<illegal opcode>`).\n\
       - Descifrado de Cadenas: Descifra o reemplaza matrices de cadenas cifradas (`String[] a`) por sus textos legibles correspondientes.\n\
       - Cero Errores y Cero Truncamiento: El código sugerido DEBE ser 100% completo, sintácticamente impecable, sin puntos suspensivos (...) y ejecutable/compilable.\n\
    3. Entrega siempre el código final encerrado en un bloque Markdown con el identificador del lenguaje correspondiente (ejemplo: ```java ... ``` o ```python ... ```).";

/// Detects the programming language of a file based on its extension or content keywords.
pub fn detect_source_language(filename: &str, content: &str) -> &'static str {
    let lower = filename.to_lowercase();
    if lower.ends_with(".java") {
        "java"
    } else if lower.ends_with(".py") || lower.ends_with(".pyw") {
        "python"
    } else if lower.ends_with(".js") || lower.ends_with(".mjs") {
        "javascript"
    } else if lower.ends_with(".ts") {
        "typescript"
    } else if lower.ends_with(".rs") {
        "rust"
    } else if lower.ends_with(".cs") {
        "csharp"
    } else if lower.ends_with(".cpp") || lower.ends_with(".c") || lower.ends_with(".h") {
        "cpp"
    } else if content.contains("package ")
        || content.contains("public class ")
        || content.contains("import java.")
    {
        "java"
    } else if content.contains("def ")
        || (content.contains("import ") && !content.contains(';'))
    {
        "python"
    } else if content.contains("fn ") && content.contains("let ") {
        "rust"
    } else {
        "java"
    }
}

/// Action 1: Explain code or selected function.
pub fn prompt_explain_code(
    snippet: &str,
    filename: &str,
    line_start: Option<usize>,
    line_end: Option<usize>,
) -> String {
    let loc_info = match (line_start, line_end) {
        (Some(s), Some(e)) => format!(" (Líneas {}-{})", s, e),
        _ => String::new(),
    };
    let lang = detect_source_language(filename, snippet);
    format!(
        "Acción: EXPLICACIÓN DE CÓDIGO\n\
        Archivo: {}{}\n\
        Lenguaje: {}\n\n\
        Código seleccionado:\n\
        ```{lang}\n\
        {}\n\
        ```\n\n\
        Instrucciones para el análisis:\n\
        1. Explica sucintamente el propósito funcional de este bloque.\n\
        2. Describe paso a paso las operaciones matemáticas, lógicas o de manipulación de datos.\n\
        3. Identifica entradas (parámetros/variables) y salidas (valores devueltos/efectos secundarios).\n\
        4. Si detectas mecanismos de evasión, antidebug o técnicas antianálisis, destácalos claramente.",
        filename, loc_info, lang, snippet
    )
}

/// Action 2: Comprehensive file analysis.
pub fn prompt_analyze_file(
    content: &str,
    filename: &str,
    static_audit_summary: Option<&str>,
) -> String {
    let audit_section = match static_audit_summary {
        Some(summary) if !summary.is_empty() => {
            format!("\nPre-auditoría estática KRONOS:\n{}\n", summary)
        }
        _ => String::new(),
    };
    let lang = detect_source_language(filename, content);
    format!(
        "Acción: ANÁLISIS INTEGRAL DE ARCHIVO\n\
        Archivo activo: {}\n\
        Lenguaje: {}\n\
        Tamaño: {} bytes, {} líneas\n\
        {}\n\
        Contenido del archivo:\n\
        ```{lang}\n\
        {}\n\
        ```\n\n\
        Instrucciones para el análisis:\n\
        1. Resumen de la arquitectura y objetivo principal del script/clase.\n\
        2. Auditoría de seguridad: Identifica llamadas a sinks sensibles (Runtime.exec, ProcessBuilder, eval, exec, subprocess, sockets, winreg, urllib).\n\
        3. Evaluación de riesgo: Clasifica el archivo como Benigno, Sospechoso o Malicioso con justificación técnica.\n\
        4. Dependencias e indicadores de red/persistencia detectados.",
        filename, lang, content.len(), content.lines().count(), audit_section, content
    )
}

/// Action 3: Interpret Abstract Syntax Tree structure.
pub fn prompt_interpret_ast(
    ast_dump: &str,
    source_reference: &str,
    filename: &str,
) -> String {
    let lang = detect_source_language(filename, source_reference);
    format!(
        "Acción: INTERPRETACIÓN DE ESTRUCTURA AST\n\
        Archivo: {}\n\
        Lenguaje: {}\n\n\
        Código fuente de referencia:\n\
        ```{lang}\n\
        {}\n\
        ```\n\n\
        Jerarquía AST observada:\n\
        ```text\n\
        {}\n\
        ```\n\n\
        Instrucciones para el análisis:\n\
        1. Traduce la jerarquía de nodos del AST al flujo de control y llamadas subyacente.\n\
        2. Explica la estructura de bifurcaciones condicionales, bucles y expresiones compuestas.\n\
        3. Identifica si existen construcciones anómalas en el árbol sintáctico (ej. aplanamiento de flujo de control, árboles sobredimensionados diseñados para agotar analizadores estáticos).",
        filename, lang, source_reference, ast_dump
    )
}

/// Action 4: Identify obfuscation techniques & entropy.
pub fn prompt_detect_obfuscation(
    code: &str,
    entropy_bits: f64,
    encoded_strings: usize,
    folded_constants: usize,
    renamed_vars: usize,
    filename: &str,
) -> String {
    let lang = detect_source_language(filename, code);
    format!(
        "Acción: IDENTIFICACIÓN DE OFUSCACIÓN Y ENTROPÍA\n\
        Archivo: {}\n\
        Lenguaje: {}\n\
        Telemetría estática KRONOS:\n\
        - Entropía de Shannon calculada: {:.3} bits/byte (Umbral cifrado/comprimido > 5.5)\n\
        - Cadenas codificadas detectadas: {}\n\
        - Constantes numéricas/lógicas plegadas: {}\n\
        - Variables con nombres anómalos: {}\n\n\
        Código a examinar:\n\
        ```{lang}\n\
        {}\n\
        ```\n\n\
        Instrucciones para el análisis:\n\
        1. Diagnostica las técnicas de ofuscación activas (sustitución de variables, Allatori/Zelix Unicode, Base64/Hex/XOR, control flow flattening, dynamic execution).\n\
        2. Interpreta la cifra de entropía ({:.3} bits/byte) y especifica si sugiere empaquetamiento o cifrado.\n\
        3. Clasifica la posible familia de ofuscador (ej. Allatori, Zelix KlassMaster, ProGuard, PyArmor o script personalizado).\n\
        4. Recomienda la estrategia óptima para neutralizar la ofuscación.",
        filename, lang, entropy_bits, encoded_strings, folded_constants, renamed_vars, code, entropy_bits
    )
}

/// Action 5: Suggest deobfuscation and reconstruction pipeline.
pub fn prompt_suggest_reconstruction(
    original_code: &str,
    pre_transformed_code: &str,
    filename: &str,
) -> String {
    let lang = detect_source_language(filename, original_code);
    format!(
        "Acción: SUGERENCIA DE PIPELINE Y RECONSTRUCCIÓN (PROTOCOLO KRONOS)\n\
        Archivo: {}\n\
        Lenguaje detectado: {}\n\n\
        Código original ofuscado:\n\
        ```{lang}\n\
        {}\n\
        ```\n\n\
        Código pre-transformado por decodificadores estáticos KRONOS AST (Zero-Exec):\n\
        ```{lang}\n\
        {}\n\
        ```\n\n\
        Instrucciones para la desofuscación (PROTOCOLO KRONOS):\n\
        1. Renombra identificadores crípticos o en alfabeto griego a nombres semánticos idiomáticos en inglés.\n\
        2. Resuelve trampas de cálculo y flujo como Integer.reverse(-2147483648) -> 1, constantes plegadas y ramas muertas.\n\
        3. Elimina artefactos de descompilación (/* 20 */, // Byte code:, <illegal opcode>).\n\
        4. Genera la versión FINAL, 100% COMPLETA, SINTÁCTICAMENTE VÁLIDA Y SIN TRUNCAR en {}.\n\
        5. Encierra obligatoriamente el código completo en un bloque ```{lang} ... ``` para permitir su aplicación directa al editor.",
        filename, lang, original_code, pre_transformed_code, lang
    )
}

/// Computes the Shannon entropy of a byte slice in bits per byte (0.0 to 8.0).
pub fn compute_shannon_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut byte_counts = [0usize; 256];
    for &b in data {
        byte_counts[b as usize] += 1;
    }
    let total_len = data.len() as f64;
    let mut entropy = 0.0;
    for &count in &byte_counts {
        if count > 0 {
            let p = count as f64 / total_len;
            entropy -= p * p.log2();
        }
    }
    entropy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_explain_code_with_selection() {
        let snippet = "def decode_key(k): return bytes.fromhex(k)";
        let prompt = prompt_explain_code(snippet, "cipher.py", Some(10), Some(15));
        assert!(prompt.contains("cipher.py (Líneas 10-15)"));
        assert!(prompt.contains("EXPLICACIÓN DE CÓDIGO"));
        assert!(prompt.contains(snippet));
    }

    #[test]
    fn test_prompt_analyze_file_with_audit() {
        let content = "import os\nos.system('calc.exe')";
        let audit = "ALERTA: Llamada a os.system detectada en línea 2";
        let prompt = prompt_analyze_file(content, "payload.py", Some(audit));
        assert!(prompt.contains("ANÁLISIS INTEGRAL DE ARCHIVO"));
        assert!(prompt.contains("payload.py"));
        assert!(prompt.contains("Pre-auditoría estática KRONOS"));
        assert!(prompt.contains("calc.exe"));
    }

    #[test]
    fn test_prompt_interpret_ast() {
        let source = "x = a + b";
        let ast = "Assign(targets=[Name(id='x')], value=BinOp(left=Name(id='a'), op=Add(), right=Name(id='b')))";
        let prompt = prompt_interpret_ast(ast, source, "math.py");
        assert!(prompt.contains("INTERPRETACIÓN DE ESTRUCTURA AST"));
        assert!(prompt.contains("BinOp"));
        assert!(prompt.contains("x = a + b"));
    }

    #[test]
    fn test_prompt_detect_obfuscation_with_entropy() {
        let code = "x = 'aGVsbG8gd29ybGQ='";
        let prompt = prompt_detect_obfuscation(code, 5.821, 1, 0, 1, "packed.py");
        assert!(prompt.contains("IDENTIFICACIÓN DE OFUSCACIÓN Y ENTROPÍA"));
        assert!(prompt.contains("5.821 bits/byte"));
        assert!(prompt.contains("Cadenas codificadas detectadas: 1"));
    }

    #[test]
    fn test_prompt_suggest_reconstruction() {
        let orig = "var_1 = base64.b64decode('aGVsbG8=')";
        let pre_trans = "cadena_decodificada_1 = 'hello'";
        let prompt = prompt_suggest_reconstruction(orig, pre_trans, "stage1.py");
        assert!(prompt.contains("SUGERENCIA DE PIPELINE Y RECONSTRUCCIÓN"));
        assert!(prompt.contains("cadena_decodificada_1 = 'hello'"));
    }

    #[test]
    fn test_shannon_entropy_calculation() {
        let uniform: Vec<u8> = (0..=255).collect();
        let ent_uniform = compute_shannon_entropy(&uniform);
        assert!((ent_uniform - 8.0).abs() < 0.001);

        let constant = vec![b'A'; 1000];
        let ent_const = compute_shannon_entropy(&constant);
        assert_eq!(ent_const, 0.0);

        let text = b"The quick brown fox jumps over the lazy dog. Python reverse engineering station.";
        let ent_text = compute_shannon_entropy(text);
        assert!(ent_text > 4.0 && ent_text < 5.0);

        let b64 = b"YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXoxMjM0NTY3ODkw+/==";
        let ent_b64 = compute_shannon_entropy(b64);
        assert!(ent_b64 > 5.0);
    }
}
