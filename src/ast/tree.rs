use slint::{Color, ModelRc, SharedString, VecModel};
use std::rc::Rc;
use crate::SyntaxLine;
use crate::fs::split_code_lines;

/// Paleta oficial de resaltado sintáctico Visual Studio Code (Dark+ / Dark Modern)
pub fn col_comment() -> Color {
    Color::from_rgb_u8(0x6A, 0x99, 0x55) // #6A9955 Verde comentario
}

pub fn col_special_comment() -> Color {
    Color::from_rgb_u8(0x4E, 0xC9, 0xB0) // #4EC9B0 Turquesa directiva AST
}

pub fn col_keyword() -> Color {
    Color::from_rgb_u8(0xC5, 0x86, 0xC0) // #C586C0 Púrpura control (import, if, else, for, in)
}

pub fn col_bool_kw() -> Color {
    Color::from_rgb_u8(0x56, 0x9C, 0xD6) // #569CD6 Azul (False, True, None, not, def)
}

pub fn col_function() -> Color {
    Color::from_rgb_u8(0xDC, 0xDC, 0xAA) // #DCDCAA Amarillo función (print, chr, decode, b64decode)
}

pub fn col_module() -> Color {
    Color::from_rgb_u8(0x4E, 0xC9, 0xB0) // #4EC9B0 Turquesa clase/módulo (base64, bytes, os)
}

pub fn col_string() -> Color {
    Color::from_rgb_u8(0xCE, 0x91, 0x78) // #CE9178 Naranja cadena ("...")
}

pub fn col_number() -> Color {
    Color::from_rgb_u8(0xB5, 0xCE, 0xA8) // #B5CEA8 Verde salvia número (65, 0xDEADBEEF)
}

pub fn col_obf_var() -> Color {
    Color::from_rgb_u8(0xF4, 0x87, 0x71) // #F48771 Coral alerta para IDs ofuscados (_0x..., _Il1...)
}

pub fn col_norm_var() -> Color {
    Color::from_rgb_u8(0x4F, 0xC1, 0xFF) // #4FC1FF Azul brillante para var_1, var_2 normalizadas
}

pub fn col_ident() -> Color {
    Color::from_rgb_u8(0x9C, 0xDC, 0xFE) // #9CDCFE Azul claro variable estándar
}

pub fn col_punct() -> Color {
    Color::from_rgb_u8(0xD4, 0xD4, 0xD4) // #D4D4D4 Gris claro operadores y signos
}

