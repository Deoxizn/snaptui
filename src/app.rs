use std::collections::{HashMap, HashSet};
use std::time::Instant;

use ratatui::widgets::{ListState, TableState};

use crate::fmt::username;
use crate::snapper::{friendly_error, SnapperConfig, SnapperDb, Snapshot};
use crate::theme::{Theme, ThemeSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Snapshots,
    Configs,
    Limits,
}

impl Tab {
    pub fn next(self) -> Self {
        match self {
            Self::Snapshots => Self::Configs,
            Self::Configs => Self::Limits,
            Self::Limits => Self::Snapshots,
        }
    }
}

pub enum Popup {
    ConfirmDelete { ids: Vec<u32> },
    CreateSnapshot { description: String, cleanup: String, focus: usize, error: Option<String> },
    EditSnapshot { id: u32, description: String, cleanup: String, focus: usize },
    EditLimit { key: String, value: String },
    Message { text: String },
}

impl Clone for Popup {
    fn clone(&self) -> Self {
        match self {
            Self::ConfirmDelete { ids } => Self::ConfirmDelete { ids: ids.clone() },
            Self::CreateSnapshot { description, cleanup, focus, error } => Self::CreateSnapshot {
                description: description.clone(),
                cleanup: cleanup.clone(),
                focus: *focus,
                error: error.clone(),
            },
            Self::EditSnapshot { id, description, cleanup, focus } => Self::EditSnapshot {
                id: *id,
                description: description.clone(),
                cleanup: cleanup.clone(),
                focus: *focus,
            },
            Self::EditLimit { key, value } => Self::EditLimit {
                key: key.clone(),
                value: value.clone(),
            },
            Self::Message { text } => Self::Message { text: text.clone() },
        }
    }
}

pub enum Command {
    RunSudo(Vec<String>),
    None,
}

pub const LIMIT_KEYS: [(&str, bool); 17] = [
    ("NUMBER_LIMIT", true),
    ("NUMBER_LIMIT_IMPORTANT", true),
    ("NUMBER_MIN_AGE", true),
    ("TIMELINE_CREATE", true),
    ("TIMELINE_CLEANUP", true),
    ("TIMELINE_MIN_AGE", true),
    ("TIMELINE_LIMIT_HOURLY", true),
    ("TIMELINE_LIMIT_DAILY", true),
    ("TIMELINE_LIMIT_WEEKLY", true),
    ("TIMELINE_LIMIT_MONTHLY", true),
    ("TIMELINE_LIMIT_YEARLY", true),
    ("EMPTY_GROUP", true),
    ("EMPTY_SERIES_PRE_POST", true),
    ("SPACE_LIMIT", true),
    ("FREE_LIMIT", true),
    ("ALLOW_USERS", true),
    ("ALLOW_GROUPS", true),
];

pub struct App {
    pub theme: Theme,
    pub theme_source: Option<ThemeSource>,
    pub db: SnapperDb,
    pub configs: Vec<SnapperConfig>,
    pub config_idx: usize,
    pub snapshots: Vec<Snapshot>,
    pub used: HashMap<u32, u64>,
    pub users: HashMap<u32, String>,
    pub sizes_unknown: bool,
    pub tab: Tab,
    pub snapshot_state: TableState,
    pub config_state: ListState,
    pub limit_state: ListState,
    pub limit_dirty: HashSet<String>,
    pub selected: HashSet<u32>,
    pub popup: Option<Popup>,
    pub status: String,
    pub status_at: Option<Instant>,
    pub fs_free: u64,
    pub fs_total: u64,
    pub quit: bool,
}

impl App {
    pub fn new(db: SnapperDb, theme: Theme, theme_source: Option<ThemeSource>) -> Self {
        let mut app = Self {
            theme,
            theme_source,
            db,
            configs: Vec::new(),
            config_idx: 0,
            snapshots: Vec::new(),
            used: HashMap::new(),
            users: HashMap::new(),
            sizes_unknown: false,
            tab: Tab::Snapshots,
            snapshot_state: TableState::default(),
            config_state: ListState::default(),
            limit_state: ListState::default(),
            limit_dirty: HashSet::new(),
            selected: HashSet::new(),
            popup: None,
            status: String::new(),
            status_at: None,
            fs_free: 0,
            fs_total: 0,
            quit: false,
        };
        app.refresh_all();
        app.clamp_config_idx();
        app
    }

