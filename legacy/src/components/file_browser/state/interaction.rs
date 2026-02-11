use crate::core::component::{Component, ComponentResponse};
use crate::core::value::Value;
use crate::inputs::Input;
use crate::terminal::{KeyCode, KeyModifiers};
use std::fs;
use std::time::Duration;

use super::super::model::EntryFilter;
use super::FileBrowserState;

impl FileBrowserState {
    pub fn handle_list_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> ComponentResponse {
        self.poll_scans();
        if modifiers != KeyModifiers::NONE {
            return ComponentResponse::not_handled();
        }

        match code {
            KeyCode::Up | KeyCode::Down => self.select.handle_key(code, modifiers),
            KeyCode::Right => {
                if let Some(entry) = self.selected_entry().cloned() {
                    if entry.is_dir {
                        self.enter_dir(&entry.path);
                        return ComponentResponse::handled();
                    }
                }
                ComponentResponse::not_handled()
            }
            KeyCode::Left => {
                self.leave_dir();
                ComponentResponse::handled()
            }
            KeyCode::Enter => {
                if self.nav.entries.is_empty() {
                    if let Some(new_entry) = self.new_entry_candidate() {
                        if new_entry.is_dir {
                            if fs::create_dir_all(&new_entry.path).is_ok() {
                                self.enter_dir(&new_entry.path);
                                return ComponentResponse::handled();
                            }
                        } else {
                            if let Some(parent) = new_entry.path.parent() {
                                let _ = fs::create_dir_all(parent);
                            }
                            if fs::File::create(&new_entry.path).is_ok() {
                                return ComponentResponse::produced(Value::Text(
                                    new_entry.path.to_string_lossy().to_string(),
                                ));
                            }
                        }
                    }
                }

                if let Some(entry) = self.selected_entry().cloned() {
                    if entry.is_dir {
                        self.enter_dir(&entry.path);
                        return ComponentResponse::handled();
                    }
                }

                if let Some(value) = self.selected_value() {
                    return ComponentResponse::produced(value);
                }

                ComponentResponse::not_handled()
            }
            _ => ComponentResponse::not_handled(),
        }
    }

    pub fn handle_input_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> ComponentResponse {
        self.poll_scans();

        if modifiers.contains(KeyModifiers::CONTROL) {
            match code {
                KeyCode::Char('h') => {
                    self.search.hide_hidden = !self.search.hide_hidden;
                    self.refresh_view();
                    return ComponentResponse::handled();
                }
                KeyCode::Char('f') => {
                    self.toggle_entry_filter(EntryFilter::FilesOnly);
                    return ComponentResponse::handled();
                }
                KeyCode::Char('d') => {
                    self.toggle_entry_filter(EntryFilter::DirsOnly);
                    return ComponentResponse::handled();
                }
                KeyCode::Char('g') => {
                    self.search.show_info = !self.search.show_info;
                    self.refresh_view();
                    return ComponentResponse::handled();
                }
                _ => {}
            }
        }

        if modifiers == KeyModifiers::NONE && code == KeyCode::Tab {
            if !self.has_autocomplete_candidates() {
                return ComponentResponse::not_handled();
            }
            let _ = self.apply_autocomplete();
            return ComponentResponse::handled();
        }

        let before = self.input.value();
        let result = self.input.handle_key(code, modifiers);
        let after = self.input.value();

        if before != after {
            // Set debounce timer instead of immediate refresh.
            self.mark_input_changed();
            return ComponentResponse::handled();
        }

        match result {
            crate::inputs::KeyResult::Submit => ComponentResponse::submit_requested(),
            crate::inputs::KeyResult::Handled => ComponentResponse::handled(),
            crate::inputs::KeyResult::NotHandled => ComponentResponse::not_handled(),
        }
    }

    pub fn handle_combined_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> ComponentResponse {
        let list_response = self.handle_list_key(code, modifiers);
        if list_response.handled {
            return list_response;
        }
        self.handle_input_key(code, modifiers)
    }

    pub fn poll(&mut self) -> bool {
        let updated_scans = self.poll_scans();
        let updated_cache = self.apply_cached_search_if_ready();

        let mut debounce_triggered = false;
        if self
            .search
            .take_debounce_if_elapsed(Duration::from_millis(50))
        {
            self.refresh_view();
            debounce_triggered = true;
        }

        let updated_spinner = self.search.tick_spinner(self.is_searching_current());
        let debounce_pending = self.search.debounce_pending();

        updated_scans || updated_cache || updated_spinner || debounce_triggered || debounce_pending
    }

    pub fn delete_word(&mut self) -> ComponentResponse {
        let before = self.input.value();
        self.input.delete_word();
        let after = self.input.value();
        if before != after {
            self.mark_input_changed();
            ComponentResponse::handled()
        } else {
            ComponentResponse::not_handled()
        }
    }

    pub fn delete_word_forward(&mut self) -> ComponentResponse {
        let before = self.input.value();
        self.input.delete_word_forward();
        let after = self.input.value();
        if before != after {
            self.mark_input_changed();
            ComponentResponse::handled()
        } else {
            ComponentResponse::not_handled()
        }
    }
}
