use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Terminal,
};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

use be_core::dispatcher::BeeEvent;

#[derive(Clone, PartialEq)]
pub enum BeeState {
    Queued,
    Running,
    Done,
    Failed,
}

pub struct GridApp {
    pub states: Vec<BeeState>,
    pub completed: usize,
    pub failed: usize,
    pub total: usize,
    pub cost_so_far: f64,
    pub job_id: String,
    pub bee_name: String,
    pub model: String,
    pub done: bool,
}

impl GridApp {
    pub fn new(total: usize, job_id: String, bee_name: String, model: String) -> Self {
        Self {
            states: vec![BeeState::Queued; total],
            completed: 0,
            failed: 0,
            total,
            cost_so_far: 0.0,
            job_id,
            bee_name,
            model,
            done: false,
        }
    }

    pub fn update(&mut self, event: &BeeEvent) {
        match event {
            BeeEvent::Started { index, .. } => {
                if *index < self.states.len() {
                    self.states[*index] = BeeState::Running;
                }
            }
            BeeEvent::Completed { index, .. } => {
                if *index < self.states.len() {
                    self.states[*index] = BeeState::Done;
                }
                self.completed += 1;
            }
            BeeEvent::Failed { index, .. } => {
                if *index < self.states.len() {
                    self.states[*index] = BeeState::Failed;
                }
                self.failed += 1;
            }
            BeeEvent::JobDone { .. } => {
                self.done = true;
            }
        }
    }
}

pub async fn run_grid(
    mut rx: mpsc::Receiver<BeeEvent>,
    total: usize,
    job_id: String,
    bee_name: String,
    model: String,
) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = Arc::new(Mutex::new(GridApp::new(total, job_id, bee_name, model)));
    let app_clone = Arc::clone(&app);

    // Spawn event receiver
    let recv_handle = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let mut a = app_clone.lock().unwrap();
            a.update(&event);
            if a.done {
                break;
            }
        }
    });

    loop {
        {
            let a = app.lock().unwrap();
            terminal.draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(5),
                        Constraint::Length(4),
                        Constraint::Length(2),
                    ])
                    .split(f.size());

                // Header
                let header = Paragraph::new(format!(
                    "Job {} · {} · {}",
                    &a.job_id[..8], a.bee_name, a.model
                ))
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().add_modifier(Modifier::BOLD));
                f.render_widget(header, chunks[0]);

                // Bee grid
                let cols = 25usize;
                let mut lines: Vec<Line> = Vec::new();
                for chunk in a.states.chunks(cols) {
                    let spans: Vec<Span> = chunk
                        .iter()
                        .map(|s| match s {
                            BeeState::Queued  => Span::styled("○ ", Style::default().fg(Color::DarkGray)),
                            BeeState::Running => Span::styled("● ", Style::default().fg(Color::Cyan)),
                            BeeState::Done    => Span::styled("✓ ", Style::default().fg(Color::Green)),
                            BeeState::Failed  => Span::styled("✗ ", Style::default().fg(Color::Red)),
                        })
                        .collect();
                    lines.push(Line::from(spans));
                }
                let grid = Paragraph::new(lines)
                    .block(Block::default().borders(Borders::ALL).title(" Bees "));
                f.render_widget(grid, chunks[1]);

                // Progress bar
                let done = a.completed + a.failed;
                let pct = if a.total > 0 { done as f64 / a.total as f64 } else { 1.0 };
                let gauge = Gauge::default()
                    .block(Block::default().borders(Borders::ALL))
                    .gauge_style(Style::default().fg(Color::Green))
                    .ratio(pct)
                    .label(format!(
                        "{}/{} · ✓ {} · ✗ {} · cost ${:.4}",
                        done, a.total, a.completed, a.failed, a.cost_so_far
                    ));
                f.render_widget(gauge, chunks[2]);

                // Footer
                let footer = Paragraph::new("[Q] quit  [P] pause  [R] retry failed")
                    .style(Style::default().fg(Color::DarkGray));
                f.render_widget(footer, chunks[3]);
            })?;
        }

        // Check if done
        if app.lock().unwrap().done {
            break;
        }

        // Handle keyboard input
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => break,
                        _ => {}
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    recv_handle.abort();
    Ok(())
}
