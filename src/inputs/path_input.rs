use crate::inputs::{Input, InputBase, KeyResult};
use crate::span::Span;
use crate::terminal::{KeyCode, KeyModifiers};
use crate::text_input::TextInput;
use crate::validators::Validator;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathMode {
    Files,
    Dirs,
    Any,
}

pub struct PathInput {
    inner: TextInput,
    mode: PathMode,
    extensions: Vec<String>,
    completion_dir: String,
    completion_prefix: String,
    completion_matches: Vec<(String, bool)>,
    completion_index: usize,
}

impl PathInput {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let inner = TextInput::new(id, label).with_placeholder("./");
        Self {
            inner,
            mode: PathMode::Any,
            extensions: Vec::new(),
            completion_dir: String::new(),
            completion_prefix: String::new(),
            completion_matches: Vec::new(),
            completion_index: 0,
        }
    }

    pub fn with_min_width(mut self, width: usize) -> Self {
        self.inner = self.inner.with_min_width(width);
        self
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.inner = self.inner.with_validator(validator);
        self
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.inner = self.inner.with_placeholder(placeholder);
        self
    }

    pub fn with_mode(mut self, mode: PathMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_extensions(mut self, extensions: Vec<&str>) -> Self {
        self.extensions = extensions
            .into_iter()
            .map(|ext| ext.trim_start_matches('.').to_ascii_lowercase())
            .collect();
        self
    }

    pub fn with_extension(mut self, extension: &str) -> Self {
        let ext = extension.trim_start_matches('.').to_ascii_lowercase();
        if !ext.is_empty() {
            self.extensions.push(ext);
        }
        self
    }

    fn should_attempt_completion(&self) -> bool {
        let value = self.inner.value();
        if value.is_empty() {
            return false;
        }
        matches!(value.chars().last(), Some('/') | Some('.') | Some('~'))
    }

    fn autocomplete(&mut self) -> bool {
        let value = self.inner.value();
        let (mut dir_part, mut prefix) = split_dir_prefix(&value);
        let mut dir_path = if dir_part.is_empty() {
            ".".to_string()
        } else {
            dir_part.clone()
        };

        if !Path::new(&dir_path).is_dir() {
            let trimmed = value.trim_end_matches('/');
            if let Some(idx) = trimmed.rfind('/') {
                dir_part = trimmed[..idx + 1].to_string();
                prefix.clear();
                dir_path = if dir_part.is_empty() {
                    ".".to_string()
                } else {
                    dir_part.clone()
                };
            } else {
                dir_part.clear();
                prefix.clear();
                dir_path = ".".to_string();
            }
        }

        let dir = Path::new(&dir_path);
        let Ok(entries) = fs::read_dir(dir) else {
            return false;
        };

        let mut matches: Vec<(String, bool)> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                if !name.starts_with(&prefix) {
                    return None;
                }
                let is_dir = entry.metadata().ok()?.is_dir();
                if !self.accepts_entry(&name, is_dir) {
                    return None;
                }
                Some((name, is_dir))
            })
            .collect();

        if matches.is_empty() {
            self.reset_completion();
            return false;
        }

        matches.sort_by(|a, b| a.0.cmp(&b.0));

        let mut effective_prefix = prefix.clone();
        if matches.len() == 1 && matches[0].0 == prefix {
            let Ok(entries) = fs::read_dir(Path::new(&dir_path)) else {
                return false;
            };
            let mut all_matches: Vec<(String, bool)> = entries
                .flatten()
                .filter_map(|entry| {
                    let name = entry.file_name().into_string().ok()?;
                    let is_dir = entry.metadata().ok()?.is_dir();
                    if !self.accepts_entry(&name, is_dir) {
                        return None;
                    }
                    Some((name, is_dir))
                })
                .collect();
            if !all_matches.is_empty() {
                all_matches.sort_by(|a, b| a.0.cmp(&b.0));
                matches = all_matches;
                effective_prefix.clear();
            }
        }

        let is_same_query =
            self.completion_dir == dir_part && self.completion_prefix == effective_prefix;
        if is_same_query && !self.completion_matches.is_empty() {
            self.completion_index = (self.completion_index + 1) % self.completion_matches.len();
        } else {
            self.completion_dir = dir_part.clone();
            self.completion_prefix = effective_prefix.clone();
            self.completion_matches = matches;
            self.completion_index = 0;
        }

        let (name, is_dir) = self.completion_matches[self.completion_index].clone();
        let mut new_value = String::new();
        new_value.push_str(&dir_part);
        new_value.push_str(&name);
        if is_dir && !new_value.ends_with('/') {
            new_value.push('/');
        }
        self.inner.set_value(new_value);
        true
    }

    fn reset_completion(&mut self) {
        self.completion_dir.clear();
        self.completion_prefix.clear();
        self.completion_matches.clear();
        self.completion_index = 0;
    }

    fn accepts_entry(&self, name: &str, is_dir: bool) -> bool {
        match self.mode {
            PathMode::Dirs => is_dir,
            PathMode::Files => {
                if is_dir {
                    return false;
                }
                self.matches_extension(name)
            }
            PathMode::Any => {
                if is_dir {
                    true
                } else {
                    self.matches_extension(name)
                }
            }
        }
    }

    fn matches_extension(&self, name: &str) -> bool {
        if self.extensions.is_empty() {
            return true;
        }
        let ext = name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
        self.extensions
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(ext))
    }
}

impl Input for PathInput {
    fn base(&self) -> &InputBase {
        self.inner.base_ref()
    }

    fn base_mut(&mut self) -> &mut InputBase {
        self.inner.base_mut_ref()
    }

    fn value(&self) -> String {
        self.inner.value()
    }

    fn set_value(&mut self, value: String) {
        self.inner.set_value(value);
    }

    fn raw_value(&self) -> String {
        self.inner.raw_value()
    }

    fn is_complete(&self) -> bool {
        self.inner.is_complete()
    }

    fn cursor_pos(&self) -> usize {
        self.inner.cursor_pos()
    }

    fn supports_tab_completion(&self) -> bool {
        true
    }

    fn handle_tab_completion(&mut self) -> bool {
        if !self.should_attempt_completion() {
            return false;
        }

        let before = self.inner.value();
        if !self.autocomplete() {
            return false;
        }
        let after = self.inner.value();
        before != after
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> KeyResult {
        match code {
            KeyCode::Tab if modifiers == KeyModifiers::NONE => {
                if self.handle_tab_completion() {
                    KeyResult::Handled
                } else {
                    KeyResult::NotHandled
                }
            }
            _ => {
                self.reset_completion();
                self.inner.handle_key(code, modifiers)
            }
        }
    }

    fn render_content(&self, theme: &crate::theme::Theme) -> Vec<Span> {
        self.inner.render_content(theme)
    }

    fn cursor_offset_in_content(&self) -> usize {
        self.inner.cursor_offset_in_content()
    }

    fn delete_word(&mut self) {
        self.inner.delete_word();
    }

    fn delete_word_forward(&mut self) {
        self.inner.delete_word_forward();
    }
}

fn split_dir_prefix(value: &str) -> (String, String) {
    if value.ends_with('/') {
        return (value.to_string(), String::new());
    }
    if let Some(idx) = value.rfind('/') {
        let (dir, rest) = value.split_at(idx + 1);
        (dir.to_string(), rest.to_string())
    } else {
        (String::new(), value.to_string())
    }
}