    pub fn current_config(&self) -> Option<&SnapperConfig> {
        self.configs.get(self.config_idx)
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
        self.status_at = Some(Instant::now());
    }

    pub fn refresh_all(&mut self) {
        self.refresh_fs();
        match self.db.list_configs() {
            Ok(configs) => {
                if configs != self.configs {
                    self.configs = configs;
                    if self.config_idx >= self.configs.len() {
                        self.config_idx = 0;
                    }
                    self.limit_dirty.clear();
                }
            }
            Err(e) => {
                self.set_status(friendly_error(&e));
                return;
            }
        }
        self.refresh_snapshots();
        self.refresh_fs();
    }

    pub fn refresh_snapshots(&mut self) {
        let config = match self.current_config() {
            Some(c) => c.name.clone(),
            None => {
                self.snapshots.clear();
                return;
            }
        };
        match self.db.list_snapshots(&config) {
            Ok(snaps) => {
                self.snapshots = snaps;
                self.users = self
                    .snapshots
                    .iter()
                    .map(|s| (s.uid, username(s.uid)))
                    .collect();
                self.selected.retain(|id| self.snapshots.iter().any(|s| s.id == *id));
                let idx = self.snapshot_state.selected().unwrap_or(0);
                self.snapshot_state.select(Some(idx.min(self.snapshots.len().saturating_sub(1))));
                let mut used_ok = 0;
                let mut used_err = 0;
                for snap in &self.snapshots {
                    match self.db.get_used_space(&config, snap.id) {
                        Ok(bytes) => {
                            self.used.insert(snap.id, bytes);
                            used_ok += 1;
                        }
                        Err(_) => {
                            self.used.remove(&snap.id);
                            used_err += 1;
                        }
                    }
                }
                self.sizes_unknown = used_err > 0 && used_ok == 0;
                if self.sizes_unknown {
                    self.set_status("quota disabled — sizes unknown, run: sudo btrfs quota enable /");
                }
            }
            Err(e) => {
                self.snapshots.clear();
                self.set_status(friendly_error(&e));
            }
        }
    }

    fn refresh_fs(&mut self) {
        let path = self
            .current_config()
            .map(|c| std::path::PathBuf::from(&c.subvolume))
            .unwrap_or_else(|| std::path::PathBuf::from("/"));
        if let Some((free, total)) = statvfs_free_total(&path) {
            self.fs_free = free;
            self.fs_total = total;
        }
    }

