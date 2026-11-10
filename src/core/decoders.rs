use regex::Regex;
use std::collections::HashMap;

/// Pase 1: Resuelve llamadas estáticas a chr(N) y chr(0xNN)
pub fn decode_chr_calls(code: &str) -> (String, usize) {
    let mut count = 0;
    let mut result = code.to_string();

    if let Ok(re) = Regex::new(r"chr\(\s*(0x[0-9a-fA-F]+|\d+)\s*\)") {
        let mut replacements = Vec::new();
        for cap in re.captures_iter(&result) {
            if let Some(m) = cap.get(0) {
                let raw = &cap[1];
                let parsed = if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
                    u32::from_str_radix(hex, 16)
                } else {
                    raw.parse::<u32>()
                };
                if let Ok(num) = parsed {
                    if let Some(ch) = char::from_u32(num) {
                        replacements.push((m.range(), format!("{:?}", ch.to_string())));
                        count += 1;
                    }
                }
            }
        }
        for (range, val) in replacements.into_iter().rev() {
            result.replace_range(range, &val);
        }
    }

    (result, count)
}

/// Pase 2: Resuelve comprensiones XOR típicas: "".join([chr(x ^ KEY) for x in [n1, n2, ...]])
pub fn decode_xor_comprehensions(code: &str) -> (String, usize) {
    let mut count = 0;
    let mut result = code.to_string();

    if let Ok(re) = Regex::new(
        r#"["']["']\.join\(\[\s*chr\(\s*[a-zA-Z_]\w*\s*\^\s*(0x[0-9a-fA-F]+|\d+)\s*\)\s+for\s+[a-zA-Z_]\w*\s+in\s+\[([0-9xXa-fA-F,\s]+)\]\s*\]\)"#,
    ) {
        let mut replacements = Vec::new();
        for cap in re.captures_iter(&result) {
            if let Some(m) = cap.get(0) {
                let key_str = &cap[1];
                let list_str = &cap[2];

                let key_opt = if let Some(h) = key_str.strip_prefix("0x").or_else(|| key_str.strip_prefix("0X")) {
                    u32::from_str_radix(h, 16).ok()
                } else {
                    key_str.parse::<u32>().ok()
                };

                if let Some(key) = key_opt {
                    let mut decoded = String::new();
                    let mut valid = true;
                    for item in list_str.split(',') {
                        let tok = item.trim();
                        if tok.is_empty() {
                            continue;
                        }
                        let val_opt = if let Some(h) = tok.strip_prefix("0x").or_else(|| tok.strip_prefix("0X")) {
                            u32::from_str_radix(h, 16).ok()
                        } else {
                            tok.parse::<u32>().ok()
                        };
                        if let Some(v) = val_opt {
                            if let Some(ch) = char::from_u32(v ^ key) {
                                decoded.push(ch);
                            } else {
                                valid = false;
                                break;
                            }
                        } else {
                            valid = false;
                            break;
                        }
                    }
                    if valid && !decoded.is_empty() {
                        replacements.push((m.range(), format!("{:?}", decoded)));
                        count += 1;
                    }
                }
            }
        }
        for (range, val) in replacements.into_iter().rev() {
            result.replace_range(range, &val);
        }
    }

    (result, count)
}

/// Pase 3: Colapsa concatenaciones de literales de texto ("a" + "b" -> "ab")
pub fn fold_string_concatenations(code: &str) -> (String, usize) {
    let mut count = 0;
    let mut result = code.to_string();

    if let Ok(re_double) = Regex::new(r#""\s*\+\s*""#) {
        loop {
            let matches = re_double.find_iter(&result).count();
            if matches == 0 {
                break;
            }
            result = re_double.replace_all(&result, "").to_string();
            count += matches;
        }
    }
    if let Ok(re_single) = Regex::new(r#"'\s*\+\s*'"#) {
        loop {
            let matches = re_single.find_iter(&result).count();
            if matches == 0 {
                break;
            }
            result = re_single.replace_all(&result, "").to_string();
            count += matches;
        }
    }

    (result, count)
}

