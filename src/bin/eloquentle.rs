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
    ManualKnowledgeMenu,
    ManualDirect,
    ManualIndirect,
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
    // Manual knowledge entry fields
    manual_input_buffer: String,
    manual_word: String,
    manual_feedback: String,
    manual_mode_step: usize, // 0 = word input, 1 = feedback input
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
            manual_input_buffer: String::new(),
            manual_word: String::new(),
            manual_feedback: String::new(),
            manual_mode_step: 0,
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
        if let Some(time) = self.error_time
            && time.elapsed() > Duration::from_secs(3)
        {
            self.error_message = None;
            self.error_time = None;
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
                        && (c.eq_ignore_ascii_case(&'G') || c.eq_ignore_ascii_case(&'Y'))
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

/// Parse direct Info input from string format
/// Supports formats:
/// - Not(x) or x@- or -x
/// - Correct(a, 2) or a@2
/// - NotAt(b, 3) or b!@3
///   Positions are 1-indexed for user input (converted to 0-indexed internally)
fn parse_info_string(input: &str) -> Result<HashSet<Info>, String> {
    let mut info_set = HashSet::new();

    // Split by comma but be smart about it - only split outside parentheses
    let entries = split_by_comma_outside_parens(input);

    for entry in entries {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }

        // Try to parse different formats
        let info = if let Some(parsed) = parse_not_format(entry) {
            parsed
        } else if let Some(parsed) = parse_correct_format(entry) {
            parsed
        } else if let Some(parsed) = parse_notat_format(entry) {
            parsed
        } else if let Some(parsed) = parse_shorthand_format(entry) {
            parsed
        } else {
            return Err(format!(
                "Invalid format: '{}'. Expected: Not(x), Correct(a, 2), NotAt(b, 3), a@2, b!@3, or -x",
                entry
            ));
        };

        info_set.insert(info);
    }

    if info_set.is_empty() {
        return Err("No valid Info entries found".to_string());
    }

    Ok(info_set)
}