    fn clamp_config_idx(&mut self) {
        if self.config_idx >= self.configs.len() {
            self.config_idx = self.configs.len().saturating_sub(1);
        }
        self.config_state.select(Some(self.config_idx));
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Command {
        if let Some(popup) = self.popup.take() {
            let result = self.handle_popup_key(popup, key);
            return result;
        }
        self.handle_main_key(key)
    }

    fn handle_popup_key(&mut self, popup: Popup, key: crossterm::event::KeyEvent) -> Command {
        match popup {
            Popup::ConfirmDelete { ids } => match key.code {
                crossterm::event::KeyCode::Char('y') | crossterm::event::KeyCode::Char('Y') => {
                    let config = self.current_config().map(|c| c.name.clone());
                    if let (Some(config), Some(db)) = (config, Some(&self.db)) {
                        match db.delete(&config, &ids) {
                            Ok(_) => {
                                self.set_status(format!("deleted {} snapshot(s)", ids.len()));
                                self.selected.clear();
                            }
                            Err(e) => self.set_status(friendly_error(&e)),
                        }
                    }
                    self.refresh_snapshots();
                    Command::None
                }
                crossterm::event::KeyCode::Char('n')
                | crossterm::event::KeyCode::Char('N')
                | crossterm::event::KeyCode::Esc => Command::None,
                _ => {
                    self.popup = Some(Popup::ConfirmDelete { ids });
                    Command::None
                }
            },
            Popup::CreateSnapshot { mut description, mut cleanup, mut focus, error } => {
                match key.code {
                    crossterm::event::KeyCode::Esc => Command::None,
                    crossterm::event::KeyCode::Enter => {
                        if focus == 0 && description.is_empty() {
                            self.popup = Some(Popup::CreateSnapshot {
                                description,
                                cleanup,
                                focus,
                                error: Some("description required".into()),
                            });
                            return Command::None;
                        }
                        let config = self.current_config().map(|c| c.name.clone());
                        if let (Some(config), Some(db)) = (config, Some(&self.db)) {
                            match db.create_single(&config, &description, &cleanup) {
                                Ok(id) => {
                                    self.set_status(format!("created snapshot #{id}"));
                                    self.refresh_snapshots();
                                }
                                Err(e) => {
                                    self.popup = Some(Popup::CreateSnapshot {
                                        description,
                                        cleanup,
                                        focus,
                                        error: Some(friendly_error(&e)),
                                    });
                                    return Command::None;
                                }
                            }
                        }
                        Command::None
                    }
                    crossterm::event::KeyCode::Tab => {
                        focus = (focus + 1) % 2;
                        self.popup = Some(Popup::CreateSnapshot {
                            description,
                            cleanup,
                            focus,
                            error,
                        });
                        Command::None
                    }
                    crossterm::event::KeyCode::Char(c) => {
                        if focus == 0 {
                            description.push(c);
                        } else {
                            cleanup.push(c);
                        }
                        self.popup = Some(Popup::CreateSnapshot {
                            description,
                            cleanup,
                            focus,
                            error,
                        });
                        Command::None
                    }
                    crossterm::event::KeyCode::Backspace => {
                        let buf = if focus == 0 { &mut description } else { &mut cleanup };
                        buf.pop();
                        self.popup = Some(Popup::CreateSnapshot {
                            description,
                            cleanup,
                            focus,
                            error,
                        });
                        Command::None
                    }
                    _ => {
                        self.popup = Some(Popup::CreateSnapshot {
                            description,
                            cleanup,
                            focus,
                            error,
                        });
                        Command::None
                    }
                }
            }
            Popup::EditSnapshot { id, mut description, mut cleanup, mut focus } => {
                match key.code {
                    crossterm::event::KeyCode::Esc => Command::None,
                    crossterm::event::KeyCode::Enter => {
                        let config = self.current_config().map(|c| c.name.clone());
                        if let (Some(config), Some(db)) = (config, Some(&self.db)) {
                            match db.set_snapshot(&config, id, &description, &cleanup) {
                                Ok(_) => {
                                    self.set_status(format!("updated snapshot #{id}"));
                                    self.refresh_snapshots();
                                }
                                Err(e) => self.set_status(friendly_error(&e)),
                            }
                        }
                        Command::None
                    }
                    crossterm::event::KeyCode::Tab => {
                        focus = (focus + 1) % 2;
                        self.popup = Some(Popup::EditSnapshot { id, description, cleanup, focus });
                        Command::None
                    }
                    crossterm::event::KeyCode::Char(c) => {
                        let buf = if focus == 0 { &mut description } else { &mut cleanup };
                        buf.push(c);
                        self.popup = Some(Popup::EditSnapshot { id, description, cleanup, focus });
                        Command::None
                    }
                    crossterm::event::KeyCode::Backspace => {
                        let buf = if focus == 0 { &mut description } else { &mut cleanup };
                        buf.pop();
                        self.popup = Some(Popup::EditSnapshot { id, description, cleanup, focus });
                        Command::None
                    }
                    _ => {
                        self.popup = Some(Popup::EditSnapshot { id, description, cleanup, focus });
                        Command::None
                    }
                }
            }
            Popup::EditLimit { key: key_name, mut value } => {
                match key.code {
                    crossterm::event::KeyCode::Esc => Command::None,
                    crossterm::event::KeyCode::Enter => {
                        self.apply_limit(&key_name, &value);
                        Command::None
                    }
                    crossterm::event::KeyCode::Char(c) => {
                        value.push(c);
                        self.popup = Some(Popup::EditLimit { key: key_name, value });
                        Command::None
                    }
                    crossterm::event::KeyCode::Backspace => {
                        value.pop();
                        self.popup = Some(Popup::EditLimit { key: key_name, value });
                        Command::None
                    }
                    _ => {
                        self.popup = Some(Popup::EditLimit { key: key_name, value });
                        Command::None
                    }
                }
            }
            Popup::Message { .. } => {
                if matches!(key.code, crossterm::event::KeyCode::Enter | crossterm::event::KeyCode::Esc
                    | crossterm::event::KeyCode::Char('q'))
                {
                    Command::None
                } else {
                    self.popup = Some(popup);
                    Command::None
                }
            }
        }
    }

    fn apply_limit(&mut self, key: &str, value: &str) {
        if let Some(config) = self.current_config() {
            let mut values = config.values.clone();
            let prev = values.insert(key.to_string(), value.to_string());
            self.configs[self.config_idx].values = values;
            if prev.as_deref() != Some(value) {
                self.limit_dirty.insert(key.to_string());
            }
        }
    }

    fn handle_main_key(&mut self, key: crossterm::event::KeyEvent) -> Command {
        use crossterm::event::{KeyCode, KeyModifiers};

        if let KeyCode::Char('q') = key.code {
            if self.limit_dirty.is_empty() {
                self.quit = true;
            } else {
                self.popup = Some(Popup::Message {
                    text: "unsaved limit changes — press 's' to save or 'x' to discard".into(),
                });
            }
            return Command::None;
        }

        match key.code {
            KeyCode::Tab => {
                self.tab = self.tab.next();
                self.clamp_config_idx();
            }
            KeyCode::Char('r') => {
                self.refresh_all();
                if !self.sizes_unknown {
                    self.set_status("refreshed");
                }
            }
            KeyCode::Char('t') => {
                self.tab = self.tab.next();
                self.clamp_config_idx();
            }
            _ => {}
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return Command::None;
        }

        match self.tab {
            Tab::Snapshots => self.handle_snapshots_key(key),
            Tab::Configs => self.handle_configs_key(key),
            Tab::Limits => self.handle_limits_key(key),
        }
    }

    fn handle_snapshots_key(&mut self, key: crossterm::event::KeyEvent) -> Command {
        use crossterm::event::KeyCode;
        let len = self.snapshots.len();
        let idx = self.snapshot_state.selected().unwrap_or(0);

        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if len > 0 {
                    let next = (idx + 1).min(len - 1);
                    self.snapshot_state.select(Some(next));
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.snapshot_state.select(Some(idx.saturating_sub(1)));
            }
            KeyCode::Home | KeyCode::Char('g') => self.snapshot_state.select(Some(0)),
            KeyCode::End | KeyCode::Char('G') => {
                self.snapshot_state.select(Some(len.saturating_sub(1)))
            }
            KeyCode::Char(' ') => {
                if let Some(snap) = self.snapshots.get(idx) {
                    if !self.selected.remove(&snap.id) {
                        self.selected.insert(snap.id);
                    }
                }
            }
            KeyCode::Char('a') => {
                self.selected = self.snapshots.iter().map(|s| s.id).collect();
            }
            KeyCode::Char('c') => {
                self.popup = Some(Popup::CreateSnapshot {
                    description: String::new(),
                    cleanup: "number".into(),
                    focus: 0,
                    error: None,
                });
            }
            KeyCode::Char('d') => {
                let mut ids: Vec<u32> = self.selected.iter().copied().collect();
                if ids.is_empty() {
                    if let Some(snap) = self.snapshots.get(idx) {
                        ids.push(snap.id);
                    }
                }
                ids.sort();
                if !ids.is_empty() {
                    self.popup = Some(Popup::ConfirmDelete { ids });
                }
            }
            KeyCode::Char('e') => {
                if let Some(snap) = self.snapshots.get(idx) {
                    self.popup = Some(Popup::EditSnapshot {
                        id: snap.id,
                        description: snap.description.clone(),
                        cleanup: snap.cleanup.clone(),
                        focus: 0,
                    });
                }
            }
            KeyCode::Char('i') => {
                return self.init_storage();
            }
            KeyCode::Esc => self.selected.clear(),
            _ => {}
        }
        Command::None
    }

