mod state;
mod ui;

use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use self::state::{App, SortMode};
use self::ui::{in_rect, layout_chunks, row_from_mouse, ui, visible_rows_in_table};

const TICK_RATE: Duration = Duration::from_secs(1);

pub fn run() -> io::Result<()> {
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(
        io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;

    let res = run_app(&mut terminal);

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    res
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut app = App::new();
    let mut last_tick = Instant::now();
    let mut mouse_capture_enabled = true;

    loop {
        sync_mouse_capture(app.text_select_mode, &mut mouse_capture_enabled)?;

        terminal.draw(|f| ui(f, &mut app))?;

        let timeout = TICK_RATE.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if handle_key_event(key, &mut app, terminal)? {
                        break;
                    }
                }
                Event::Mouse(mouse) => handle_mouse_event(mouse, &mut app, terminal, mouse_capture_enabled)?,
                _ => {}
            }
        }

        if last_tick.elapsed() >= TICK_RATE {
            app.refresh_rows();
            last_tick = Instant::now();
        }
    }

    Ok(())
}

fn sync_mouse_capture(text_select_mode: bool, mouse_capture_enabled: &mut bool) -> io::Result<()> {
    let should_enable = !text_select_mode;
    if should_enable == *mouse_capture_enabled {
        return Ok(());
    }

    if should_enable {
        crossterm::execute!(io::stdout(), crossterm::event::EnableMouseCapture)?;
    } else {
        crossterm::execute!(io::stdout(), crossterm::event::DisableMouseCapture)?;
    }
    *mouse_capture_enabled = should_enable;
    Ok(())
}

fn handle_key_event(
    key: KeyEvent,
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> io::Result<bool> {
    if key.kind != KeyEventKind::Press {
        return Ok(false);
    }

    if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Ok(true);
    }

    if app.input_mode {
        handle_input_mode_key(key, app);
        return Ok(false);
    }

    let size = terminal.size()?;
    let chunks = layout_chunks(size.into(), app.selected_mode);
    let table_area = chunks[0];
    let visible_rows = visible_rows_in_table(table_area);

    match key.code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Down => app.next(visible_rows),
        KeyCode::Up => app.prev(visible_rows),
        KeyCode::Char('k') => app.handle_k_press(visible_rows),
        KeyCode::Char('v') => app.text_select_mode = !app.text_select_mode,
        KeyCode::Char('f') => {
            app.input_mode = true;
            app.search_cursor = app.search.len();
        }
        KeyCode::Char('u') => app.refresh_rows(),
        KeyCode::Char('c') => {
            app.sort_mode = SortMode::Cpu;
            app.refresh_rows();
        }
        KeyCode::Char('m') => {
            app.sort_mode = SortMode::Mem;
            app.refresh_rows();
        }
        KeyCode::Char('p') => {
            app.sort_mode = SortMode::Pid;
            app.refresh_rows();
        }
        KeyCode::Enter => app.select_focused(),
        KeyCode::Esc => app.clear_selection(),
        _ => {}
    }

    Ok(false)
}

fn handle_input_mode_key(key: KeyEvent, app: &mut App) {
    match key.code {
        KeyCode::Esc => app.input_mode = false,
        KeyCode::Left => app.move_search_cursor_left(),
        KeyCode::Right => app.move_search_cursor_right(),
        KeyCode::Home => app.search_cursor = 0,
        KeyCode::End => app.search_cursor = app.search.len(),
        KeyCode::Backspace => app.backspace_search_char(),
        KeyCode::Delete => app.delete_search_char(),
        KeyCode::Enter => app.input_mode = false,
        KeyCode::Char(c) => app.insert_search_char(c),
        _ => {}
    }
}

fn handle_mouse_event(
    mouse: MouseEvent,
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mouse_capture_enabled: bool,
) -> io::Result<()> {
    if !mouse_capture_enabled {
        return Ok(());
    }

    let size = terminal.size()?;
    let chunks = layout_chunks(size.into(), app.selected_mode);
    let table_area = chunks[0];
    let visible_rows = visible_rows_in_table(table_area);
    let details_area = if app.selected_mode && chunks.len() > 4 {
        Some(chunks[2])
    } else {
        None
    };

    match mouse.kind {
        MouseEventKind::ScrollDown => {
            if in_rect(mouse.column, mouse.row, table_area) {
                app.scroll_table(1, visible_rows);
            } else if let Some(area) = details_area {
                if in_rect(mouse.column, mouse.row, area) {
                    app.scroll_details_down();
                }
            }
        }
        MouseEventKind::ScrollUp => {
            if in_rect(mouse.column, mouse.row, table_area) {
                app.scroll_table(-1, visible_rows);
            } else if let Some(area) = details_area {
                if in_rect(mouse.column, mouse.row, area) {
                    app.scroll_details_up();
                }
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(idx) = row_from_mouse(
                mouse.column,
                mouse.row,
                table_area,
                app.rows.len(),
                app.state.offset(),
            ) {
                app.handle_table_click(idx);
            }
        }
        _ => {}
    }

    Ok(())
}
