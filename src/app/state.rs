use std::time::{Duration, Instant};

use ratatui::widgets::TableState;
use sysinfo::{Pid, ProcessesToUpdate, Signal, System};

#[derive(Clone)]
pub struct ProcRow {
    pub pid: Pid,
    pub name: String,
    pub cpu: f32,
    pub mem_mb: u64,
    pub cmd: String,
}

#[derive(Clone, Copy)]
pub enum SortMode {
    Cpu,
    Mem,
    Pid,
}

pub struct App {
    pub sys: System,
    pub rows: Vec<ProcRow>,
    pub state: TableState,
    pub focus_idx: usize,
    pub selected_mode: bool,
    pub selected_pid: Option<Pid>,
    pub details_scroll: u16,
    pub text_select_mode: bool,
    pub sort_mode: SortMode,
    pub search: String,
    pub search_cursor: usize,
    pub input_mode: bool,
    pub last_k_press: Option<Instant>,
    pub last_mouse_click: Option<(usize, Instant)>,
}

impl App {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        let mut app = Self {
            sys,
            rows: Vec::new(),
            state: TableState::default(),
            focus_idx: 0,
            selected_mode: false,
            selected_pid: None,
            details_scroll: 0,
            text_select_mode: false,
            sort_mode: SortMode::Cpu,
            search: String::new(),
            search_cursor: 0,
            input_mode: false,
            last_k_press: None,
            last_mouse_click: None,
        };
        app.refresh_rows();
        app
    }

    pub fn refresh_rows(&mut self) {
        self.sys.refresh_processes(ProcessesToUpdate::All, true);

        let tokens: Vec<String> = self
            .search
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();

        let mut rows: Vec<ProcRow> = self
            .sys
            .processes()
            .iter()
            .filter_map(|(pid, p)| {
                let name = p.name().to_string_lossy().to_string();
                let cmd = p
                    .cmd()
                    .iter()
                    .map(|part| part.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ");
                let haystack = format!("{} {}", name.to_lowercase(), cmd.to_lowercase());

                let matches = if tokens.is_empty() {
                    true
                } else {
                    tokens.iter().any(|t| haystack.contains(t))
                };

                if matches {
                    Some(ProcRow {
                        pid: *pid,
                        name,
                        cpu: p.cpu_usage(),
                        mem_mb: p.memory() / 1024 / 1024,
                        cmd,
                    })
                } else {
                    None
                }
            })
            .collect();

        match self.sort_mode {
            SortMode::Cpu => {
                rows.sort_by(|a, b| {
                    b.cpu
                        .partial_cmp(&a.cpu)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            SortMode::Mem => rows.sort_by(|a, b| b.mem_mb.cmp(&a.mem_mb)),
            SortMode::Pid => rows.sort_by(|a, b| b.pid.as_u32().cmp(&a.pid.as_u32())),
        }
        self.rows = rows;

        if self.rows.is_empty() {
            self.focus_idx = 0;
            self.selected_mode = false;
            self.selected_pid = None;
            self.details_scroll = 0;
            *self.state.offset_mut() = 0;
            return;
        }

        if self.focus_idx >= self.rows.len() {
            self.focus_idx = self.rows.len() - 1;
        }

        if let Some(pid) = self.selected_pid {
            if !self.rows.iter().any(|r| r.pid == pid) {
                self.selected_mode = false;
                self.selected_pid = None;
            }
        }

        let max_offset = self.rows.len().saturating_sub(1);
        if self.state.offset() > max_offset {
            *self.state.offset_mut() = max_offset;
        }
    }

    pub fn selected(&self) -> Option<&ProcRow> {
        let pid = self.selected_pid?;
        self.rows.iter().find(|r| r.pid == pid)
    }

    pub fn next(&mut self, visible_rows: usize) {
        if self.rows.is_empty() {
            return;
        }
        self.focus_idx = (self.focus_idx + 1).min(self.rows.len() - 1);
        self.ensure_focus_visible(visible_rows);
    }

    pub fn prev(&mut self, visible_rows: usize) {
        if self.rows.is_empty() {
            return;
        }
        self.focus_idx = self.focus_idx.saturating_sub(1);
        self.ensure_focus_visible(visible_rows);
    }

    pub fn select_focused(&mut self) {
        if let Some(row) = self.rows.get(self.focus_idx) {
            self.selected_pid = Some(row.pid);
            self.selected_mode = true;
            self.details_scroll = 0;
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_mode = false;
        self.selected_pid = None;
        self.details_scroll = 0;
    }

    fn clamp_search_cursor(&mut self) {
        if self.search_cursor > self.search.len() {
            self.search_cursor = self.search.len();
        }
    }

    pub fn move_search_cursor_left(&mut self) {
        self.clamp_search_cursor();
        if self.search_cursor > 0 {
            self.search_cursor = self.search[..self.search_cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    pub fn move_search_cursor_right(&mut self) {
        self.clamp_search_cursor();
        if self.search_cursor < self.search.len() {
            self.search_cursor = self.search[self.search_cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.search_cursor + i)
                .unwrap_or(self.search.len());
        }
    }

    pub fn insert_search_char(&mut self, c: char) {
        self.clamp_search_cursor();
        self.search.insert(self.search_cursor, c);
        self.search_cursor += c.len_utf8();
        self.refresh_rows();
    }

    pub fn backspace_search_char(&mut self) {
        self.clamp_search_cursor();
        if self.search_cursor == 0 {
            return;
        }
        let prev = self.search[..self.search_cursor]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.search.drain(prev..self.search_cursor);
        self.search_cursor = prev;
        self.refresh_rows();
    }

    pub fn delete_search_char(&mut self) {
        self.clamp_search_cursor();
        if self.search_cursor >= self.search.len() {
            return;
        }
        let next = self.search[self.search_cursor..]
            .char_indices()
            .nth(1)
            .map(|(i, _)| self.search_cursor + i)
            .unwrap_or(self.search.len());
        self.search.drain(self.search_cursor..next);
        self.refresh_rows();
    }

    fn target_pid_for_kill(&self) -> Option<Pid> {
        if self.selected_mode {
            return self.selected_pid;
        }
        self.rows.get(self.focus_idx).map(|r| r.pid)
    }

    pub fn kill_target_sigkill(&mut self) {
        let Some(pid) = self.target_pid_for_kill() else {
            return;
        };
        if let Some(proc_ref) = self.sys.process(pid) {
            let _ = proc_ref.kill_with(Signal::Kill);
        }
        self.refresh_rows();
    }

    pub fn handle_k_press(&mut self, visible_rows: usize) {
        let now = Instant::now();
        let quick_double_k = self
            .last_k_press
            .map(|t| now.duration_since(t) <= Duration::from_millis(450))
            .unwrap_or(false);

        if quick_double_k {
            self.kill_target_sigkill();
            self.last_k_press = None;
        } else {
            self.prev(visible_rows);
            self.last_k_press = Some(now);
        }
    }

    pub fn handle_table_click(&mut self, idx: usize) {
        self.focus_idx = idx;
        let now = Instant::now();
        let is_double_click = self
            .last_mouse_click
            .map(|(last_idx, t)| last_idx == idx && now.duration_since(t) <= Duration::from_millis(350))
            .unwrap_or(false);

        if is_double_click {
            self.select_focused();
            self.last_mouse_click = None;
        } else {
            self.last_mouse_click = Some((idx, now));
        }
    }

    pub fn details_text(&self) -> String {
        if let Some(proc_row) = self.selected() {
            if let Some(proc_ref) = self.sys.process(proc_row.pid) {
                let exe = proc_ref
                    .exe()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "-".to_string());
                let cwd = proc_ref
                    .cwd()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "-".to_string());
                let status = format!("{:?}", proc_ref.status());
                let cmd = proc_ref
                    .cmd()
                    .iter()
                    .map(|part| part.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ");

                return format!(
                    "PID: {}\nName: {}\nCPU: {:.1}%\nMemory: {} MB\nStatus: {}\nExe: {}\nCwd: {}\nCmd: {}",
                    proc_row.pid.as_u32(),
                    proc_row.name,
                    proc_ref.cpu_usage(),
                    proc_ref.memory() / 1024 / 1024,
                    status,
                    exe,
                    cwd,
                    if cmd.is_empty() { "-".to_string() } else { cmd }
                );
            }
        }
        "No process selected".to_string()
    }

    pub fn scroll_table(&mut self, delta: i32, visible_rows: usize) {
        if self.rows.is_empty() {
            *self.state.offset_mut() = 0;
            return;
        }

        let visible = visible_rows.max(1);
        let max_offset = self.rows.len().saturating_sub(visible);
        let current = self.state.offset() as i32;
        let next = (current + delta).clamp(0, max_offset as i32) as usize;
        *self.state.offset_mut() = next;
    }

    fn ensure_focus_visible(&mut self, visible_rows: usize) {
        let visible = visible_rows.max(1);
        let offset = self.state.offset();
        if self.focus_idx < offset {
            *self.state.offset_mut() = self.focus_idx;
            return;
        }
        let end = offset + visible;
        if self.focus_idx >= end {
            *self.state.offset_mut() = self.focus_idx + 1 - visible;
        }
    }

    pub fn scroll_details_up(&mut self) {
        self.details_scroll = self.details_scroll.saturating_sub(1);
    }

    pub fn scroll_details_down(&mut self) {
        self.details_scroll = self.details_scroll.saturating_add(1);
    }
}
