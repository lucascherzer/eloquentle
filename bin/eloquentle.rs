use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use eloquentle::{
    filter::{Filter, get_best_first_guess},
    info::Info,
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};
use std::{
    collections::HashSet,
    error::Error,
    io, thread,
    time::{Duration, Instant},
};

enum AppState {
    Running,
    GettingFeedback,
    Loading,
}

struct App {
    state: AppState,
    filter: Filter,
    current_guess: String,
    turn: usize,
    feedback_input: String,
    loading_idx: usize,
    loading_frames: Vec<char>,
    loading_last_update: Instant,
    quit: bool,
}

impl App {
    fn new() -> Self {
        Self {
            state: AppState::Running,
            filter: Filter::default(),
            current_guess: get_best_first_guess().to_string(),
            turn: 1,
            feedback_input: String::new(),
            loading_idx: 0,
            loading_frames: vec!['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'],
            loading_last_update: Instant::now(),
            quit: false,
        }
    }

    fn reset(&mut self) {
        let use_solution_words = self.filter.uses_solution_words_only();
        if use_solution_words {
            self.filter = Filter::new_with_solution_words();
        } else {
            self.filter = Filter::default();
        }
        self.current_guess = get_best_first_guess().to_string();
        self.turn = 1;
        self.feedback_input.clear();
        self.state = AppState::Running;
    }

    fn toggle_solution_words(&mut self) {
        let currently_using = self.filter.uses_solution_words_only();
        self.filter.set_use_solution_words_only(!currently_using);
        self.filter.rebuild_from_info();
    }

    fn update_loading_animation(&mut self) {
        if self.loading_last_update.elapsed() > Duration::from_millis(100) {
            self.loading_idx = (self.loading_idx + 1) % self.loading_frames.len();
            self.loading_last_update = Instant::now();
        }
    }

    fn submit_feedback(&mut self) -> Result<(), String> {
        if self.feedback_input.len() != 5 {
            return Err("Feedback must be exactly 5 characters".to_string());
        }

        let info_set = process_feedback(&self.current_guess, &self.feedback_input)?;
        self.filter.add_info_set(&info_set);
        self.turn += 1;

        self.state = AppState::Loading;

        // In a real implementation, we would spawn a thread here to calculate the next guess
        // For now, we'll just simulate a delay
        thread::sleep(Duration::from_millis(500));

        // After "calculation" is done, update the current guess and return to Running state
        if self.filter.remaining_count() <= 1 {
            // We're done - keep the state as Running so we can show the final result
        } else {
            self.current_guess = self.filter.recommend_guess();
        }
        self.state = AppState::Running;
        self.feedback_input.clear();

        Ok(())
    }
}

fn process_feedback(guess: &str, feedback: &str) -> Result<HashSet<Info>, String> {
    if feedback.len() != 5 || guess.len() != 5 {
        return Err("Invalid feedback or guess length".to_string());
    }

    let mut info_set = HashSet::new();
    let guess_chars: Vec<char> = guess.chars().collect();

    for (i, feedback_char) in feedback.chars().enumerate() {
        let guess_char = guess_chars[i];

        match feedback_char.to_ascii_uppercase() {
            'G' => {
                info_set.insert(Info::Correct(guess_char, i));
            }
            'Y' => {
                info_set.insert(Info::NotAt(guess_char, i));
            }
            'N' => {
                // Check if this letter appears elsewhere with G or Y feedback
                let is_elsewhere = feedback.chars().enumerate().any(|(j, c)| {
                    j != i
                        && guess_chars[j] == guess_char
                        && (c.to_ascii_uppercase() == 'G' || c.to_ascii_uppercase() == 'Y')
                });

                if !is_elsewhere {
                    info_set.insert(Info::Not(guess_char));
                }
            }
            _ => return Err("Feedback must only contain G, Y, or N".to_string()),
        }
    }

    Ok(info_set)
}