/// Tokeniza una línea de Python en pares (Texto, Color VS Code Dark+)
pub fn tokenize_python_line(line: &str) -> Vec<(String, Color)> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        let color = if trimmed.contains("[KRONOS")
            || trimmed.contains("[AST PRUNED")
            || trimmed.contains("[GEMINI")
        {
            col_special_comment()
        } else {
            col_comment()
        };
        return vec![(line.to_string(), color)];
    }

    let mut raw_tokens: Vec<(String, Color)> = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut i = 0;

    while i < n {
        let c = chars[i];

        // 1. Espacios en blanco
        if c.is_whitespace() {
            let start = i;
            while i < n && chars[i].is_whitespace() {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            raw_tokens.push((s, col_punct()));
            continue;
        }

        // 2. Comentario al final de línea
        if c == '#' {
            let s: String = chars[i..].iter().collect();
            raw_tokens.push((s, col_comment()));
            break;
        }

        // 3. Prefijo de bytes b"..." o b'...'
        if (c == 'b' || c == 'B') && i + 1 < n && (chars[i + 1] == '"' || chars[i + 1] == '\'') {
            let quote = chars[i + 1];
            let start = i;
            i += 2;
            while i < n {
                if chars[i] == '\\' && i + 1 < n {
                    i += 2;
                    continue;
                }
                if chars[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            raw_tokens.push((s, col_string()));
            continue;
        }

        // 4. Literales de cadena "..." o '...'
        if c == '"' || c == '\'' {
            let quote = c;
            let start = i;
            i += 1;
            while i < n {
                if chars[i] == '\\' && i + 1 < n {
                    i += 2;
                    continue;
                }
                if chars[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            raw_tokens.push((s, col_string()));
            continue;
        }

        // 5. Números decimales o hexadecimales (0x...)
        if c.is_ascii_digit() {
            let start = i;
            if c == '0' && i + 1 < n && (chars[i + 1] == 'x' || chars[i + 1] == 'X') {
                i += 2;
                while i < n && chars[i].is_ascii_hexdigit() {
                    i += 1;
                }
            } else {
                while i < n && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
            }
            let s: String = chars[start..i].iter().collect();
            raw_tokens.push((s, col_number()));
            continue;
        }

        // 6. Identificadores y palabras clave
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < n && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let color = match word.as_str() {
                "import" | "from" | "as" | "if" | "else" | "elif" | "for" | "in" | "while"
                | "return" | "try" | "except" | "with" | "pass" | "break" | "continue" => {
                    col_keyword()
                }
                "False" | "True" | "None" | "and" | "or" | "not" | "def" | "class" | "lambda" => {
                    col_bool_kw()
                }
                "print" | "chr" | "ord" | "fromhex" | "b64decode" | "decode" | "join" | "eval"
                | "exec" | "system" | "len" | "range" => col_function(),
                "base64" | "bytes" | "os" | "sys" | "subprocess" | "socket" => col_module(),
                w if w.starts_with("_0x")
                    || w.starts_with("_0X")
                    || w.starts_with("_Il")
                    || w.starts_with("_I1")
                    || w.starts_with("_O0") =>
                {
                    col_obf_var()
                }
                w if w.starts_with("var_")
                    || w.starts_with("cadena_decodificada")
                    || w.starts_with("payload_hex")
                    || w.starts_with("usuario_b64")
                    || w.starts_with("rol_resuelto") =>
                {
                    col_norm_var()
                }
                _ => col_ident(),
            };
            raw_tokens.push((word, color));
            continue;
        }

        // 7. Operadores y puntuación
        let start = i;
        while i < n
            && !chars[i].is_whitespace()
            && !chars[i].is_ascii_alphanumeric()
            && chars[i] != '_'
            && chars[i] != '"'
            && chars[i] != '\''
            && chars[i] != '#'
        {
            i += 1;
        }
        let s: String = chars[start..i].iter().collect();
        raw_tokens.push((s, col_punct()));
    }

    // Fusionar tokens adyacentes del mismo color para caber en 12 slots
    let mut merged: Vec<(String, Color)> = Vec::new();
    for (text, color) in raw_tokens {
        if let Some((last_text, last_col)) = merged.last_mut() {
            if *last_col == color || text.chars().all(|ch| ch.is_whitespace()) {
                last_text.push_str(&text);
                continue;
            }
        }
        merged.push((text, color));
    }

    if merged.len() > 12 {
        let mut tail_text = String::new();
        let tail_color = merged[11].1;
        for (t, _) in merged.drain(11..) {
            tail_text.push_str(&t);
        }
        merged.push((tail_text, tail_color));
    }

    merged
}

/// Genera la columna de números de línea "1\n2\n3..." para el editor multilínea
pub fn build_line_numbers_text(code: &str) -> SharedString {
    let count = split_code_lines(code).len().max(1);
    let nums: Vec<String> = (1..=count).map(|n| n.to_string()).collect();
    SharedString::from(nums.join("\n"))
}

/// Convierte un bloque de código completo en un modelo Slint `[SyntaxLine]`
/// Soportando ocultación de comentarios (`hide_comments`) y filtrado por búsqueda (`search_query`)
pub fn build_syntax_lines(
    code: &str,
    is_output: bool,
    hide_comments: bool,
    search_query: &str,
) -> ModelRc<SyntaxLine> {
    build_syntax_lines_with_active(code, is_output, hide_comments, search_query, -1)
}

pub fn build_syntax_lines_with_active(
    code: &str,
    is_output: bool,
    hide_comments: bool,
    search_query: &str,
    active_editing_line: i32,
) -> ModelRc<SyntaxLine> {
    let mut lines_vec: Vec<SyntaxLine> = Vec::new();
    let query_lower = search_query.trim().to_lowercase();
    let raw_lines = split_code_lines(code);

    for (idx, line) in raw_lines.into_iter().enumerate() {
        let is_being_edited = (idx as i32) == active_editing_line;
        let trimmed = line.trim();
        let is_comment = trimmed.starts_with('#');

        if !is_being_edited && hide_comments && is_comment {
            continue;
        }

        if !is_being_edited
            && !query_lower.is_empty()
            && !line.to_lowercase().contains(&query_lower)
        {
            continue;
        }

        let diff_state = if !is_output {
            if trimmed.contains("chr(")
                || trimmed.contains("fromhex(")
                || trimmed.contains("b64decode(")
                || trimmed.contains("_0x")
                || trimmed.contains("_Il")
                || trimmed.contains("_I1")
                || trimmed.contains("_O0")
                || trimmed.starts_with("if False")
                || trimmed.starts_with("if 0:")
                || trimmed.starts_with("if not True")
            {
                1
            } else {
                0
            }
        } else if trimmed.starts_with("# [KRONOS") || trimmed.starts_with("# [GEMINI") {
            3
        } else if trimmed.starts_with("# [AST PRUNED")
            || trimmed.contains("var_")
            || (!trimmed.starts_with('#') && !trimmed.is_empty() && !trimmed.starts_with("import "))
        {
            2
        } else {
            0
        };

        let mut spans = tokenize_python_line(line);
        while spans.len() < 12 {
            spans.push((String::new(), col_punct()));
        }

        lines_vec.push(SyntaxLine {
            line_num: SharedString::from(format!("{}", idx + 1)),
            line_index: idx as i32,
            raw_text: SharedString::from(line),
            is_comment,
            diff_state,
            t1: SharedString::from(&spans[0].0),
            c1: spans[0].1,
            t2: SharedString::from(&spans[1].0),
            c2: spans[1].1,
            t3: SharedString::from(&spans[2].0),
            c3: spans[2].1,
            t4: SharedString::from(&spans[3].0),
            c4: spans[3].1,
            t5: SharedString::from(&spans[4].0),
            c5: spans[4].1,
            t6: SharedString::from(&spans[5].0),
            c6: spans[5].1,
            t7: SharedString::from(&spans[6].0),
            c7: spans[6].1,
            t8: SharedString::from(&spans[7].0),
            c8: spans[7].1,
            t9: SharedString::from(&spans[8].0),
            c9: spans[8].1,
            t10: SharedString::from(&spans[9].0),
            c10: spans[9].1,
            t11: SharedString::from(&spans[10].0),
            c11: spans[10].1,
            t12: SharedString::from(&spans[11].0),
            c12: spans[11].1,
        });
    }

    if lines_vec.is_empty() {
        lines_vec.push(SyntaxLine {
            line_num: SharedString::from("1"),
            line_index: 0,
            raw_text: SharedString::from(""),
            is_comment: false,
            diff_state: 0,
            t1: SharedString::from("# Sin coincidencias para el filtro actual"),
            c1: col_comment(),
            t2: SharedString::default(),
            c2: col_punct(),
            t3: SharedString::default(),
            c3: col_punct(),
            t4: SharedString::default(),
            c4: col_punct(),
            t5: SharedString::default(),
            c5: col_punct(),
            t6: SharedString::default(),
            c6: col_punct(),
            t7: SharedString::default(),
            c7: col_punct(),
            t8: SharedString::default(),
            c8: col_punct(),
            t9: SharedString::default(),
            c9: col_punct(),
            t10: SharedString::default(),
            c10: col_punct(),
            t11: SharedString::default(),
            c11: col_punct(),
            t12: SharedString::default(),
            c12: col_punct(),
        });
    }

    ModelRc::from(Rc::new(VecModel::from(lines_vec)))
}

/// Abstract Syntax Tree node representation for visual exploration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AstNode {
    pub id: usize,
    pub node_type: String,
    pub name: String,
    pub line_start: usize,
    pub line_end: usize,
    pub depth: usize,
    pub children: Vec<AstNode>,
}

/// Abstract Syntax Tree container.
#[derive(Debug, Clone, Default)]
pub struct AstTree {
    pub root: Option<AstNode>,
}

/// Flattened AST item for visual UI tree exploration.
#[derive(Debug, Clone)]
pub struct FlatAstEntry {
    pub id: i32,
    pub node_type: String,
    pub name: String,
    pub detail: String,
    pub line_number: i32,
    pub depth: i32,
    pub is_alert: bool,
}

/// Parses Python source code into a structured flat list of AST nodes for the AST Explorer.
pub fn parse_python_ast_flat(code: &str) -> Vec<FlatAstEntry> {
    let mut entries = Vec::new();
    let lines = split_code_lines(code);
    let total_lines = lines.len().max(1);

    entries.push(FlatAstEntry {
        id: 0,
        node_type: "Module".to_string(),
        name: "root".to_string(),
        detail: format!("{} líneas", total_lines),
        line_number: 1,
        depth: 0,
        is_alert: false,
    });

    let mut next_id = 1;
    for (idx, raw_line) in lines.iter().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let line_num = (idx + 1) as i32;
        let indent_spaces = raw_line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
        let depth = ((indent_spaces / 4) + 1).min(6) as i32;

        if trimmed.starts_with('#') {
            if trimmed.contains("[KRONOS") || trimmed.contains("[AST") || trimmed.contains("[GEMINI") {
                entries.push(FlatAstEntry {
                    id: next_id,
                    node_type: "Directive".to_string(),
                    name: trimmed.trim_start_matches('#').trim().chars().take(48).collect(),
                    detail: format!("L{}", line_num),
                    line_number: line_num,
                    depth,
                    is_alert: false,
                });
                next_id += 1;
            }
            continue;
        }

        let is_alert = trimmed.contains("eval(")
            || trimmed.contains("exec(")
            || trimmed.contains("os.system(")
            || trimmed.contains("subprocess")
            || trimmed.contains("__import__")
            || trimmed.contains("_0x")
            || trimmed.contains("b64decode")
            || trimmed.contains("fromhex");

        let (node_type, name, detail) = if let Some(rest) = trimmed.strip_prefix("import ") {
            ("Import".to_string(), rest.trim().to_string(), "módulo".to_string())
        } else if let Some(rest) = trimmed.strip_prefix("from ") {
            ("ImportFrom".to_string(), rest.trim().to_string(), "importación".to_string())
        } else if let Some(rest) = trimmed.strip_prefix("def ") {
            let fn_name = rest.split('(').next().unwrap_or(rest).trim();
            ("FunctionDef".to_string(), fn_name.to_string(), "función".to_string())
        } else if let Some(rest) = trimmed.strip_prefix("class ") {
            let cls_name = rest.split(['(', ':']).next().unwrap_or(rest).trim();
            ("ClassDef".to_string(), cls_name.to_string(), "clase".to_string())
        } else if trimmed.starts_with("if ") || trimmed.starts_with("elif ") {
            let cond = trimmed.trim_end_matches(':');
            let kind = if trimmed.contains("if False") || trimmed.contains("if 0") {
                "OpaqueBranch"
            } else {
                "IfStmt"
            };
            (kind.to_string(), cond.chars().take(42).collect(), "condicional".to_string())
        } else if trimmed == "else:" {
            ("ElseBranch".to_string(), "else".to_string(), "rama alternativa".to_string())
        } else if trimmed.starts_with("for ") || trimmed.starts_with("while ") {
            ("LoopStmt".to_string(), trimmed.trim_end_matches(':').chars().take(42).collect(), "bucle".to_string())
        } else if trimmed.starts_with("return ") || trimmed == "return" {
            ("ReturnStmt".to_string(), trimmed.chars().take(42).collect(), "retorno".to_string())
        } else if let Some((lhs, rhs)) = trimmed.split_once('=') {
            if !lhs.ends_with('=') && !lhs.ends_with('!') && !lhs.ends_with('<') && !lhs.ends_with('>') && !rhs.starts_with('=') {
                let target = lhs.trim();
                let val_preview: String = rhs.trim().chars().take(36).collect();
                let ntype = if target.starts_with("_0x") || target.starts_with("_Il") {
                    "ObfuscatedAssign"
                } else {
                    "Assign"
                };
                (ntype.to_string(), target.to_string(), val_preview)
            } else {
                ("Expr".to_string(), trimmed.chars().take(42).collect(), "expresión".to_string())
            }
        } else if trimmed.contains('(') && trimmed.ends_with(')') {
            let call_name = trimmed.split('(').next().unwrap_or("call").trim();
            let ntype = if is_alert { "SecuritySinkCall" } else { "CallExpr" };
            (ntype.to_string(), call_name.to_string(), trimmed.chars().take(40).collect())
        } else {
            ("Stmt".to_string(), trimmed.chars().take(42).collect(), "sentencia".to_string())
        };

        entries.push(FlatAstEntry {
            id: next_id,
            node_type,
            name,
            detail,
            line_number: line_num,
            depth,
            is_alert,
        });
        next_id += 1;
    }

    entries
}

