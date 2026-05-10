use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Cell, Clear, Paragraph, Row, Table};
use unicode_width::UnicodeWidthStr;

use crate::app::state::App;

pub fn ui(f: &mut ratatui::Frame, app: &mut App) {
    let base_chunks = layout_chunks(f.area(), app.selected_mode);
    let header_area = *base_chunks.last().unwrap_or(&base_chunks[0]);

    let header = Paragraph::new(if app.input_mode {
        format!("Filter: {}  | Enter/Esc: finish | q: quit", app.search)
    } else {
        format!(
            "f - filter:{} | u - update | c/m/p - sort | Enter - Detailed View | v - select text {} | ctrl+q quit anyproc",
            app.search,
            if app.text_select_mode { "ON" } else { "OFF" }
        )
    })
    .block(Block::default());
    f.render_widget(header, header_area);

    if app.input_mode {
        let cursor_idx = app.search_cursor.min(app.search.len());
        let left = &app.search[..cursor_idx];
        let x = header_area.x + ("Filter: ".width() + left.width()) as u16;
        let y = header_area.y;
        f.set_cursor_position((x, y));
    }

    let rows = app.rows.iter().enumerate().map(|(i, p)| {
        let mut row = Row::new(vec![
            Cell::from(p.pid.as_u32().to_string()),
            Cell::from(format!("{:.1}", p.cpu)),
            Cell::from(p.mem_mb.to_string()),
            Cell::from(p.name.clone()),
            Cell::from(p.cmd.clone()),
        ]);

        if app.selected_mode && app.selected_pid == Some(p.pid) {
            row = row.style(Style::default().bg(Color::Green).fg(Color::Black));
        } else if i == app.focus_idx {
            row = row.style(Style::default().bg(Color::Blue).fg(Color::White));
        }

        row
    });

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(36),
            Constraint::Min(20),
        ],
    )
    .header(
        Row::new(vec!["PID", "CPU%", "MEM(MB)", "PROCESS", "CMD"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(Block::default().title("Processes"));

    f.render_stateful_widget(table, base_chunks[0], &mut app.state);

    if app.selected_mode {
        let area = base_chunks[2];
        f.render_widget(Clear, area);
        let details = Paragraph::new(app.details_text())
            .wrap(ratatui::widgets::Wrap { trim: false })
            .scroll((app.details_scroll, 0))
            .block(Block::default().title("Process Details"));
        f.render_widget(details, area);
    }
}

pub fn layout_chunks(area: Rect, show_details: bool) -> Vec<Rect> {
    if show_details {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10),
                Constraint::Length(1),
                Constraint::Length(10),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(area)
            .to_vec()
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Length(1), Constraint::Length(1)])
            .split(area)
            .to_vec()
    }
}

pub fn in_rect(x: u16, y: u16, rect: Rect) -> bool {
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}

pub fn row_from_mouse(
    x: u16,
    y: u16,
    table_area: Rect,
    row_count: usize,
    table_offset: usize,
) -> Option<usize> {
    if !in_rect(x, y, table_area) || row_count == 0 {
        return None;
    }

    // With table title rendered, effective data starts after title+header rows.
    let rows_start_y = table_area.y.saturating_add(2);
    let rows_end_y = table_area.y + table_area.height;

    if y < rows_start_y || y >= rows_end_y {
        return None;
    }

    let idx = table_offset + (y - rows_start_y) as usize;
    if idx < row_count {
        Some(idx)
    } else {
        None
    }
}

pub fn visible_rows_in_table(table_area: Rect) -> usize {
    // Reserve one row for title/header overlap + one for header.
    table_area.height.saturating_sub(2) as usize
}
