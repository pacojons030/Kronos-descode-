use regex::Regex;
use std::collections::HashMap;

pub fn normalize_obfuscated_identifiers(code: &str) -> (String, usize, Vec<(String, String)>) {
    let Ok(re) = Regex::new(r"\b(?:_0x[0-9a-fA-F]+|_[Il1O0]{4,}|[Il1]{6,})\b") else {
        return (code.to_string(), 0, Vec::new());
    };

    let mut map: HashMap<String, String> = HashMap::new();
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut counter = 1;

    for mat in re.find_iter(code) {
        let name = mat.as_str();
        if !map.contains_key(name) {
            let new_name = format!("var_{}", counter);
            map.insert(name.to_string(), new_name.clone());
            pairs.push((name.to_string(), new_name));
            counter += 1;
        }
    }

    if map.is_empty() {
        return (code.to_string(), 0, Vec::new());
    }

    let count = map.len();
    let result = re
        .replace_all(code, |caps: &regex::Captures| {
            let matched = &caps[0];
            map.get(matched)
                .cloned()
                .unwrap_or_else(|| matched.to_string())
        })
        .into_owned();

    (result, count, pairs)
}
