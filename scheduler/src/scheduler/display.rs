use super::{runner::RunnerEvent, Scheduler};
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use std::{
    io::{self, Stdout},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table},
    Terminal,
};

pub enum DisplayEvent {
    Input(KeyEvent),
    Tick,
}

const TICK_RATE: Duration = Duration::from_millis(200);

pub struct DisplayTerminal {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    input_rx: Receiver<DisplayEvent>,
}

impl DisplayTerminal {
    pub fn new() -> Result<Self, io::Error> {
        crossterm::terminal::enable_raw_mode()?;

        // Set up the input handling thread
        let (input_tx, input_rx) = mpsc::channel();
        thread::spawn(move || {
            let mut last_tick = Instant::now();
            loop {
                let timeout = TICK_RATE
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or(Duration::ZERO);

                if event::poll(timeout).expect("Failed to poll events.") {
                    if let Event::Key(key) = event::read().expect("Failed to read events.") {
                        input_tx
                            .send(DisplayEvent::Input(key))
                            .expect("Failed to send input events.");
                    }
                }

                if last_tick.elapsed() >= TICK_RATE && input_tx.send(DisplayEvent::Tick).is_ok() {
                    last_tick = Instant::now();
                }
            }
        });

        // Set up the terminal-user-interface
        let backend = CrosstermBackend::new(io::stdout());
        let terminal = Terminal::new(backend)?;

        Ok(Self { terminal, input_rx })
    }

    pub fn draw<S>(&mut self, scheduler: &S, process_output: String)
    where
        S: Scheduler,
    {
        let current_process = scheduler.current_process();

        // Draw the tui to the terminal
        self.terminal
            .draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([Constraint::Length(3), Constraint::Min(5)])
                    .split(f.size());

                let process = Paragraph::new(match current_process {
                    Some(process) => format!(
                        "{} | {} | Output: \"{}\"",
                        process.pid(),
                        process.name(),
                        process_output
                    ),
                    None => "No task is currently running.".to_owned(),
                })
                .style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::LightBlue),
                )
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Current Task")
                        .border_type(BorderType::Rounded),
                );

                f.render_widget(process, chunks[0]);

                let cpu_elapsed = scheduler.cpu_elapsed();
                let items = scheduler.processes().iter().map(|process| {
                    Row::new(vec![
                        Cell::from(process.pid().to_string())
                            .style(Style::default().add_modifier(Modifier::BOLD)),
                        Cell::from("|"),
                        Cell::from(process.name()),
                        Cell::from("|"),
                        Cell::from(process.niceness().to_string()),
                        Cell::from("|"),
                        Cell::from(process.cpu_usage_percentage(cpu_elapsed)),
                    ])
                });

                let table = Table::new(items)
                    .header(
                        Row::new(vec!["PID", "|", "Name", "|", "Niceness", "|", "CPU"])
                            .style(Style::default().add_modifier(Modifier::BOLD)),
                    )
                    .widths(&[
                        Constraint::Length(3),
                        Constraint::Length(1),
                        Constraint::Length(20),
                        Constraint::Length(1),
                        Constraint::Length(8),
                        Constraint::Length(1),
                        Constraint::Length(3),
                    ])
                    .block(Block::default().title(S::NAME).borders(Borders::ALL))
                    .style(Style::default().fg(Color::LightGreen))
                    .column_spacing(1);

                f.render_widget(table, chunks[1]);
            })
            .expect("Failed to draw frame.");
    }

    pub fn get_input(&self) -> RunnerEvent {
        // Get the user's input and return a matching event
        match self
            .input_rx
            .recv()
            .expect("Failed to recieve input events.")
        {
            DisplayEvent::Input(key) => {
                if key.modifiers.is_empty() {
                    match key.code {
                        KeyCode::Char('q') => return RunnerEvent::Quit,
                        KeyCode::Char('p') => return RunnerEvent::Pause,
                        KeyCode::Char('r') => return RunnerEvent::Resume,
                        KeyCode::Char('s') => return RunnerEvent::Step,
                        _ => {}
                    };
                }
            }
            DisplayEvent::Tick => {}
        }
        RunnerEvent::None
    }
}