/// Pase 4: Decodifica bytes.fromhex("...").decode("utf-8")
pub fn decode_hex_bytes(code: &str) -> (String, usize) {
    let mut count = 0;
    let mut result = code.to_string();

    if let Ok(re) = Regex::new(
        r#"bytes\.fromhex\(\s*["']([0-9a-fA-F\s]+)["']\s*\)(?:\.decode\(\s*(?:["']utf-?8["'])?\s*\))?"#,
    ) {
        let mut replacements = Vec::new();
        for cap in re.captures_iter(&result) {
            if let Some(m) = cap.get(0) {
                let hex_clean: String = cap[1].chars().filter(|c| !c.is_whitespace()).collect();
                if !hex_clean.len().is_multiple_of(2) || hex_clean.is_empty() {
                    continue;
                }
                let mut bytes = Vec::with_capacity(hex_clean.len() / 2);
                let mut valid = true;
                for i in (0..hex_clean.len()).step_by(2) {
                    if let Ok(b) = u8::from_str_radix(&hex_clean[i..i + 2], 16) {
                        bytes.push(b);
                    } else {
                        valid = false;
                        break;
                    }
                }
                if valid {
                    if let Ok(decoded) = String::from_utf8(bytes) {
                        replacements.push((m.range(), format!("{:?}", decoded)));
                        count += 1;
                    }
                }
            }
        }
        for (range, val) in replacements.into_iter().rev() {
            result.replace_range(range, &val);
        }
    }

    (result, count)
}

/// Pase 5: Decodifica base64.b64decode(b"...").decode("utf-8")
pub fn decode_base64_calls(code: &str) -> (String, usize) {
    let mut count = 0;
    let mut result = code.to_string();

    if let Ok(re) = Regex::new(
        r#"base64\.(?:standard_)?b64decode\(\s*b?["']([A-Za-z0-9+/=\s]+)["']\s*\)(?:\.decode\(\s*(?:["']utf-?8["'])?\s*\))?"#,
    ) {
        let mut replacements = Vec::new();
        for cap in re.captures_iter(&result) {
            if let Some(m) = cap.get(0) {
                let b64_clean: String = cap[1].chars().filter(|c| !c.is_whitespace()).collect();
                use base64::Engine;
                if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&b64_clean) {
                    if let Ok(decoded) = String::from_utf8(bytes) {
                        replacements.push((m.range(), format!("{:?}", decoded)));
                        count += 1;
                    }
                }
            }
        }
        for (range, val) in replacements.into_iter().rev() {
            result.replace_range(range, &val);
        }
    }

    (result, count)
}

/// Pase 6: Plegado de expresiones aritméticas y bit a bit constantes: (100 * 5), (50 // 2), (10 ^ 2)
pub fn fold_arithmetic_constants(code: &str) -> (String, usize) {
    let mut count = 0;
    let mut result = code.to_string();

    // Resolver subexpresiones entre paréntesis: (A op B)
    if let Ok(re_paren) = Regex::new(r"\(\s*(\d+)\s*(\+|\-|\*|//|\^|&|\|)\s*(\d+)\s*\)") {
        for _ in 0..4 {
            let mut replacements = Vec::new();
            for cap in re_paren.captures_iter(&result) {
                if let Some(m) = cap.get(0) {
                    if let (Ok(a), Ok(b)) = (cap[1].parse::<i64>(), cap[3].parse::<i64>()) {
                        let op = &cap[2];
                        let val_opt = match op {
                            "+" => Some(a.wrapping_add(b)),
                            "-" => Some(a.wrapping_sub(b)),
                            "*" => Some(a.wrapping_mul(b)),
                            "//" => a.checked_div(b),
                            "^" => Some(a ^ b),
                            "&" => Some(a & b),
                            "|" => Some(a | b),
                            _ => None,
                        };
                        if let Some(v) = val_opt {
                            replacements.push((m.range(), v.to_string()));
                            count += 1;
                        }
                    }
                }
            }
            if replacements.is_empty() {
                break;
            }
            for (range, val) in replacements.into_iter().rev() {
                result.replace_range(range, &val);
            }
        }
    }

    // Resolver sumas/restas simples en asignaciones: = A + B - C
    if let Ok(re_chain) = Regex::new(r"=\s*(\d+)\s*(\+|\-)\s*(\d+)(?:\s*(\+|\-)\s*(\d+))?") {
        let mut replacements = Vec::new();
        for cap in re_chain.captures_iter(&result) {
            if let Some(m) = cap.get(0) {
                if let (Ok(a), Ok(b)) = (cap[1].parse::<i64>(), cap[3].parse::<i64>()) {
                    let mut acc = if &cap[2] == "+" {
                        a.wrapping_add(b)
                    } else {
                        a.wrapping_sub(b)
                    };
                    if let (Some(op2), Some(c_match)) = (cap.get(4), cap.get(5)) {
                        if let Ok(c) = c_match.as_str().parse::<i64>() {
                            acc = if op2.as_str() == "+" {
                                acc.wrapping_add(c)
                            } else {
                                acc.wrapping_sub(c)
                            };
                        }
                    }
                    replacements.push((m.range(), format!("= {}", acc)));
                    count += 1;
                }
            }
        }
        for (range, val) in replacements.into_iter().rev() {
            result.replace_range(range, &val);
        }
    }

    (result, count)
}

