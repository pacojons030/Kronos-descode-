//! Resilient Markdown Code Block Parser with Multi-Block and Language Detection, plus Conversation History.

use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedCodeBlock {
    pub language: String,
    pub code: String,
    pub is_closed: bool,
}

pub struct MarkdownCodeParser;

impl MarkdownCodeParser {
    /// Extracts all code blocks (```lang ... ```) from markdown text.
    pub fn extract_all_blocks(text: &str) -> Vec<ExtractedCodeBlock> {
        let mut blocks = Vec::new();
        let mut cursor = 0;

        while let Some(start_tick) = text[cursor..].find("```") {
            let abs_start = cursor + start_tick;
            let after_ticks = &text[abs_start + 3..];

            // Find end of the opening line containing the language identifier
            let line_end = after_ticks.find('\n').unwrap_or(after_ticks.len());
            let lang = after_ticks[..line_end].trim().to_lowercase();
            let code_start = abs_start + 3 + line_end + if line_end < after_ticks.len() { 1 } else { 0 };

            // Find closing backticks
            if let Some(close_tick) = text[code_start..].find("```") {
                let code_content = text[code_start..code_start + close_tick].trim();
                blocks.push(ExtractedCodeBlock {
                    language: lang,
                    code: code_content.to_string(),
                    is_closed: true,
                });
                cursor = code_start + close_tick + 3;
            } else {
                // Unclosed block (e.g. truncated response from LLM max tokens)
                let remaining_code = text[code_start..].trim();
                if !remaining_code.is_empty() {
                    blocks.push(ExtractedCodeBlock {
                        language: lang,
                        code: remaining_code.to_string(),
                        is_closed: false,
                    });
                }
                break;
            }
        }

        blocks
    }

    /// Selects the primary code block for applying to the editor.
    /// Priority:
    /// 1. Closed Python block (`python` or `py`).
    /// 2. Longest non-shell code block.
    /// 3. First available code block.
    pub fn select_primary_code_block(text: &str) -> Option<String> {
        let blocks = Self::extract_all_blocks(text);
        if blocks.is_empty() {
            return None;
        }

        // 1. Look for explicit target language code block (Java, Kotlin, Python, Rust, C/C++, JS/TS)
        if let Some(target_block) = blocks.iter().find(|b| {
            matches!(
                b.language.as_str(),
                "java"
                    | "kotlin"
                    | "kt"
                    | "python"
                    | "py"
                    | "rust"
                    | "rs"
                    | "c"
                    | "cpp"
                    | "javascript"
                    | "typescript"
                    | "js"
                    | "ts"
            ) && !b.code.is_empty()
        }) {
            return Some(target_block.code.clone());
        }

        // 2. Look for non-shell blocks (exclude bash, sh, shell, cmd, powershell)
        let non_shell: Vec<&ExtractedCodeBlock> = blocks
            .iter()
            .filter(|b| !matches!(b.language.as_str(), "bash" | "sh" | "shell" | "cmd" | "powershell"))
            .collect();

        if let Some(longest) = non_shell.into_iter().max_by_key(|b| b.code.len()) {
            return Some(longest.code.clone());
        }

        // 3. Fallback to first block
        blocks.first().map(|b| b.code.clone())
    }

    /// Backwards-compatible code block extraction function.
    pub fn extract_code_block(text: &str) -> Option<String> {
        Self::select_primary_code_block(text)
    }
}

/// Standalone re-export for backward compatibility.
pub fn extract_code_block(text: &str) -> Option<String> {
    MarkdownCodeParser::select_primary_code_block(text)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Model,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: String,
}

#[derive(Debug, Clone)]
pub struct ConversationHistory {
    pub messages: Vec<ConversationMessage>,
    pub max_turns: usize,
}

impl ConversationHistory {
    pub fn new(max_turns: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_turns: if max_turns == 0 { 10 } else { max_turns },
        }
    }

    pub fn push(&mut self, role: MessageRole, content: String, timestamp: String) {
        self.messages.push(ConversationMessage { role, content, timestamp });
        // Maintain sliding window (max_turns user-assistant pairs = 2 * max_turns messages)
        let max_messages = self.max_turns * 2;
        if self.messages.len() > max_messages {
            let overflow = self.messages.len() - max_messages;
            self.messages.drain(0..overflow);
        }
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Builds the standard Gemini REST contents JSON payload.
    pub fn to_gemini_contents(&self) -> Vec<Value> {
        self.messages
            .iter()
            .filter_map(|msg| {
                let role_str = match msg.role {
                    MessageRole::User => "user",
                    MessageRole::Model => "model",
                    MessageRole::System => return None, // System handled via systemInstruction
                };
                Some(json!({
                    "role": role_str,
                    "parts": [{ "text": msg.content }]
                }))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_extracts_multiple_blocks() {
        let markdown =
"Primero instala las dependencias:
```bash
pip install cryptography
```
Luego ejecuta el script desofuscado:
```python
import cryptography
print('Seguro')
```
Y la configuración generada es:
```json
{ \"status\": \"ok\" }
```";

        let blocks = MarkdownCodeParser::extract_all_blocks(markdown);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].language, "bash");
        assert_eq!(blocks[0].code, "pip install cryptography");
        assert_eq!(blocks[1].language, "python");
        assert!(blocks[1].code.contains("print('Seguro')"));
        assert_eq!(blocks[2].language, "json");
    }

    #[test]
    fn test_parser_selects_primary_python_block() {
        let markdown =
"```bash\ncurl evil.com\n```\nAquí está el script:\n```python\ndef run():\n    pass\n```";
        let primary = MarkdownCodeParser::select_primary_code_block(markdown);
        assert_eq!(primary, Some("def run():\n    pass".to_string()));
    }

    #[test]
    fn test_parser_handles_unclosed_block_gracefully() {
        let markdown = "Aquí está el código:\n```python\nx = 100\ny = 200";
        let blocks = MarkdownCodeParser::extract_all_blocks(markdown);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language, "python");
        assert_eq!(blocks[0].code, "x = 100\ny = 200");
        assert!(!blocks[0].is_closed);
    }

    #[test]
    fn test_conversation_history_sliding_window() {
        let mut hist = ConversationHistory::new(2); // Max 2 turns = 4 messages
        hist.push(MessageRole::User, "Msg 1".into(), "10:00".into());
        hist.push(MessageRole::Model, "Resp 1".into(), "10:01".into());
        hist.push(MessageRole::User, "Msg 2".into(), "10:02".into());
        hist.push(MessageRole::Model, "Resp 2".into(), "10:03".into());
        assert_eq!(hist.messages.len(), 4);

        // Exceed limit -> oldest turns purged
        hist.push(MessageRole::User, "Msg 3".into(), "10:04".into());
        hist.push(MessageRole::Model, "Resp 3".into(), "10:05".into());
        assert_eq!(hist.messages.len(), 4);
        assert_eq!(hist.messages[0].content, "Msg 2");
        assert_eq!(hist.messages[3].content, "Resp 3");
    }

    #[test]
    fn test_conversation_to_gemini_contents() {
        let mut hist = ConversationHistory::new(5);
        hist.push(MessageRole::User, "Hello".into(), "00:00".into());
        hist.push(MessageRole::Model, "Hi there".into(), "00:01".into());
        let contents = hist.to_gemini_contents();
        assert_eq!(contents.len(), 2);
        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[0]["parts"][0]["text"], "Hello");
        assert_eq!(contents[1]["role"], "model");
        assert_eq!(contents[1]["parts"][0]["text"], "Hi there");
    }
}
