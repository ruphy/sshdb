// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2024 Riccardo Iaconelli <riccardo@kde.org>

use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use crate::config::ConfigStore;
use crate::model::{Config, Host};
use crate::ssh;

#[derive(Clone, Copy, Debug)]
pub enum StatusKind {
    Info,
    Warn,
    Error,
}

pub struct StatusLine {
    pub text: String,
    pub kind: StatusKind,
}

#[derive(Clone, Copy, Debug)]
pub enum FormKind {
    Add,
    Edit,
}

#[derive(Clone, Debug)]
pub enum ConfirmKind {
    Connect { extra_cmd: String },
    Delete,
}

#[derive(Clone, Debug)]
pub struct FormField {
    pub label: &'static str,
    pub value: String,
    pub cursor: usize,
}

#[derive(Clone, Debug)]
pub struct BastionDropdownState {
    pub search_filter: String,
    pub filtered_indices: Vec<usize>,
    pub selected: usize,
    exclude_host: Option<String>,
}

impl BastionDropdownState {
    pub fn new(config: &Config, exclude_host: Option<&str>) -> Self {
        let mut state = Self {
            search_filter: String::new(),
            filtered_indices: Vec::new(),
            selected: 0,
            exclude_host: exclude_host.map(|s| s.to_string()),
        };
        state.rebuild_filter(config);
        state
    }

    pub fn rebuild_filter(&mut self, config: &Config) {
        let matcher = SkimMatcherV2::default();
        if self.search_filter.is_empty() {
            self.filtered_indices = config
                .hosts
                .iter()
                .enumerate()
                .filter(|(_, h)| self.exclude_host.as_deref() != Some(&h.name))
                .map(|(i, _)| i)
                .collect();
        } else {
            let mut scored: Vec<(i64, usize)> = Vec::new();
            for (i, host) in config.hosts.iter().enumerate() {
                if self.exclude_host.as_deref() == Some(&host.name) {
                    continue;
                }
                let haystack = format!(
                    "{} {} {} {}",
                    host.name,
                    host.address,
                    host.tags.join(" "),
                    host.description.clone().unwrap_or_default()
                );
                if let Some(score) = matcher.fuzzy_match(&haystack, &self.search_filter) {
                    scored.push((score, i));
                }
            }
            scored.sort_by(|a, b| b.0.cmp(&a.0));
            self.filtered_indices = scored.into_iter().map(|(_, i)| i).collect();
        }
        // Reset selection to top when filter changes
        self.selected = 0;
        if self.selected >= self.filtered_indices.len() {
            self.selected = self.filtered_indices.len().saturating_sub(1);
        }
    }
}

#[derive(Clone, Debug)]
pub struct FormState {
    pub kind: FormKind,
    pub fields: Vec<FormField>,
    pub index: usize,
    pub bastion_dropdown: Option<BastionDropdownState>,
    editing_host_name: Option<String>,
}

impl FormState {
    pub fn new(kind: FormKind, host: Option<&Host>, config: &Config) -> Self {
        let blank = Host {
            name: "".into(),
            address: "".into(),
            user: None,
            port: None,
            key_path: None,
            tags: Vec::new(),
            options: Vec::new(),
            remote_command: None,
            description: None,
            bastion: None,
        };
        let h = host.unwrap_or(&blank);
        let mut fields = Vec::new();

        if matches!(kind, FormKind::Add) {
            let cmd_val = if h.address.is_empty() {
                "".into()
            } else {
                ssh::command_preview(h, config, None, None)
            };
            let cmd_cursor = cmd_val.len();
            fields.push(FormField {
                label: "SSH command",
                value: cmd_val,
                cursor: cmd_cursor,
            });
        }

        let name = h.name.clone();
        let host_addr = h.address.clone();
        let user = h.user.clone().unwrap_or_default();
        let port = h.port.map(|p| p.to_string()).unwrap_or_default();
        let key = h.key_path.clone().unwrap_or_default();
        let bastion = h.bastion.clone().unwrap_or_default();
        let tags = if h.tags.is_empty() {
            "".into()
        } else {
            h.tags.join(",")
        };
        let options = if h.options.is_empty() {
            "".into()
        } else {
            h.options.join(" ")
        };
        let remote = h.remote_command.clone().unwrap_or_default();
        let desc = h.description.clone().unwrap_or_default();

        fields.extend([
            FormField {
                label: "Name",
                value: name.clone(),
                cursor: name.len(),
            },
            FormField {
                label: "Host / IP",
                value: host_addr.clone(),
                cursor: host_addr.len(),
            },
            FormField {
                label: "User",
                value: user.clone(),
                cursor: user.len(),
            },
            FormField {
                label: "Port",
                value: port.clone(),
                cursor: port.len(),
            },
            FormField {
                label: "Key path",
                value: key.clone(),
                cursor: key.len(),
            },
            FormField {
                label: "Bastion",
                value: bastion.clone(),
                cursor: bastion.len(),
            },
            FormField {
                label: "Tags (comma)",
                value: tags.clone(),
                cursor: tags.len(),
            },
            FormField {
                label: "Options",
                value: options.clone(),
                cursor: options.len(),
            },
            FormField {
                label: "Remote command",
                value: remote.clone(),
                cursor: remote.len(),
            },
            FormField {
                label: "Description",
                value: desc.clone(),
                cursor: desc.len(),
            },
        ]);

        Self {
            kind,
            fields,
            index: 0,
            bastion_dropdown: None,
            editing_host_name: host.map(|h| h.name.clone()),
        }
    }