/// Pase 7: Resolución de accesos estáticos a listas constantes: arr = ["a", "b"]; x = arr[1] -> "b"
pub fn resolve_static_list_indices(code: &str) -> (String, usize) {
    let mut count = 0;
    let mut result = code.to_string();
    let mut lists: HashMap<String, Vec<String>> = HashMap::new();

    if let Ok(re_list) = Regex::new(r#"([a-zA-Z_]\w*)\s*=\s*\[\s*("[^"]*"(?:\s*,\s*"[^"]*")*)\s*\]"#) {
        if let Ok(re_item) = Regex::new(r#""([^"]*)""#) {
            for cap in re_list.captures_iter(&result) {
                let var_name = cap[1].to_string();
                let items_raw = &cap[2];
                let items: Vec<String> = re_item
                    .captures_iter(items_raw)
                    .map(|c| c[1].to_string())
                    .collect();
                if !items.is_empty() {
                    lists.insert(var_name, items);
                }
            }
        }
    }

    if lists.is_empty() {
        return (result, 0);
    }

    if let Ok(re_idx) = Regex::new(r"\b([a-zA-Z_]\w*)\[\s*(\d+)\s*\]") {
        let mut replacements = Vec::new();
        for cap in re_idx.captures_iter(&result) {
            if let Some(m) = cap.get(0) {
                let name = &cap[1];
                if let Some(items) = lists.get(name) {
                    if let Ok(idx) = cap[2].parse::<usize>() {
                        if let Some(val) = items.get(idx) {
                            replacements.push((m.range(), format!("{:?}", val)));
                            count += 1;
                        }
                    }
                }
            }
        }
        for (range, val) in replacements.into_iter().rev() {
            result.replace_range(range, &val);
        }
    }

    (result, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fold_arithmetic_constants_wrapping_overflow() {
        // Addition overflow wraps around to i64::MIN
        let (res1, c1) = fold_arithmetic_constants("x = 9223372036854775807 + 1");
        assert_eq!(res1, "x = -9223372036854775808");
        assert_eq!(c1, 1);

        // Subtraction underflow wraps around to positive
        let (res2, c2) = fold_arithmetic_constants("x = 0 - 9223372036854775807 - 2");
        assert_eq!(res2, "x = 9223372036854775807");
        assert_eq!(c2, 1);

        // Parenthesized expression wrapping multiplication
        let (res3, c3) = fold_arithmetic_constants("x = (9223372036854775807 * 2)");
        assert_eq!(res3, "x = -2");
        assert_eq!(c3, 1);
    }

    #[test]
    fn test_fold_arithmetic_constants_checked_div() {
        // Division by zero does not panic
        let (res1, c1) = fold_arithmetic_constants("x = (100 // 0)");
        assert_eq!(res1, "x = (100 // 0)");
        assert_eq!(c1, 0);

        // Normal division succeeds
        let (res2, c2) = fold_arithmetic_constants("x = (100 // 4)");
        assert_eq!(res2, "x = 25");
        assert_eq!(c2, 1);
    }
}