    fn handle_configs_key(&mut self, key: crossterm::event::KeyEvent) -> Command {
        use crossterm::event::KeyCode;
        let len = self.configs.len();
        let idx = self.config_state.selected().unwrap_or(self.config_idx);

        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if len > 0 {
                    let next = (idx + 1).min(len - 1);
                    self.config_idx = next;
                    self.config_state.select(Some(next));
                    self.refresh_snapshots();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let next = idx.saturating_sub(1);
                self.config_idx = next;
                self.config_state.select(Some(next));
                self.refresh_snapshots();
            }
            KeyCode::Enter => {
                self.tab = Tab::Snapshots;
                self.refresh_snapshots();
            }
            KeyCode::Char('i') => {
                return self.init_storage();
            }
            _ => {}
        }
        Command::None
    }

    fn init_storage(&mut self) -> Command {
        let Some(config) = self.current_config() else {
            return Command::None;
        };
        let subvol = config.subvolume.trim_end_matches('/').to_string();
        let snap_dir = if subvol.is_empty() {
            "/.snapshots".to_string()
        } else {
            format!("{subvol}/.snapshots")
        };
        self.set_status(format!("initializing snapshot storage at {snap_dir}…"));
        Command::RunSudo(vec![
            "btrfs".into(),
            "subvolume".into(),
            "create".into(),
            snap_dir,
        ])
    }

    fn handle_limits_key(&mut self, key: crossterm::event::KeyEvent) -> Command {
        use crossterm::event::KeyCode;
        let len = LIMIT_KEYS.len();
        let idx = self.limit_state.selected().unwrap_or(0);

        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.limit_state.select(Some((idx + 1).min(len - 1)));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.limit_state.select(Some(idx.saturating_sub(1)));
            }
            KeyCode::Enter | KeyCode::Char('e') => {
                let (key, editable) = LIMIT_KEYS[idx];
                if editable {
                    let value = self.current_limit_value(key);
                    self.popup = Some(Popup::EditLimit { key: key.into(), value });
                }
            }
            KeyCode::Char('s') => {
                return self.save_limits();
            }
            KeyCode::Char('x') => {
                if !self.limit_dirty.is_empty() {
                    self.reload_limits();
                    self.set_status("limit changes discarded");
                }
            }
            KeyCode::Char('r') => {
                self.reload_limits();
                self.set_status("limits reloaded");
            }
            _ => {}
        }
        Command::None
    }

    pub fn current_limit_value(&self, key: &str) -> String {
        self.current_config()
            .and_then(|c| c.values.get(key).cloned())
            .unwrap_or_else(|| "unset".into())
    }

    fn reload_limits(&mut self) {
        if let Ok(configs) = self.db.list_configs() {
            self.configs = configs;
            self.clamp_config_idx();
        }
        self.limit_dirty.clear();
    }

    fn save_limits(&mut self) -> Command {
        if self.limit_dirty.is_empty() {
            self.set_status("no changes to save");
            return Command::None;
        }
        let config = self.current_config().map(|c| c.name.clone());
        let mut args = Vec::new();
        if let Some(config) = config {
            args.push("-c".to_string());
            args.push(config);
        }
        args.push("set-config".to_string());
        for key in &self.limit_dirty {
            args.push(format!("{key}={}", self.current_limit_value(key)));
        }
        if args.len() < 3 {
            self.set_status("no changes to save");
            return Command::None;
        }
        self.set_status("saving limits via sudo…");
        Command::RunSudo(args)
    }

    pub fn after_sudo(&mut self, ok: bool) {
        if ok {
            if self.limit_dirty.is_empty() {
                self.set_status("snapshot storage initialized");
            } else {
                self.set_status("limits saved");
            }
            self.limit_dirty.clear();
            self.reload_limits();
            self.refresh_snapshots();
            self.refresh_fs();
        } else {
            self.set_status("operation failed");
        }
    }
}

pub fn statvfs_free_total(path: &std::path::Path) -> Option<(u64, u64)> {
    use std::ffi::CString;
    let cpath = CString::new(path.to_string_lossy().as_bytes()).ok()?;
    let mut buf: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(cpath.as_ptr(), &mut buf) } != 0 {
        return None;
    }
    let block = buf.f_frsize as u64;
    let free = (buf.f_bavail as u64).saturating_mul(block);
    let total = (buf.f_blocks as u64).saturating_mul(block);
    Some((free, total))
}
