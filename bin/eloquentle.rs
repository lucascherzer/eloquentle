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
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::{
    collections::HashSet,
    error::Error,
    io,
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
    best_guess: String, // Added for tracking best guess in knowledge pane
    turn: usize,
    feedback_input: String,
    loading_idx: usize,
    loading_frames: Vec<char>,
    loading_bar_pos: usize,
    loading_bar_direction: i8,
    loading_last_update: Instant,
    error_message: Option<String>,
    error_time: Option<Instant>,
    history: Vec<(String, String, f64)>, // (guess, feedback, info_gain)
    current_entropy: f64,
    quit: bool,
}

impl App {
    fn new() -> Self {
        let first_guess = get_best_first_guess().to_string();
        Self {
            state: AppState::Running,
            filter: Filter::default(),
            current_guess: first_guess.clone(),
            best_guess: first_guess,
            turn: 1,
            feedback_input: String::new(),
            loading_idx: 0,
            loading_frames: vec!['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'],
            loading_bar_pos: 0,
            loading_bar_direction: 1,
            loading_last_update: Instant::now(),
            error_message: None,
            error_time: None,
            history: Vec::new(),
            current_entropy: 5.87, // Hardcoded for first guess
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
        let first_guess = get_best_first_guess().to_string();
        self.current_guess = first_guess.clone();
        self.best_guess = first_guess;
        self.turn = 1;
        self.feedback_input.clear();
        self.state = AppState::Running;
        self.current_entropy = 5.87; // Hardcoded for first guess
        self.history = Vec::new();
    }

    fn toggle_solution_words(&mut self) {
        let currently_using = self.filter.uses_solution_words_only();
        self.filter.set_use_solution_words_only(!currently_using);
        self.filter.rebuild_from_info();
    }

    fn update_loading_animation(&mut self) {
        if self.loading_last_update.elapsed() > Duration::from_millis(100) {
            self.loading_idx = (self.loading_idx + 1) % self.loading_frames.len();

            // Update loading bar position
            if self.loading_bar_pos == 0 && self.loading_bar_direction == -1 {
                self.loading_bar_direction = 1;
            } else if self.loading_bar_pos == 10 && self.loading_bar_direction == 1 {
                self.loading_bar_direction = -1;
            }
            self.loading_bar_pos =
                ((self.loading_bar_pos as i8) + self.loading_bar_direction) as usize;

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

        // Save the current guess and feedback to history
        self.history.push((
            self.current_guess.clone(),
            self.feedback_input.clone(),
            self.current_entropy,
        ));

        // Enter loading state to calculate the next guess
        self.state = AppState::Loading;
        self.feedback_input.clear();

        Ok(())
    }

    fn show_error(&mut self, error: String) {
        self.error_message = Some(error);
        self.error_time = Some(Instant::now());
    }

    fn update_error_state(&mut self) {
        if let Some(time) = self.error_time {
            if time.elapsed() > Duration::from_secs(3) {
                self.error_message = None;
                self.error_time = None;
            }
        }
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

            // Calculate the next guess immediately to prevent hanging
            if app.loading_idx == 3 {
                // After a few frames of animation
                if app.filter.remaining_count() > 1 {
                    let (guess, entropy) = app.filter.recommend_guess();
                    app.current_guess = guess.clone();
                    app.best_guess = guess;
                    app.current_entropy = entropy;
                }
                app.state = AppState::Running;
            }
        }

        // Update error message if needed
        app.update_error_state();

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
                                    app.show_error(e);
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
                Constraint::Length(3), // Input area/Help text
            ]
            .as_ref(),
        )
        .split(size);

    // Title bar
    let title = Paragraph::new("eloquentle")
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Loading indicator moved to the Guess pane title

    // Status bar with remaining candidates and turn information
    let candidates_text = format!(
        "Turn: {} | Candidates: {}",
        app.turn,
        app.filter.remaining_count()
    );

    let status_block = Block::default().borders(Borders::ALL);
    f.render_widget(status_block.clone(), chunks[1]);