fn split_by_comma_outside_parens(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut paren_depth = 0;

    for c in s.chars() {
        match c {
            '(' => {
                paren_depth += 1;
                current.push(c);
            }
            ')' => {
                paren_depth -= 1;
                current.push(c);
            }
            ',' if paren_depth == 0 => {
                if !current.trim().is_empty() {
                    result.push(current.trim().to_string());
                }
                current.clear();
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.trim().is_empty() {
        result.push(current.trim().to_string());
    }

    result
}

fn parse_not_format(s: &str) -> Option<Info> {
    // Match: Not(x) or not(x) or -x
    if s.starts_with('-') && s.len() == 2 {
        let c = s.chars().nth(1)?;
        if c.is_ascii_lowercase() {
            return Some(Info::Not(c));
        }
    }

    let s_lower = s.to_lowercase();
    if s_lower.starts_with("not(") && s_lower.ends_with(')') {
        let inner = &s[4..s.len() - 1].trim();
        if inner.len() == 1 {
            let c = inner.chars().next()?;
            if c.is_ascii_lowercase() {
                return Some(Info::Not(c));
            }
        }
    }
    None
}

fn parse_correct_format(s: &str) -> Option<Info> {
    // Match: Correct(a, 2) or correct(a, 2) or a@2
    if let Some(_at_pos) = s.find('@')
        && !s.contains('!')
    {
        let parts: Vec<&str> = s.split('@').collect();
        if parts.len() == 2 {
            let c = parts[0].trim();
            let pos_str = parts[1].trim();
            if c.len() == 1
                && c.chars().next()?.is_ascii_lowercase()
                && let Ok(pos) = pos_str.parse::<usize>()
                && (1..=5).contains(&pos)
            {
                return Some(Info::Correct(c.chars().next()?, pos - 1));
            }
        }
    }

    let s_lower = s.to_lowercase();
    if s_lower.starts_with("correct(") && s_lower.ends_with(')') {
        let inner = &s[8..s.len() - 1];
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() == 2 {
            let c = parts[0].trim();
            let pos_str = parts[1].trim();
            if c.len() == 1
                && c.chars().next()?.is_ascii_lowercase()
                && let Ok(pos) = pos_str.parse::<usize>()
                && (1..=5).contains(&pos)
            {
                return Some(Info::Correct(c.chars().next()?, pos - 1));
            }
        }
    }
    None
}

fn parse_notat_format(s: &str) -> Option<Info> {
    // Match: NotAt(b, 3) or notat(b, 3) or b!@3
    if s.contains("!@") {
        let parts: Vec<&str> = s.split("!@").collect();
        if parts.len() == 2 {
            let c = parts[0].trim();
            let pos_str = parts[1].trim();
            if c.len() == 1
                && c.chars().next()?.is_ascii_lowercase()
                && let Ok(pos) = pos_str.parse::<usize>()
                && (1..=5).contains(&pos)
            {
                return Some(Info::NotAt(c.chars().next()?, pos - 1));
            }
        }
    }

    let s_lower = s.to_lowercase();
    if s_lower.starts_with("notat(") && s_lower.ends_with(')') {
        let inner = &s[6..s.len() - 1];
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() == 2 {
            let c = parts[0].trim();
            let pos_str = parts[1].trim();
            if c.len() == 1
                && c.chars().next()?.is_ascii_lowercase()
                && let Ok(pos) = pos_str.parse::<usize>()
                && (1..=5).contains(&pos)
            {
                return Some(Info::NotAt(c.chars().next()?, pos - 1));
            }
        }
    }
    None
}

fn parse_shorthand_format(_s: &str) -> Option<Info> {
    // Additional shorthand that might be intuitive
    // Already handled in other functions, but this is a catch-all
    None
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
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match app.state {
                AppState::Running => match key.code {
                    KeyCode::Char('q') => app.quit = true,
                    KeyCode::Char('r') => app.reset(),
                    KeyCode::Char('s') => app.toggle_solution_words(),
                    KeyCode::Char('f') => app.state = AppState::GettingFeedback,
                    KeyCode::Char('m') => app.state = AppState::ManualKnowledgeMenu,
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
                AppState::ManualKnowledgeMenu => match key.code {
                    KeyCode::Char('1') => {
                        app.manual_input_buffer.clear();
                        app.state = AppState::ManualDirect;
                    }
                    KeyCode::Char('2') => {
                        app.manual_word.clear();
                        app.manual_feedback.clear();
                        app.manual_mode_step = 0;
                        app.state = AppState::ManualIndirect;
                    }
                    KeyCode::Esc => {
                        app.state = AppState::Running;
                    }
                    _ => {}
                },
                AppState::ManualDirect => match key.code {
                    KeyCode::Char(c) => {
                        // Accept letters, numbers, special chars for Info syntax
                        if c.is_ascii_alphanumeric() || "(),@!- ".contains(c) {
                            app.manual_input_buffer.push(c);
                        }
                    }
                    KeyCode::Backspace => {
                        app.manual_input_buffer.pop();
                    }
                    KeyCode::Enter => match parse_info_string(&app.manual_input_buffer) {
                        Ok(info_set) => {
                            let count = info_set.len();
                            app.filter.add_info_set(&info_set);
                            app.show_error(format!("Added {} pieces of knowledge", count));
                            app.manual_input_buffer.clear();
                            // Trigger recommendation recomputation
                            app.loading_idx = 0;
                            app.state = AppState::Loading;
                        }
                        Err(e) => {
                            app.show_error(e);
                        }
                    },
                    KeyCode::Esc => {
                        app.manual_input_buffer.clear();
                        app.state = AppState::Running;
                    }
                    _ => {}
                },
                AppState::ManualIndirect => match key.code {
                    KeyCode::Char(c) => {
                        if app.manual_mode_step == 0 {
                            // Step 1: collecting word
                            if c.is_ascii_lowercase() && app.manual_word.len() < 5 {
                                app.manual_word.push(c);
                            }
                        } else {
                            // Step 2: collecting feedback
                            if "GYNgyn".contains(c) && app.manual_feedback.len() < 5 {
                                app.manual_feedback.push(c.to_ascii_uppercase());
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        if app.manual_mode_step == 0 {
                            app.manual_word.pop();
                        } else {
                            app.manual_feedback.pop();
                        }
                    }
                    KeyCode::Enter => {
                        if app.manual_mode_step == 0 {
                            // Move to step 2 if word is valid
                            if app.manual_word.len() == 5 {
                                app.manual_mode_step = 1;
                            } else {
                                app.show_error("Word must be exactly 5 letters".to_string());
                            }
                        } else {
                            // Submit feedback
                            if app.manual_feedback.len() == 5 {
                                match process_feedback(&app.manual_word, &app.manual_feedback) {
                                    Ok(info_set) => {
                                        let count = info_set.len();
                                        app.filter.add_info_set(&info_set);

                                        // Add to history
                                        app.history.push((
                                            app.manual_word.clone(),
                                            app.manual_feedback.clone(),
                                            0.0, // No entropy for manual entries
                                        ));

                                        app.show_error(format!(
                                            "Added {} pieces of knowledge",
                                            count
                                        ));
                                        app.manual_word.clear();
                                        app.manual_feedback.clear();
                                        app.manual_mode_step = 0;
                                        // Trigger recommendation recomputation
                                        app.loading_idx = 0;
                                        app.state = AppState::Loading;
                                    }
                                    Err(e) => {
                                        app.show_error(e);
                                    }
                                }
                            } else {
                                app.show_error("Feedback must be exactly 5 characters".to_string());
                            }
                        }
                    }
                    KeyCode::Esc => {
                        app.manual_word.clear();
                        app.manual_feedback.clear();
                        app.manual_mode_step = 0;
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
        recommendation,
        Line::from(""),
        word_outline,
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

    for (pos, &known) in known_letters.iter().enumerate() {
        let mut position_spans = vec![Span::styled(
            format!("Pos {}: ", pos + 1),
            Style::default().fg(Color::White),
        )];

        // Check if we have a correct letter for this position
        if known != '_' {
            position_spans.push(Span::styled(
                format!("{}", known),
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
        let lines_needed = sample_count.div_ceil(words_per_line);

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
            AppState::ManualKnowledgeMenu => {
                "1=direct input | 2=word+feedback | Esc=cancel".to_string()
            }
            AppState::ManualDirect => {
                "Enter Info (e.g., Not(x), a@2, b!@3) | Enter=submit | Esc=cancel".to_string()
            }
            AppState::ManualIndirect => {
                if app.manual_mode_step == 0 {
                    "Enter word (5 letters) | Enter=continue | Esc=cancel".to_string()
                } else {
                    "G=green | Y=yellow | N=gray | Enter=submit | Esc=cancel".to_string()
                }
            }
            _ => "f=enter feedback | m=manual knowledge | r=reset | s=toggle solutions | q=quit"
                .to_string(),
        }
    };

    let help = Paragraph::new(help_text)
        .style(help_style)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[3]);

    // Render manual entry overlays
    match app.state {
        AppState::ManualKnowledgeMenu => {
            render_manual_menu(f, chunks[2]);
        }
        AppState::ManualDirect => {
            render_manual_direct(f, chunks[2], app);
        }
        AppState::ManualIndirect => {
            render_manual_indirect(f, chunks[2], app);
        }
        _ => {}
    }
}

fn render_manual_menu(f: &mut ratatui::Frame, area: Rect) {
    // Create centered popup - larger to show all content
    let popup_area = centered_rect(70, 60, area);

    let menu_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Manual Knowledge Entry",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Choose an input mode:"),
        Line::from(""),
        Line::from("  1. Direct input"),
        Line::from("     Format: Not(x), Correct(a, 2), NotAt(b, 3)"),
        Line::from("     Shorthand: -x, a@2, b!@3"),
        Line::from(""),
        Line::from("  2. Word + feedback pattern"),
        Line::from("     Enter a word, then G/Y/N feedback"),
        Line::from(""),
    ];

    let menu = Paragraph::new(menu_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });

    f.render_widget(ratatui::widgets::Clear, popup_area);
    f.render_widget(menu, popup_area);
}

fn render_manual_direct(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let popup_area = centered_rect(80, 50, area);

    let text_lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Direct Knowledge Entry",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Enter Info (comma-separated for multiple):"),
        Line::from(""),
        Line::from(Span::styled(
            format!("> {}", app.manual_input_buffer),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from("Examples:"),
        Line::from("  Not(x) or -x          - Letter x not in word"),
        Line::from("  Correct(a, 2) or a@2  - Letter a at position 2"),
        Line::from("  NotAt(b, 3) or b!@3   - Letter b in word but not at position 3"),
        Line::from(""),
        Line::from("Multiple: Not(x), a@2, b!@3"),
    ];

    let direct = Paragraph::new(text_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });

    f.render_widget(ratatui::widgets::Clear, popup_area);
    f.render_widget(direct, popup_area);
}

fn render_manual_indirect(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let popup_area = centered_rect(70, 35, area);

    let text_lines = if app.manual_mode_step == 0 {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "Word + Feedback Entry (Step 1/2)",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Enter the word that was guessed:"),
            Line::from(""),
            Line::from(Span::styled(
                format!("> {}", app.manual_word),
                Style::default().fg(Color::Cyan),
            )),
            Line::from(""),
            Line::from("(5 lowercase letters)"),
        ]
    } else {
        // Build feedback display
        let mut feedback_spans = vec![Span::raw("> ")];
        for (i, fb_char) in app.manual_feedback.chars().enumerate() {
            let color = match fb_char {
                'G' => Color::Green,
                'Y' => Color::Yellow,
                'N' => Color::Gray,
                _ => Color::White,
            };

            let word_char = app.manual_word.chars().nth(i).unwrap_or(' ');
            feedback_spans.push(Span::styled(
                format!("{} ", word_char.to_uppercase()),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ));
        }

        // Add placeholders for remaining positions
        for i in app.manual_feedback.len()..5 {
            let word_char = app.manual_word.chars().nth(i).unwrap_or('_');
            feedback_spans.push(Span::raw(format!("{} ", word_char)));
        }

        vec![
            Line::from(""),
            Line::from(Span::styled(
                "Word + Feedback Entry (Step 2/2)",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("Word: {}", app.manual_word.to_uppercase())),
            Line::from(""),
            Line::from("Enter feedback (G=Green, Y=Yellow, N=Gray):"),
            Line::from(""),
            Line::from(feedback_spans),
        ]
    };

    let indirect = Paragraph::new(text_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });

    f.render_widget(ratatui::widgets::Clear, popup_area);
    f.render_widget(indirect, popup_area);
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_not_format() {
        let result = parse_info_string("Not(x)").unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(&Info::Not('x')));

        let result = parse_info_string("not(y)").unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(&Info::Not('y')));

        let result = parse_info_string("-z").unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(&Info::Not('z')));
    }

    #[test]
    fn test_parse_correct_format() {
        let result = parse_info_string("Correct(a, 2)").unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(&Info::Correct('a', 1))); // 1-indexed to 0-indexed

        let result = parse_info_string("correct(b, 5)").unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(&Info::Correct('b', 4)));

        let result = parse_info_string("a@2").unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(&Info::Correct('a', 1)));

        let result = parse_info_string("c@1").unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(&Info::Correct('c', 0)));
    }

    #[test]
    fn test_parse_notat_format() {
        let result = parse_info_string("NotAt(b, 3)").unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(&Info::NotAt('b', 2)));

        let result = parse_info_string("notat(c, 1)").unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(&Info::NotAt('c', 0)));

        let result = parse_info_string("b!@3").unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(&Info::NotAt('b', 2)));

        let result = parse_info_string("d!@5").unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(&Info::NotAt('d', 4)));
    }

    #[test]
    fn test_parse_multiple_entries() {
        let result = parse_info_string("Not(x), Correct(a, 2), NotAt(b, 3)").unwrap();
        assert_eq!(result.len(), 3);
        assert!(result.contains(&Info::Not('x')));
        assert!(result.contains(&Info::Correct('a', 1)));
        assert!(result.contains(&Info::NotAt('b', 2)));

        let result = parse_info_string("-x, a@2, b!@3").unwrap();
        assert_eq!(result.len(), 3);
        assert!(result.contains(&Info::Not('x')));
        assert!(result.contains(&Info::Correct('a', 1)));
        assert!(result.contains(&Info::NotAt('b', 2)));
    }

    #[test]
    fn test_parse_mixed_formats() {
        let result = parse_info_string("Not(x), a@2, b!@3").unwrap();
        assert_eq!(result.len(), 3);

        let result = parse_info_string("-x, Correct(a, 2), b!@3").unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_parse_with_whitespace() {
        let result = parse_info_string("  Not(x)  ,  a@2  ,  b!@3  ").unwrap();
        assert_eq!(result.len(), 3);
        assert!(result.contains(&Info::Not('x')));
        assert!(result.contains(&Info::Correct('a', 1)));
        assert!(result.contains(&Info::NotAt('b', 2)));
    }

    #[test]
    fn test_parse_invalid_position() {
        assert!(parse_info_string("Correct(a, 0)").is_err());
        assert!(parse_info_string("Correct(a, 6)").is_err());
        assert!(parse_info_string("a@0").is_err());
        assert!(parse_info_string("a@6").is_err());
    }

    #[test]
    fn test_parse_invalid_format() {
        assert!(parse_info_string("InvalidFormat").is_err());
        assert!(parse_info_string("Not()").is_err());
        assert!(parse_info_string("Correct(a)").is_err());
        assert!(parse_info_string("NotAt(b)").is_err());
    }

    #[test]
    fn test_parse_empty_string() {
        assert!(parse_info_string("").is_err());
        assert!(parse_info_string("   ").is_err());
    }

    #[test]
    fn test_parse_deduplication() {
        // HashSet should deduplicate identical entries
        let result = parse_info_string("Not(x), Not(x), Not(x)").unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(&Info::Not('x')));
    }

    #[test]
    fn test_split_by_comma_outside_parens() {
        let result = split_by_comma_outside_parens("Not(x), Correct(a, 2), NotAt(b, 3)");
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "Not(x)");
        assert_eq!(result[1], "Correct(a, 2)");
        assert_eq!(result[2], "NotAt(b, 3)");

        let result = split_by_comma_outside_parens("a@2,b!@3,-x");
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "a@2");
        assert_eq!(result[1], "b!@3");
        assert_eq!(result[2], "-x");
    }

    #[test]
    fn test_process_feedback_basic() {
        let result = process_feedback("raise", "GYYNG").unwrap();
        assert!(result.contains(&Info::Correct('r', 0)));
        assert!(result.contains(&Info::NotAt('a', 1)));
        assert!(result.contains(&Info::NotAt('i', 2)));
        assert!(result.contains(&Info::Correct('e', 4)));
    }

    #[test]
    fn test_process_feedback_duplicate_letters() {
        // If a letter appears twice and one is N, we shouldn't add Not(letter)
        // if the other occurrence is G or Y
        let result = process_feedback("abort", "NGGNN").unwrap();
        // 'a' at position 0 is gray, but no other 'a'
        assert!(result.contains(&Info::Not('a')));
        // 'b' at position 1 is green
        assert!(result.contains(&Info::Correct('b', 1)));
        // 'o' at position 2 is green
        assert!(result.contains(&Info::Correct('o', 2)));
        // 'r' at position 3 is gray
        assert!(result.contains(&Info::Not('r')));
        // 't' at position 4 is gray
        assert!(result.contains(&Info::Not('t')));
    }
}