fn main() -> Result<(), Box<dyn Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new();

    // Main loop
    while !app.quit {
        terminal.draw(|f| ui(f, &mut app))?;

        // Update loading animation if in loading state
        if let AppState::Loading = app.state {
            app.update_loading_animation();
        }

        // Handle input
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.state {
                        AppState::Running => match key.code {
                            KeyCode::Char('q') => app.quit = true,
                            KeyCode::Char('r') => app.reset(),
                            KeyCode::Char('s') => app.toggle_solution_words(),
                            KeyCode::Char('f') => app.state = AppState::GettingFeedback,
                            _ => {}
                        },
                        AppState::GettingFeedback => match key.code {
                            KeyCode::Char(c) => {
                                if "GYNgyn".contains(c) && app.feedback_input.len() < 5 {
                                    app.feedback_input.push(c.to_ascii_uppercase());
                                }
                            }
                            KeyCode::Backspace => {
                                app.feedback_input.pop();
                            }
                            KeyCode::Enter => {
                                if let Err(e) = app.submit_feedback() {
                                    // TODO(ui): Show error message to user
                                    println!("Error: {}", e);
                                }
                            }
                            KeyCode::Esc => {
                                app.feedback_input.clear();
                                app.state = AppState::Running;
                            }
                            _ => {}
                        },
                        AppState::Loading => {
                            // No input processing during loading state
                        }
                    }
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    let size = f.size();

    // Create the layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(3), // Title
                Constraint::Length(3), // Status bar
                Constraint::Min(10),   // Main content area
                Constraint::Length(3), // Input area
            ]
            .as_ref(),
        )
        .split(size);

    // Title bar
    let title = Paragraph::new("ELOQUENTLE WORDLE SOLVER")
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Status bar with remaining candidates and turn information
    let candidates_text = format!(
        "Turn: {} | Candidates: {} | {}",
        app.turn,
        app.filter.remaining_count(),
        if app.filter.uses_solution_words_only() {
            "Using solution words only"
        } else {
            "Using full dictionary"
        }
    );
    let status = Paragraph::new(candidates_text)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(status, chunks[1]);

    // Main content area split into two columns
    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(chunks[2]);

    // Left side: Current guess and feedback input
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(horizontal_chunks[0]);

    // Current guess display
    let current_guess_text = match app.state {
        AppState::Loading => {
            format!(
                "Calculating next guess {}...",
                app.loading_frames[app.loading_idx]
            )
        }
        _ => {
            if app.filter.remaining_count() == 1 {
                format!("Solved! The word is: {}", app.filter.remaining_words()[0])
            } else if app.filter.remaining_count() == 0 {
                "No words match the given constraints!".to_string()
            } else {
                format!("Current guess: {}", app.current_guess)
            }
        }
    };
    let current_guess = Paragraph::new(current_guess_text)
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().title("Guess").borders(Borders::ALL));
    f.render_widget(current_guess, left_chunks[0]);

    // Feedback input area - shows differently based on state
    let feedback_area = match app.state {
        AppState::GettingFeedback => {
            let mut lines = vec![Line::from("Enter feedback (G=Green, Y=Yellow, N=Gray):")];

            // Show the current guess with the feedback input so far
            let mut input_display = String::new();
            for (i, c) in app.current_guess.chars().enumerate() {
                if i < app.feedback_input.len() {
                    match app.feedback_input.chars().nth(i).unwrap() {
                        'G' => input_display.push_str(&format!("[{}:G] ", c)),
                        'Y' => input_display.push_str(&format!("[{}:Y] ", c)),
                        'N' => input_display.push_str(&format!("[{}:N] ", c)),
                        _ => input_display.push_str(&format!("[{}:?] ", c)),
                    }
                } else {
                    input_display.push_str(&format!("[{}:_] ", c));
                }
            }
            lines.push(Line::from(input_display));

            // Instructions
            lines.push(Line::from("Press Enter to submit, Esc to cancel"));

            Paragraph::new(Text::from(lines))
                .style(Style::default().fg(Color::Cyan))
                .block(
                    Block::default()
                        .title("Enter Feedback")
                        .borders(Borders::ALL),
                )
        }
        _ => {
            let lines = vec![
                Line::from("Press keys to:"),
                Line::from("  [f] Enter feedback for current guess"),
                Line::from("  [r] Reset the game"),
                Line::from("  [s] Toggle solution words only"),
                Line::from("  [q] Quit"),
            ];
            Paragraph::new(Text::from(lines))
                .style(Style::default().fg(Color::White))
                .block(Block::default().title("Controls").borders(Borders::ALL))
        }
    };
    f.render_widget(feedback_area, left_chunks[1]);

    // Right side: Knowledge display and remaining words sample
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
        .split(horizontal_chunks[1]);

    // Knowledge display
    // TODO(ui): Implement a more visually pleasing way to display the knowledge
    let info_set = app.filter.get_info();
    let mut knowledge_items: Vec<ListItem> = Vec::new();

    // Group infos by type for better visualization
    let mut correct_info = vec![];
    let mut not_at_info = vec![];
    let mut not_in_word_info = vec![];

    for info in info_set {
        match info {
            Info::Correct(c, pos) => correct_info.push(format!("{}@{}", c, pos)),
            Info::NotAt(c, pos) => not_at_info.push(format!("{}!@{}", c, pos)),
            Info::Not(c) => not_in_word_info.push(format!("{}", c)),
        }
    }

    if !correct_info.is_empty() {
        knowledge_items.push(ListItem::new(Span::styled(
            format!("Correct positions: {}", correct_info.join(", ")),
            Style::default().fg(Color::Green),
        )));
    }

    if !not_at_info.is_empty() {
        knowledge_items.push(ListItem::new(Span::styled(
            format!("Wrong positions: {}", not_at_info.join(", ")),
            Style::default().fg(Color::Yellow),
        )));
    }

    if !not_in_word_info.is_empty() {
        knowledge_items.push(ListItem::new(Span::styled(
            format!("Not in word: {}", not_in_word_info.join(", ")),
            Style::default().fg(Color::Red),
        )));
    }

    if knowledge_items.is_empty() {
        knowledge_items.push(ListItem::new("No knowledge yet"));
    }

    let knowledge_list =
        List::new(knowledge_items).block(Block::default().title("Knowledge").borders(Borders::ALL));
    f.render_widget(knowledge_list, right_chunks[0]);

    // Sample of remaining words
    let remaining_words = app.filter.remaining_words();
    let sample_count = remaining_words.len().min(5);
    let sample_text = if !remaining_words.is_empty() {
        format!(
            "Sample ({}): {}",
            sample_count,
            remaining_words[..sample_count].join(", ")
        )
    } else {
        "No words match the criteria".to_string()
    };

    let words_sample = Paragraph::new(sample_text)
        .block(
            Block::default()
                .title("Remaining Words")
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(words_sample, right_chunks[1]);

    // Bottom status line or help text
    let help_text = match app.state {
        AppState::GettingFeedback => "Enter G (green), Y (yellow), or N (gray) for each letter",
        _ => "Use the keyboard to navigate | See Controls panel for available commands",
    };

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[3]);
}