    pub fn handle_input(&mut self, key: KeyEvent, config: &Config) {
        // Check if we're on the bastion field (index 5 in the fields array, or 6 if Add form)
        let bastion_field_idx = if matches!(self.kind, FormKind::Add) {
            6
        } else {
            5
        };
        let is_bastion_field = self.index == bastion_field_idx;

        // Handle bastion dropdown if it's open
        if is_bastion_field && self.bastion_dropdown.is_some() {
            if let Some(dropdown) = self.bastion_dropdown.as_mut() {
                match key.code {
                    KeyCode::Esc => {
                        // Close dropdown, keep current value
                        self.bastion_dropdown = None;
                        return;
                    }
                    KeyCode::Enter => {
                        // Select from dropdown
                        if let Some(idx) = dropdown.filtered_indices.get(dropdown.selected) {
                            if let Some(host) = config.hosts.get(*idx) {
                                if let Some(f) = self.fields.get_mut(bastion_field_idx) {
                                    f.value = host.name.clone();
                                    f.cursor = f.value.len();
                                }
                            }
                        }
                        self.bastion_dropdown = None;
                        return;
                    }
                    KeyCode::Up => {
                        if dropdown.selected > 0 {
                            dropdown.selected -= 1;
                        } else {
                            dropdown.selected = dropdown.filtered_indices.len().saturating_sub(1);
                        }
                        return;
                    }
                    KeyCode::Down => {
                        if dropdown.selected + 1 < dropdown.filtered_indices.len() {
                            dropdown.selected += 1;
                        } else {
                            dropdown.selected = 0;
                        }
                        return;
                    }
                    KeyCode::Backspace => {
                        // Update the field value first
                        if let Some(f) = self.fields.get_mut(bastion_field_idx) {
                            if f.cursor > 0 {
                                f.value.remove(f.cursor - 1);
                                f.cursor -= 1;
                            }
                            // Sync dropdown search filter with field value
                            dropdown.search_filter = f.value.clone();
                            dropdown.rebuild_filter(config);
                        }
                        return;
                    }
                    KeyCode::Char(c) => {
                        // Handle Space specially - close dropdown when open
                        if c == ' ' && key.modifiers.is_empty() {
                            self.bastion_dropdown = None;
                            return;
                        }
                        // Allow typing to filter the dropdown
                        if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                            // Update the field value first
                            if let Some(f) = self.fields.get_mut(bastion_field_idx) {
                                f.value.insert(f.cursor, c);
                                f.cursor += 1;
                                // Sync dropdown search filter with field value
                                dropdown.search_filter = f.value.clone();
                                dropdown.rebuild_filter(config);
                            }
                        }
                        return;
                    }
                    _ => {}
                }
            }
        }