    // Calculate the inner area of the status block
    let inner_area = status_block.inner(chunks[1]);
    let gauge_area = Rect {
        x: inner_area.x,
        y: inner_area.y,
        width: inner_area.width,
        height: 1,
    };

    // Create the dictionary type indicator text
    let dictionary_text = if app.filter.uses_solution_words_only() {
        "Solution words only [s]"
    } else {
        "Full dictionary [s]"
    };

    let dictionary_indicator = Paragraph::new(dictionary_text)
        .style(if app.filter.uses_solution_words_only() {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Blue)
        })
        .alignment(Alignment::Center);

    f.render_widget(dictionary_indicator, gauge_area);

    let status_text = Paragraph::new(candidates_text)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);

    let status_text_area = Rect {
        x: inner_area.x,
        y: inner_area.y + 1,
        width: inner_area.width,
        height: 1,
    };

    f.render_widget(status_text, status_text_area);

    // Main content area split into two columns
    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(chunks[2]);

    // Left side: Current guess and feedback input
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(5)].as_ref())
        .split(horizontal_chunks[0]);

    // Current guess display with very compact layout
    // Build title with loading indicator if applicable
    let mut guess_title = if let AppState::GettingFeedback = app.state {
        String::from("Enter Feedback")
    } else {
        String::from("Guess")
    };

    // Add loading spinner to title if in loading state
    if let AppState::Loading = app.state {
        guess_title = format!("{} {}", guess_title, app.loading_frames[app.loading_idx]);
    }

    let guess_block = Block::default()
        .title(guess_title)
        .borders(Borders::ALL)
        .border_style(if let AppState::GettingFeedback = app.state {
            Style::default().fg(Color::Cyan)
        } else if let AppState::Loading = app.state {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    f.render_widget(guess_block.clone(), left_chunks[0]);

    let inner_area = guess_block.inner(left_chunks[0]);

    // Get the current guess text
    let current_guess_text = if app.filter.remaining_count() == 1 {
        app.filter.remaining_words()[0].to_string()
    } else if app.filter.remaining_count() == 0 {
        "?????".to_string()
    } else {
        app.current_guess.clone()
    };

    // Create a 2x5 grid for guess and feedback
    let cell_width: u16 = 3;
    let cell_height: u16 = 1;
    let grid_width = 5 * cell_width;
    let grid_start_x = inner_area.x + (inner_area.width.saturating_sub(grid_width)) / 2;
    let grid_start_y = inner_area.y;

    // Draw the current guess (top row)
    for (i, c) in current_guess_text.chars().enumerate() {
        let cell_x = grid_start_x + (i as u16 * cell_width);
        let cell = Rect::new(cell_x, grid_start_y, cell_width, cell_height);
        let char_style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);

        let cell_text = Paragraph::new(c.to_string())
            .style(char_style)
            .alignment(Alignment::Center);

        f.render_widget(cell_text, cell);
    }

    // If in loading state, show bouncing animation in the bottom row
    if let AppState::Loading = app.state {
        let loading_pos = app.loading_bar_pos.min(4);

        for i in 0..5u16 {
            let cell_x = grid_start_x + (i * cell_width);
            let cell = Rect::new(cell_x, grid_start_y + 1, cell_width, cell_height);

            let cell_text = if i as usize == loading_pos {
                Paragraph::new("▓").style(Style::default().fg(Color::Yellow))
            } else {
                Paragraph::new("░").style(Style::default().fg(Color::DarkGray))
            };

            f.render_widget(cell_text.alignment(Alignment::Center), cell);
        }
    }
    // Otherwise show feedback or empty slots in bottom row
    else if !app.feedback_input.is_empty() {
        for (i, c) in app.feedback_input.chars().enumerate() {
            let cell_x = grid_start_x + (i as u16 * cell_width);
            let cell = Rect::new(cell_x, grid_start_y + 1, cell_width, cell_height);

            let (feedback_char, style) = match c {
                'G' => ('G', Style::default().fg(Color::Green)),
                'Y' => ('Y', Style::default().fg(Color::Yellow)),
                'N' => ('N', Style::default().fg(Color::DarkGray)),
                _ => ('?', Style::default().fg(Color::White)),
            };

            let cell_text = Paragraph::new(feedback_char.to_string())
                .style(style)
                .alignment(Alignment::Center);

            f.render_widget(cell_text, cell);
        }

        // Draw empty slots for remaining positions
        for i in app.feedback_input.len()..5 {
            let cell_x = grid_start_x + (i as u16 * cell_width);
            let cell = Rect::new(cell_x, grid_start_y + 1, cell_width, cell_height);

            let cell_text = Paragraph::new("_")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center);

            f.render_widget(cell_text, cell);
        }
    }

    // In feedback mode, show instruction line below the grid
    if let AppState::GettingFeedback = app.state {
        let instruction_y = grid_start_y + 2;
        let instruction = Paragraph::new("G=Green | Y=Yellow | N=Gray")
            .style(Style::default().fg(Color::Cyan))
            .alignment(Alignment::Center);

        f.render_widget(
            instruction,
            Rect::new(inner_area.x, instruction_y, inner_area.width, 1),
        );
    }

    // Guess history display
    let mut history_lines = Vec::new();
    if app.history.is_empty() {
        history_lines.push(Line::from("No guesses yet"));
    } else {
        for (turn, (guess, feedback, info_gain)) in app.history.iter().enumerate() {
            let mut line_spans = Vec::new();
            line_spans.push(Span::styled(
                format!("{}. ", turn + 1),
                Style::default().fg(Color::White),
            ));

            // Add each letter with appropriate color based on feedback
            for (i, c) in guess.chars().enumerate() {
                let style = if i < feedback.len() {
                    match feedback.chars().nth(i).unwrap() {
                        'G' => Style::default().fg(Color::Green),
                        'Y' => Style::default().fg(Color::Yellow),
                        _ => Style::default().fg(Color::DarkGray),
                    }
                } else {
                    Style::default().fg(Color::White)
                };
                line_spans.push(Span::styled(c.to_string(), style));
            }

            // Add info gain value
            line_spans.push(Span::styled(
                format!(" ({:.2})", info_gain),
                Style::default().fg(Color::Cyan),
            ));

            history_lines.push(Line::from(line_spans));
        }
    }

    let history_area = Paragraph::new(Text::from(history_lines))
        .block(Block::default().title("History").borders(Borders::ALL));

    f.render_widget(history_area, left_chunks[1]);

    // When getting feedback, we enhance the instructions in the bottom help area
    // but we don't show a floating popup anymore - all feedback is handled inline
    // Right side: Knowledge display and remaining words sample
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
        .split(horizontal_chunks[1]);

    // Knowledge display - improved visual representation
    let info_set = app.filter.get_info();

    // Create a word outline with known/unknown letters
    let mut known_letters = ['_'; 5];
    let mut yellow_letters: Vec<(char, usize)> = Vec::new();
    let mut not_in_word: Vec<char> = Vec::new();

    for info in info_set {
        match info {
            Info::Correct(c, pos) => known_letters[*pos] = *c,
            Info::NotAt(c, pos) => yellow_letters.push((*c, *pos)),
            Info::Not(c) => not_in_word.push(*c),
        }
    }

    // Show current recommendation with entropy
    let recommendation = Line::from(vec![
        Span::styled("Best guess: ", Style::default().fg(Color::White)),
        Span::styled(
            app.best_guess.clone(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" (info gain: {:.2})", app.current_entropy),
            Style::default().fg(Color::White),
        ),
    ]);

    // Create a visual word outline
    let word_outline = Line::from(vec![
        Span::styled("Word: ", Style::default().fg(Color::White)),
        Span::styled(
            format!(
                "[ {} ] ",
                known_letters
                    .iter()
                    .map(|c| if *c == '_' { ' ' } else { *c })
                    .collect::<String>()
            ),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    // Create a keyboard-like visualization
    let mut knowledge_text = vec![
        Line::from(""),
        Line::from(recommendation),
        Line::from(""),
        Line::from(word_outline),
        Line::from(""),
    ];

    // Add letters in word but wrong position
    if !yellow_letters.is_empty() {
        let mut in_word_chars = yellow_letters.iter().map(|(c, _)| *c).collect::<Vec<_>>();
        in_word_chars.sort();
        in_word_chars.dedup();

        knowledge_text.push(Line::from(vec![
            Span::styled("In word: ", Style::default().fg(Color::White)),
            Span::styled(
                in_word_chars.iter().collect::<String>(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    // Add letters not in word
    if !not_in_word.is_empty() {
        let mut not_chars = not_in_word.clone();
        not_chars.sort();
        not_chars.dedup();

        knowledge_text.push(Line::from(vec![
            Span::styled("Not in word: ", Style::default().fg(Color::White)),
            Span::styled(
                not_chars.iter().collect::<String>(),
                Style::default().fg(Color::Red),
            ),
        ]));
    }

    // Create letter-by-letter position visualization
    knowledge_text.push(Line::from(""));
    knowledge_text.push(Line::from("Position knowledge:"));

    for pos in 0..5 {
        let mut position_spans = vec![Span::styled(
            format!("Pos {}: ", pos + 1),
            Style::default().fg(Color::White),
        )];

        // Check if we have a correct letter for this position
        if known_letters[pos] != '_' {
            position_spans.push(Span::styled(
                format!("{}", known_letters[pos]),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        // Otherwise show letters that are known not to be in this position
        else {
            let not_at_pos: Vec<_> = yellow_letters
                .iter()
                .filter(|(_, p)| *p == pos)
                .map(|(c, _)| *c)
                .collect();

            if !not_at_pos.is_empty() {
                position_spans.push(Span::styled(
                    format!("not {}", not_at_pos.iter().collect::<String>()),
                    Style::default().fg(Color::Yellow),
                ));
            } else {
                position_spans.push(Span::styled("?", Style::default().fg(Color::Gray)));
            }
        }

        knowledge_text.push(Line::from(position_spans));
    }

    let knowledge_paragraph = Paragraph::new(knowledge_text)
        .block(Block::default().title("Knowledge").borders(Borders::ALL));

    f.render_widget(knowledge_paragraph, right_chunks[0]);

    // Sample of remaining words
    let remaining_words = app.filter.remaining_words();
    let sample_count = remaining_words.len().min(10);

    let mut sample_text = Vec::new();

    if !remaining_words.is_empty() {
        let words_per_line = 5;
        let lines_needed = (sample_count + words_per_line - 1) / words_per_line;

        for i in 0..lines_needed {
            let start = i * words_per_line;
            let end = (start + words_per_line).min(sample_count);
            sample_text.push(Line::from(remaining_words[start..end].join("  ")));
        }

        if remaining_words.len() > sample_count {
            sample_text.push(Line::from(format!(
                "+ {} more...",
                remaining_words.len() - sample_count
            )));
        }
    } else {
        sample_text.push(Line::from("No words match the criteria"));
    }

    let words_sample = Paragraph::new(Text::from(sample_text))
        .block(
            Block::default()
                .title(format!("Candidates ({})", remaining_words.len()))
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(words_sample, right_chunks[1]);

    // Bottom status line with help text or error message
    let mut help_style = Style::default().fg(Color::Gray);
    let help_text = if let Some(error) = &app.error_message {
        help_style = Style::default().fg(Color::Red);
        error.clone()
    } else {
        match app.state {
            AppState::GettingFeedback => {
                "G=green | Y=yellow | N=gray | Enter=submit | Esc=cancel".to_string()
            }
            _ => "f=enter feedback | r=reset | s=toggle solutions | q=quit".to_string(),
        }
    };

    let help = Paragraph::new(help_text)
        .style(help_style)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[3]);
}