        match key.code {
            KeyCode::Tab => {
                let bastion_field_idx = if matches!(self.kind, FormKind::Add) {
                    6
                } else {
                    5
                };
                // Close dropdown when leaving bastion field
                if self.index == bastion_field_idx {
                    self.bastion_dropdown = None;
                }
                self.next();
            }
            KeyCode::BackTab => {
                let bastion_field_idx = if matches!(self.kind, FormKind::Add) {
                    6
                } else {
                    5
                };
                // Close dropdown when leaving bastion field
                if self.index == bastion_field_idx {
                    self.bastion_dropdown = None;
                }
                self.prev();
            }
            KeyCode::Up => {
                let bastion_field_idx = if matches!(self.kind, FormKind::Add) {
                    6
                } else {
                    5
                };
                // Close dropdown when leaving bastion field
                if self.index == bastion_field_idx {
                    self.bastion_dropdown = None;
                }
                self.prev();
            }
            KeyCode::Down => {
                let bastion_field_idx = if matches!(self.kind, FormKind::Add) {
                    6
                } else {
                    5
                };
                // Close dropdown when leaving bastion field
                if self.index == bastion_field_idx {
                    self.bastion_dropdown = None;
                }
                self.next();
            }
            KeyCode::Char(' ') => {
                // Space key ONLY toggles dropdown when on bastion field
                if is_bastion_field {
                    if self.bastion_dropdown.is_some() {
                        self.bastion_dropdown = None;
                    } else {
                        self.open_bastion_dropdown(config);
                    }
                    return; // Don't insert space, just toggle dropdown
                }
                // If not on bastion field, insert space normally
                if let Some(f) = self.fields.get_mut(self.index) {
                    f.value.insert(f.cursor, ' ');
                    f.cursor += 1;
                }
            }
            KeyCode::Left => {
                if let Some(f) = self.fields.get_mut(self.index) {
                    if f.cursor > 0 {
                        f.cursor -= 1;
                    }
                }
            }
            KeyCode::Right => {
                if let Some(f) = self.fields.get_mut(self.index) {
                    if f.cursor < f.value.len() {
                        f.cursor += 1;
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(f) = self.fields.get_mut(self.index) {
                    if f.cursor > 0 {
                        f.value.remove(f.cursor - 1);
                        f.cursor -= 1;
                    }
                }
                // Sync dropdown search filter if dropdown is open
                if is_bastion_field {
                    if let Some(dropdown) = self.bastion_dropdown.as_mut() {
                        if let Some(f) = self.fields.get(bastion_field_idx) {
                            dropdown.search_filter = f.value.clone();
                            dropdown.rebuild_filter(config);
                        }
                    }
                }
            }
            KeyCode::Char(c) => {
                // Space is handled above - skip it here
                if c == ' ' {
                    return;
                }
                // Insert character normally
                if let Some(f) = self.fields.get_mut(self.index) {
                    f.value.insert(f.cursor, c);
                    f.cursor += 1;
                }
                // Sync dropdown search filter if dropdown is open and on bastion field
                if is_bastion_field {
                    if let Some(dropdown) = self.bastion_dropdown.as_mut() {
                        if let Some(f) = self.fields.get(bastion_field_idx) {
                            dropdown.search_filter = f.value.clone();
                            dropdown.rebuild_filter(config);
                        }
                    }
                }
            }
            _ => {}
        }
        if let Some(f) = self.fields.get_mut(self.index) {
            f.cursor = f.cursor.min(f.value.len());
        }
        if matches!(self.kind, FormKind::Add) && self.index == 0 {
            if let Some(cmd_field) = self.fields.first() {
                if let Some(spec) =
                    non_empty(&cmd_field.value).and_then(|s| parse_ssh_spec(&s).ok())
                {
                    self.apply_spec(&spec);
                }
            }
        }
    }

    fn next(&mut self) {
        if self.index + 1 < self.fields.len() {
            self.index += 1;
        } else {
            self.index = 0;
        }
        if let Some(f) = self.fields.get_mut(self.index) {
            f.cursor = f.value.len();
        }
    }

    fn prev(&mut self) {
        if self.index == 0 {
            self.index = self.fields.len().saturating_sub(1);
        } else {
            self.index -= 1;
        }
        if let Some(f) = self.fields.get_mut(self.index) {
            f.cursor = f.value.len();
        }
    }

    fn open_bastion_dropdown(&mut self, config: &Config) {
        let bastion_field_idx = if matches!(self.kind, FormKind::Add) {
            6
        } else {
            5
        };
        let mut dropdown = BastionDropdownState::new(config, self.editing_host_name.as_deref());
        // Initialize search filter with current field value
        if let Some(f) = self.fields.get(bastion_field_idx) {
            dropdown.search_filter = f.value.clone();
            dropdown.rebuild_filter(config);
        }
        self.bastion_dropdown = Some(dropdown);
    }

    pub fn build_host(&self) -> Result<Host> {
        let cmd_idx = if matches!(self.kind, FormKind::Add) {
            Some(0)
        } else {
            None
        };
        let mut idx = if cmd_idx.is_some() { 1 } else { 0 };
        let name_field = self.fields[idx].value.trim();
        idx += 1;
        let host_field = self.fields[idx].value.trim();
        idx += 1;
        let user_field = self.fields[idx].value.trim();
        idx += 1;
        let port_field = self.fields[idx].value.trim();
        idx += 1;
        let key_field = self.fields[idx].value.trim();
        idx += 1;
        let bastion_field = self.fields[idx].value.trim();
        idx += 1;
        let tags_field = self.fields[idx].value.trim();
        idx += 1;
        let options_field = self.fields[idx].value.trim();
        idx += 1;
        let remote_field = self.fields[idx].value.trim();
        idx += 1;
        let desc_field = self.fields[idx].value.trim();

        let raw_spec = cmd_idx
            .and_then(|i| non_empty(&self.fields[i].value))
            .map(|s| parse_ssh_spec(&s))
            .transpose()?;

        let host_str = if !host_field.is_empty() {
            host_field.to_string()
        } else if let Some(spec) = &raw_spec {
            spec.address.clone()
        } else {
            "".into()
        };

        let name = if !name_field.is_empty() {
            name_field.to_string()
        } else if !host_str.is_empty() {
            host_str.clone()
        } else {
            "".into()
        };

        if name.is_empty() || host_str.is_empty() {
            return Err(anyhow!("name and host cannot be empty"));
        }

        let user = non_empty(user_field).or_else(|| raw_spec.as_ref().and_then(|s| s.user.clone()));
        let port = non_empty(port_field)
            .map(|p| p.parse::<u16>())
            .transpose()
            .context("port must be numeric")?
            .or_else(|| raw_spec.as_ref().and_then(|s| s.port));
        let key_path =
            non_empty(key_field).or_else(|| raw_spec.as_ref().and_then(|s| s.key_path.clone()));
        let bastion = non_empty(bastion_field);
        let tags = non_empty(tags_field)
            .map(|s| {
                s.split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let options = non_empty(options_field)
            .map(|s| {
                s.split_whitespace()
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let remote_command = non_empty(remote_field);
        let description = non_empty(desc_field);

        Ok(Host {
            name: name.to_string(),
            address: host_str,
            user,
            port,
            key_path,
            tags,
            options,
            remote_command,
            bastion,
            description,
        })
    }

    fn set_field_value(&mut self, label: &str, value: String) {
        if let Some(f) = self.fields.iter_mut().find(|f| f.label == label) {
            f.value = value;
            f.cursor = f.value.len();
        }
    }

    fn apply_spec(&mut self, spec: &SshSpec) {
        self.set_field_value("Host / IP", spec.address.clone());
        if let Some(user) = &spec.user {
            self.set_field_value("User", user.clone());
            if self
                .fields
                .iter()
                .find(|f| f.label == "Name")
                .map(|f| f.value.trim().is_empty())
                .unwrap_or(false)
            {
                self.set_field_value("Name", format!("{user}@{}", spec.address));
            }
        } else {
            self.set_field_value("User", "".into());
        }

        if let Some(port) = spec.port {
            self.set_field_value("Port", port.to_string());
        } else {
            self.set_field_value("Port", "".into());
        }

        if let Some(key) = &spec.key_path {
            self.set_field_value("Key path", key.clone());
        } else {
            self.set_field_value("Key path", "".into());
        }

        if !spec.options.is_empty() {
            self.set_field_value("Options", spec.options.join(" "));
        } else {
            self.set_field_value("Options", "".into());
        }
        if let Some(bastion) = &spec.bastion {
            self.set_field_value("Bastion", bastion.clone());
        } else {
            self.set_field_value("Bastion", "".into());
        }
        if let Some(remote) = &spec.remote_command {
            self.set_field_value("Remote command", remote.clone());
        } else {
            self.set_field_value("Remote command", "".into());
        }
    }
}

fn non_empty(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[derive(Debug, Clone)]
struct SshSpec {
    address: String,
    user: Option<String>,
    port: Option<u16>,
    key_path: Option<String>,
    options: Vec<String>,
    bastion: Option<String>,
    remote_command: Option<String>,
}

fn parse_ssh_spec(input: &str) -> Result<SshSpec> {
    let mut user = None;
    let mut port = None;
    let mut key_path = None;
    let mut bastion = None;
    let mut options = Vec::new();
    let tokens: Vec<&str> = input.split_whitespace().collect();
    let mut i = 0usize;
    if tokens.first() == Some(&"ssh") {
        i += 1;
    }

    let mut target = None;
    // First pass: find the target (hostname)
    while i < tokens.len() {
        let token = tokens[i];
        match token {
            "-p" => {
                if let Some(p) = tokens.get(i + 1) {
                    port = p.parse::<u16>().ok();
                    i += 1;
                }
            }
            "-i" => {
                if let Some(k) = tokens.get(i + 1) {
                    key_path = Some(k.to_string());
                    i += 1;
                }
            }
            "-J" => {
                if let Some(b) = tokens.get(i + 1) {
                    bastion = Some((*b).to_string());
                    i += 1;
                }
            }
            other if other.starts_with('-') => {
                options.push(other.to_string());
                // capture parameter if present
                if let Some(next) = tokens.get(i + 1) {
                    if !next.starts_with('-')
                        && !next.contains('@')
                        && next
                            .chars()
                            .any(|c| c.is_alphanumeric() || c == ':' || c == '/')
                    {
                        options.push((*next).to_string());
                        i += 1;
                    }
                }
            }
            _ => {
                target = Some(token.to_string());
                i += 1;
                break;
            }
        }
        i += 1;
    }

    let Some(target) = target else {
        return Err(anyhow!("ssh target missing (expected user@host or host)"));
    };

    // Second pass: continue parsing options after the target
    let mut remote_start = None;
    while i < tokens.len() {
        let token = tokens[i];
        match token {
            "-p" => {
                if let Some(p) = tokens.get(i + 1) {
                    port = p.parse::<u16>().ok();
                    i += 1;
                }
            }
            "-i" => {
                if let Some(k) = tokens.get(i + 1) {
                    key_path = Some(k.to_string());
                    i += 1;
                }
            }
            "-J" => {
                if let Some(b) = tokens.get(i + 1) {
                    bastion = Some((*b).to_string());
                    i += 1;
                }
            }
            other if other.starts_with('-') => {
                options.push(other.to_string());
                // capture parameter if present
                if let Some(next) = tokens.get(i + 1) {
                    if !next.starts_with('-')
                        && !next.contains('@')
                        && next
                            .chars()
                            .any(|c| c.is_alphanumeric() || c == ':' || c == '/')
                    {
                        options.push((*next).to_string());
                        i += 1;
                    }
                }
            }
            _ => {
                // Not an option, this is where remote command starts
                remote_start = Some(i);
                break;
            }
        }
        i += 1;
    }

    let mut addr = target.clone();
    if let Some((u, h)) = target.split_once('@') {
        user = Some(u.to_string());
        addr = h.to_string();
    }

    Ok(SshSpec {
        address: addr,
        user,
        port,
        key_path,
        options,
        bastion,
        remote_command: if let Some(start) = remote_start {
            Some(tokens[start..].join(" "))
        } else {
            None
        },
    })
}

#[derive(Clone, Debug)]
pub enum Mode {
    Normal,
    Search,
    Form,
    Confirm,
    QuickConnect,
}

pub enum AppAction {
    Quit,
    RunSsh(std::process::Command),
}

pub struct App {
    pub mode: Mode,
    pub status: Option<StatusLine>,
    pub filter: String,
    pub filtered_indices: Vec<usize>,
    pub selected: usize,
    pub dry_run: bool,
    pub form: Option<FormState>,
    pub confirm: Option<ConfirmKind>,
    pub quick_input: Option<String>,
    pub quick_cursor: usize,
    pub show_help: bool,
    pub show_about: bool,
    pub matcher: SkimMatcherV2,
    pub config: Config,
    pub config_path: PathBuf,
    pub history: Vec<Config>,
    store: ConfigStore,
}

impl App {
    pub fn new(store: ConfigStore) -> Result<Self> {
        let config = store
            .load_or_init()
            .with_context(|| "failed to open sshdb config")?;
        let config_path = store.path().to_path_buf();
        let mut app = Self {
            mode: Mode::Normal,
            status: None,
            filter: String::new(),
            filtered_indices: Vec::new(),
            selected: 0,
            dry_run: false,
            form: None,
            confirm: None,
            quick_input: None,
            quick_cursor: 0,
            show_help: false,
            show_about: false,
            matcher: SkimMatcherV2::default(),
            config,
            config_path,
            history: Vec::new(),
            store,
        };
        app.rebuild_filter();
        app.status = Some(StatusLine {
            text: "Loaded config. Dry-run is OFF; press C to toggle.".into(),
            kind: StatusKind::Info,
        });
        Ok(app)
    }

    pub fn on_event(&mut self, event: Event) -> Result<Option<AppAction>> {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key(key),
            _ => Ok(None),
        }
    }

    fn on_key(&mut self, key: KeyEvent) -> Result<Option<AppAction>> {
        if self.show_about {
            if matches!(
                key.code,
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('a')
            ) {
                self.show_about = false;
            }
            return Ok(None);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if let KeyCode::Char('c') = key.code {
                return Ok(Some(AppAction::Quit));
            }
        }
        if self.show_help {
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('h') => {
                    self.show_help = false;
                }
                _ => {}
            }
            return Ok(None);
        }
        match self.mode.clone() {
            Mode::Normal => self.handle_normal(key),
            Mode::Search => self.handle_search(key),
            Mode::Form => self.handle_form(key),
            Mode::Confirm => self.handle_confirm(key),
            Mode::QuickConnect => self.handle_quickconnect(key),
        }
    }

    fn handle_normal(&mut self, key: KeyEvent) -> Result<Option<AppAction>> {
        match key.code {
            KeyCode::Char('q') => return Ok(Some(AppAction::Quit)),
            KeyCode::Char('?') | KeyCode::Char('h') => {
                self.show_help = true;
            }
            KeyCode::Char('a') => {
                self.show_about = true;
            }
            KeyCode::Char('/') => {
                self.mode = Mode::Search;
                self.status = Some(StatusLine {
                    text: "Search: type to filter, Enter to apply.".into(),
                    kind: StatusKind::Info,
                });
            }
            KeyCode::Char('g') => {
                self.mode = Mode::QuickConnect;
                self.quick_input = Some(String::new());
                self.quick_cursor = 0;
                self.status = Some(StatusLine {
                    text: "Quick connect: paste ssh user@host string, Enter to connect.".into(),
                    kind: StatusKind::Info,
                });
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::Char('n') => {
                self.form = Some(FormState::new(FormKind::Add, None, &self.config));
                self.mode = Mode::Form;
                self.status = Some(StatusLine {
                    text: "New host: paste ssh command or fill fields; Tab to move, Enter to save."
                        .into(),
                    kind: StatusKind::Info,
                });
            }
            KeyCode::Char('u') => {
                if self.undo()? {
                    self.status = Some(StatusLine {
                        text: "Undid last change.".into(),
                        kind: StatusKind::Info,
                    });
                } else {
                    self.status = Some(StatusLine {
                        text: "Nothing to undo.".into(),
                        kind: StatusKind::Warn,
                    });
                }
            }
            KeyCode::Char('y') => {
                if let Some(host) = self.current_host().cloned() {
                    self.duplicate_host(host)?;
                }
            }
            KeyCode::Char('e') => {
                if let Some(host) = self.current_host().cloned() {
                    self.form = Some(FormState::new(FormKind::Edit, Some(&host), &self.config));
                    self.mode = Mode::Form;
                } else {
                    self.status = Some(StatusLine {
                        text: "No host selected to edit.".into(),
                        kind: StatusKind::Warn,
                    });
                }
            }
            KeyCode::Char('d') => {
                if self.current_host().is_some() {
                    self.mode = Mode::Confirm;
                    self.confirm = Some(ConfirmKind::Delete);
                }
            }
            KeyCode::Char('c') => {
                if self.current_host().is_some() {
                    self.mode = Mode::Confirm;
                    self.confirm = Some(ConfirmKind::Connect {
                        extra_cmd: String::new(),
                    });
                }
            }
            KeyCode::Enter => {
                if self.current_host().is_some() {
                    return self.connect(None);
                }
            }
            KeyCode::Char('r') => {
                self.reload_config()?;
            }
            KeyCode::Char('C') => {
                self.dry_run = !self.dry_run;
                let state = if self.dry_run { "ON" } else { "OFF" };
                self.status = Some(StatusLine {
                    text: format!("Dry-run toggled {state}."),
                    kind: StatusKind::Info,
                });
            }
            _ => {}
        }
        if let Some(buf) = self.quick_input.as_ref() {
            if self.quick_cursor > buf.len() {
                self.quick_cursor = buf.len();
            }
        } else {
            self.quick_cursor = 0;
        }
        Ok(None)
    }

    fn handle_search(&mut self, key: KeyEvent) -> Result<Option<AppAction>> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status = None;
            }
            KeyCode::Enter => {
                self.mode = Mode::Normal;
            }
            KeyCode::Char(c) => {
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                    self.filter.push(c);
                    self.rebuild_filter();
                }
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.rebuild_filter();
            }
            _ => {}
        }
        Ok(None)
    }

    fn handle_form(&mut self, key: KeyEvent) -> Result<Option<AppAction>> {
        if let Some(form) = self.form.as_mut() {
            // Check if dropdown is open - if so, handle input there first
            let bastion_field_idx = if matches!(form.kind, FormKind::Add) {
                6
            } else {
                5
            };
            let is_bastion_field = form.index == bastion_field_idx;
            if is_bastion_field && form.bastion_dropdown.is_some() {
                // If Enter is pressed with dropdown open, let handle_input handle it
                // (it will select and close dropdown, but not submit form)
                if key.code == KeyCode::Enter {
                    form.handle_input(key, &self.config);
                    return Ok(None);
                }
            }

            match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Normal;
                    self.form = None;
                }
                KeyCode::Enter => {
                    // Only submit form if dropdown is not open
                    if !(is_bastion_field && form.bastion_dropdown.is_some()) {
                        match form.build_host() {
                            Ok(host) => {
                                let action = form.kind;
                                match self.save_host(action, host) {
                                    Ok(_) => {
                                        self.form = None;
                                        self.mode = Mode::Normal;
                                    }
                                    Err(e) => {
                                        self.status = Some(StatusLine {
                                            text: e.to_string(),
                                            kind: StatusKind::Error,
                                        });
                                    }
                                }
                            }
                            Err(e) => {
                                self.status = Some(StatusLine {
                                    text: e.to_string(),
                                    kind: StatusKind::Error,
                                });
                            }
                        }
                    }
                }
                _ => {
                    form.handle_input(key, &self.config);
                }
            }
        } else {
            self.mode = Mode::Normal;
        }
        Ok(None)
    }

    fn handle_confirm(&mut self, key: KeyEvent) -> Result<Option<AppAction>> {
        match self.confirm.clone() {
            Some(ConfirmKind::Delete) => match key.code {
                KeyCode::Esc | KeyCode::Char('n') => {
                    self.mode = Mode::Normal;
                    self.confirm = None;
                }
                KeyCode::Enter | KeyCode::Char('y') => {
                    self.delete_current()?;
                    self.mode = Mode::Normal;
                    self.confirm = None;
                }
                _ => {}
            },
            Some(ConfirmKind::Connect { mut extra_cmd }) => match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Normal;
                    self.confirm = None;
                }
                KeyCode::Enter => {
                    let extra = if extra_cmd.trim().is_empty() {
                        None
                    } else {
                        Some(extra_cmd.trim().to_string())
                    };
                    self.confirm = None;
                    self.mode = Mode::Normal;
                    return self.connect(extra);
                }
                KeyCode::Backspace => {
                    extra_cmd.pop();
                    self.confirm = Some(ConfirmKind::Connect { extra_cmd });
                }
                KeyCode::Char(c) => {
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                        extra_cmd.push(c);
                        self.confirm = Some(ConfirmKind::Connect { extra_cmd });
                    }
                }
                _ => {}
            },
            None => {
                self.mode = Mode::Normal;
            }
        }
        Ok(None)
    }

    fn handle_quickconnect(&mut self, key: KeyEvent) -> Result<Option<AppAction>> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.quick_input = None;
                self.quick_cursor = 0;
            }
            KeyCode::Backspace => {
                if let Some(buf) = self.quick_input.as_mut() {
                    if self.quick_cursor > 0 {
                        buf.remove(self.quick_cursor - 1);
                        self.quick_cursor -= 1;
                    }
                }
            }
            KeyCode::Enter => {
                if let Some(buf) = self.quick_input.take() {
                    let spec = parse_ssh_spec(&buf)?;
                    self.mode = Mode::Normal;
                    self.quick_cursor = 0;
                    return self.quick_connect(spec);
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Char(c) => {
                if let Some(buf) = self.quick_input.as_mut() {
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                        buf.insert(self.quick_cursor, c);
                        self.quick_cursor += 1;
                    }
                }
            }
            KeyCode::Left => {
                if self.quick_cursor > 0 {
                    self.quick_cursor -= 1;
                }
            }
            KeyCode::Right => {
                if let Some(buf) = self.quick_input.as_ref() {
                    if self.quick_cursor < buf.len() {
                        self.quick_cursor += 1;
                    }
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn move_selection(&mut self, delta: isize) {
        if self.filtered_indices.is_empty() {
            self.selected = 0;
            return;
        }
        let len = self.filtered_indices.len() as isize;
        let new = (self.selected as isize + delta).rem_euclid(len);
        self.selected = new as usize;
    }

    pub fn current_host(&self) -> Option<&Host> {
        self.filtered_indices
            .get(self.selected)
            .and_then(|idx| self.config.hosts.get(*idx))
    }

    fn rebuild_filter(&mut self) {
        if self.filter.is_empty() {
            self.filtered_indices = (0..self.config.hosts.len()).collect();
        } else {
            let mut scored: Vec<(i64, usize)> = Vec::new();
            for (i, host) in self.config.hosts.iter().enumerate() {
                let haystack = format!(
                    "{} {} {} {}",
                    host.name,
                    host.address,
                    host.tags.join(" "),
                    host.description.clone().unwrap_or_default()
                );
                if let Some(score) = self.matcher.fuzzy_match(&haystack, &self.filter) {
                    scored.push((score, i));
                }
            }
            scored.sort_by(|a, b| b.0.cmp(&a.0));
            self.filtered_indices = scored.into_iter().map(|(_, i)| i).collect();
        }
        if self.selected >= self.filtered_indices.len() {
            self.selected = self.filtered_indices.len().saturating_sub(1);
        }
    }

    fn save_host(&mut self, kind: FormKind, host: Host) -> Result<()> {
        let mut validation_config = self.config.clone();
        match kind {
            FormKind::Add => validation_config.hosts.push(host.clone()),
            FormKind::Edit => {
                if let Some(idx) = self.current_index() {
                    validation_config.hosts[idx] = host.clone();
                } else {
                    self.status = Some(StatusLine {
                        text: "No host selected to edit.".into(),
                        kind: StatusKind::Warn,
                    });
                    return Ok(());
                }
            }
        }
        Self::validate_bastions(&validation_config)?;

        match kind {
            FormKind::Add => {
                self.push_history();
                self.config.hosts.push(host.clone());
                self.status = Some(StatusLine {
                    text: format!("Added host {}.", host.name),
                    kind: StatusKind::Info,
                });
            }
            FormKind::Edit => {
                if let Some(idx) = self.current_index() {
                    self.push_history();
                    self.config.hosts[idx] = host.clone();
                    self.status = Some(StatusLine {
                        text: format!("Updated host {}.", host.name),
                        kind: StatusKind::Info,
                    });
                } else {
                    self.status = Some(StatusLine {
                        text: "No host selected to edit.".into(),
                        kind: StatusKind::Warn,
                    });
                    return Ok(());
                }
            }
        }
        self.store.save(&self.config)?;
        self.rebuild_filter();
        Ok(())
    }

    fn validate_bastions(config: &Config) -> Result<()> {
        for host in &config.hosts {
            if let Some(bastion_name) = &host.bastion {
                if bastion_name == &host.name {
                    bail!("Host '{}' cannot use itself as bastion.", host.name);
                }

                let mut seen: Vec<String> = vec![host.name.clone()];
                let mut current = bastion_name.as_str();
                loop {
                    if seen.iter().any(|h| h == current) {
                        bail!(
                            "Circular bastion reference detected involving '{}'.",
                            current
                        );
                    }
                    let Some(bastion) = config.find_host(current) else {
                        break;
                    };
                    seen.push(current.to_string());
                    let Some(next) = &bastion.bastion else { break };
                    current = next;
                }
            }
        }
        Ok(())
    }

    fn current_index(&self) -> Option<usize> {
        self.filtered_indices.get(self.selected).cloned()
    }

    fn delete_current(&mut self) -> Result<()> {
        if let Some(idx) = self.current_index() {
            let removed_name = self.config.hosts.get(idx).map(|h| h.name.clone());
            self.push_history();
            if let Some(name) = removed_name {
                self.status = Some(StatusLine {
                    text: format!("Removed {}.", name),
                    kind: StatusKind::Warn,
                });
            }
            self.config.hosts.remove(idx);
            self.store.save(&self.config)?;
            self.rebuild_filter();
            if self.selected >= self.filtered_indices.len() {
                self.selected = self.filtered_indices.len().saturating_sub(1);
            }
        }
        Ok(())
    }

    fn duplicate_host(&mut self, host: Host) -> Result<()> {
        let base = format!("{}-copy", host.name);
        let name = self.unique_name(&base);
        let mut new_host = host.clone();
        new_host.name = name.clone();
        self.push_history();
        self.config.hosts.push(new_host);
        self.store.save(&self.config)?;
        self.rebuild_filter();
        if let Some(pos) = self
            .filtered_indices
            .iter()
            .position(|i| self.config.hosts.get(*i).map(|h| &h.name) == Some(&name))
        {
            self.selected = pos;
        }
        self.status = Some(StatusLine {
            text: format!("Duplicated host to {}.", name),
            kind: StatusKind::Info,
        });
        Ok(())
    }

    fn quick_connect(&mut self, spec: SshSpec) -> Result<Option<AppAction>> {
        // Clear filter to ensure selection works after add/lookup.
        self.filter.clear();
        self.rebuild_filter();

        let target_idx = if let Some(idx) = self.find_host_by_spec(&spec) {
            self.status = Some(StatusLine {
                text: "Quick connect using existing host.".into(),
                kind: StatusKind::Info,
            });
            idx
        } else {
            self.push_history();
            let name_base = if let Some(user) = &spec.user {
                format!("{user}@{}", spec.address)
            } else {
                spec.address.clone()
            };
            let name = self.unique_name(&name_base);
            let host = Host {
                name: name.clone(),
                address: spec.address.clone(),
                user: spec.user.clone(),
                port: spec.port,
                key_path: spec.key_path.clone(),
                tags: Vec::new(),
                options: spec.options.clone(),
                remote_command: spec.remote_command.clone(),
                bastion: spec.bastion.clone(),
                description: None,
            };
            self.config.hosts.push(host);
            self.store.save(&self.config)?;
            self.rebuild_filter();
            self.status = Some(StatusLine {
                text: format!("Added {name} and connecting..."),
                kind: StatusKind::Info,
            });
            self.config
                .hosts
                .iter()
                .position(|h| h.name == name)
                .unwrap_or(0)
        };

        if let Some(pos) = self.filtered_indices.iter().position(|i| *i == target_idx) {
            self.selected = pos;
        }

        self.connect(None)
    }

    fn find_host_by_spec(&self, spec: &SshSpec) -> Option<usize> {
        self.config.hosts.iter().position(|h| {
            h.address == spec.address
                && h.user.as_deref() == spec.user.as_deref()
                && h.port == spec.port
                && h.options == spec.options
                && h.bastion.as_deref() == spec.bastion.as_deref()
                && h.remote_command.as_deref() == spec.remote_command.as_deref()
        })
    }

    fn unique_name(&self, base: &str) -> String {
        if !self.config.hosts.iter().any(|h| h.name == base) {
            return base.to_string();
        }
        let mut i = 2;
        loop {
            let cand = format!("{base}-{i}");
            if !self.config.hosts.iter().any(|h| h.name == cand) {
                return cand;
            }
            i += 1;
        }
    }

    fn push_history(&mut self) {
        self.history.push(self.config.clone());
        if self.history.len() > 20 {
            self.history.remove(0);
        }
    }

    fn undo(&mut self) -> Result<bool> {
        if let Some(prev) = self.history.pop() {
            self.config = prev;
            self.store.save(&self.config)?;
            self.rebuild_filter();
            return Ok(true);
        }
        Ok(false)
    }

    fn connect(&mut self, extra: Option<String>) -> Result<Option<AppAction>> {
        let Some(host) = self.current_host().cloned() else {
            self.status = Some(StatusLine {
                text: "No host selected.".into(),
                kind: StatusKind::Warn,
            });
            return Ok(None);
        };

        let preview = ssh::command_preview(
            &host,
            &self.config,
            self.config.default_key.as_deref(),
            extra.as_deref(),
        );

        if self.dry_run {
            self.status = Some(StatusLine {
                text: format!("Dry-run: {preview}"),
                kind: StatusKind::Info,
            });
            return Ok(None);
        }

        let cmd = ssh::build_command(
            &host,
            &self.config,
            self.config.default_key.as_deref(),
            extra.as_deref(),
        )?;
        self.status = Some(StatusLine {
            text: format!("Connecting with: {preview}"),
            kind: StatusKind::Info,
        });
        Ok(Some(AppAction::RunSsh(cmd)))
    }

    fn reload_config(&mut self) -> Result<()> {
        self.config = self
            .store
            .load_or_init()
            .with_context(|| "failed to reload config")?;
        self.rebuild_filter();
        self.status = Some(StatusLine {
            text: "Reloaded config.".into(),
            kind: StatusKind::Info,
        });
        Ok(())
    }

    pub fn help_entries() -> &'static [(&'static str, &'static str)] {
        &[
            ("/", "search"),
            ("Enter", "connect"),
            ("c", "connect with remote command"),
            ("g", "quick connect (ssh string)"),
            ("n", "new host"),
            ("e", "edit host"),
            ("d", "delete host"),
            ("y", "duplicate host"),
            ("u", "undo last change"),
            ("r", "reload config"),
            ("j/k or arrows", "move selection"),
            ("C", "toggle dry-run"),
            ("?", "show help"),
            ("a", "about/credits"),
            ("q", "quit"),
            ("Ctrl+C", "quit immediately"),
            ("Esc", "cancel modal/help"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_app() -> App {
        let dir = tempdir().unwrap();
        let store = ConfigStore::at(dir.path().join("config.toml"));
        let config = Config::sample();
        let mut app = App {
            mode: Mode::Normal,
            status: None,
            filter: String::new(),
            filtered_indices: Vec::new(),
            selected: 0,
            dry_run: false,
            form: None,
            confirm: None,
            quick_input: None,
            quick_cursor: 0,
            show_help: false,
            show_about: false,
            matcher: SkimMatcherV2::default(),
            config_path: store.path().to_path_buf(),
            config,
            history: Vec::new(),
            store,
        };
        app.rebuild_filter();
        app
    }

    #[test]
    fn filters_hosts_with_search() {
        let mut app = test_app();
        app.filter = "prod".into();
        app.rebuild_filter();
        assert!(!app.filtered_indices.is_empty());
        let first = app.filtered_indices[0];
        assert_eq!(app.config.hosts[first].name, "prod-web");
    }

    #[test]
    fn parses_ssh_string() {
        let spec = parse_ssh_spec("ssh -p 2201 -i ~/.ssh/key deploy@1.2.3.4").unwrap();
        assert_eq!(spec.address, "1.2.3.4");
        assert_eq!(spec.user.as_deref(), Some("deploy"));
        assert_eq!(spec.port, Some(2201));
        assert_eq!(spec.key_path.as_deref(), Some("~/.ssh/key"));
    }

    #[test]
    fn parses_options_after_host() {
        // Test that -p (port option) after host is parsed correctly, not as remote command
        let spec = parse_ssh_spec("host -p 3333").unwrap();
        assert_eq!(spec.address, "host");
        assert_eq!(spec.port, Some(3333));
        assert_eq!(spec.remote_command, None);

        // Test that any option after host is parsed correctly, not as remote command
        let spec = parse_ssh_spec("host -L 8080:localhost:80").unwrap();
        assert_eq!(spec.address, "host");
        assert!(spec.options.contains(&"-L".to_string()));
        assert!(spec.options.contains(&"8080:localhost:80".to_string()));
        assert_eq!(spec.remote_command, None);

        // Test that multiple options after host are parsed correctly
        let spec = parse_ssh_spec("host -o StrictHostKeyChecking=no -v").unwrap();
        assert_eq!(spec.address, "host");
        assert!(spec.options.contains(&"-o".to_string()));
        assert!(spec
            .options
            .contains(&"StrictHostKeyChecking=no".to_string()));
        assert!(spec.options.contains(&"-v".to_string()));
        assert_eq!(spec.remote_command, None);

        // Test that actual remote command after options is parsed correctly
        let spec = parse_ssh_spec("host -p 2222 uptime").unwrap();
        assert_eq!(spec.address, "host");
        assert_eq!(spec.port, Some(2222));
        assert_eq!(spec.remote_command.as_deref(), Some("uptime"));
    }

    #[test]
    fn rejects_self_bastion() {
        let app = test_app();
        let mut config = app.config.clone();
        if let Some(host) = config.hosts.first_mut() {
            host.bastion = Some(host.name.clone());
        }
        let err = App::validate_bastions(&config).unwrap_err();
        assert!(err.to_string().contains("cannot use itself as bastion"));
    }

    #[test]
    fn rejects_circular_bastions() {
        let app = test_app();
        let mut config = app.config.clone();
        if let Some(jump) = config.hosts.iter_mut().find(|h| h.name == "jump-eu") {
            jump.bastion = Some("staging-db".into());
        }
        let err = App::validate_bastions(&config).unwrap_err();
        assert!(err
            .to_string()
            .to_lowercase()
            .contains("circular bastion reference"));
    }

    #[test]
    fn allows_unknown_bastion_name() {
        let app = test_app();
        let mut config = app.config.clone();
        if let Some(host) = config.hosts.first_mut() {
            host.bastion = Some("external.example.com".into());
        }
        App::validate_bastions(&config).unwrap();
    }

    #[test]
    fn quick_connect_adds_or_reuses() {
        let mut app = test_app();
        app.dry_run = true; // avoid spawning ssh in tests
        let spec = parse_ssh_spec("ssh deploy@10.1.2.3").unwrap();
        let initial = app.config.hosts.len();
        app.quick_connect(spec.clone()).unwrap();
        assert_eq!(app.config.hosts.len(), initial + 1);

        // Duplicate should reuse
        app.quick_connect(spec).unwrap();
        assert_eq!(app.config.hosts.len(), initial + 1);
    }

    #[test]
    fn bastion_dropdown_excludes_current_host() {
        let config = Config::sample();
        let host = config.hosts[0].clone();
        let mut form = FormState::new(FormKind::Edit, Some(&host), &config);
        form.open_bastion_dropdown(&config);
        let dropdown = form.bastion_dropdown.as_ref().expect("dropdown opened");
        assert!(dropdown
            .filtered_indices
            .iter()
            .all(|i| config.hosts[*i].name != host.name));
    }
}
