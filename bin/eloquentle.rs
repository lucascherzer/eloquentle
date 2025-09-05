use eloquentle::{
    filter::{Filter, get_best_first_guess},
    game::WordleGame,
    info::Info,
};
use std::collections::HashSet;
use std::io::{self, BufRead, Write};

/// A simple TUI for the Wordle solver
fn main() {
    // Create a new filter with the full word list
    let mut filter = Filter::default();

    println!("===========================");
    println!("= ELOQUENTLE WORDLE SOLVER =");
    println!("===========================");
    println!("Welcome to the Eloquentle Wordle solver!");
    println!("This tool will help you solve Wordle puzzles.\n");

    // Suggest the best first guess
    let first_guess = get_best_first_guess();
    println!("Best first guess: {}", first_guess);
    println!("Try this word in Wordle and then provide feedback.\n");

    // Main interaction loop
    let stdin = io::stdin();
    let mut current_guess = first_guess.to_string();
    let mut turn = 1;

    loop {
        if turn > 1 {
            // After the first turn, recommend the next best guess
            if filter.remaining_count() <= 1 {
                if filter.remaining_count() == 1 {
                    println!("\nSolved! The word is: {}", filter.remaining_words()[0]);
                } else {
                    println!(
                        "\nNo words match the given constraints. Did you make an error in your feedback?"
                    );
                }
                break;
            }

            // Get the next best guess
            current_guess = filter.recommend_guess();
            println!("\nTurn {}:", turn);
            println!("Recommended guess: {}", current_guess);
            println!("({} possible words remain)", filter.remaining_count());

            // Show a sample of possible words
            let remaining_words = filter.remaining_words();
            let display_count = remaining_words.len().min(5);
            println!(
                "Sample possible words: {:?}",
                &remaining_words[..display_count]
            );
        }

        println!(
            "\nEnter feedback for '{}' (or 'quit' to exit, 'reset' to start over):",
            current_guess
        );
        println!("Format: For each letter, enter:");
        println!("  G = Green (correct letter in correct position)");
        println!("  Y = Yellow (correct letter in wrong position)");
        println!("  N = Gray (letter not in the word)");
        println!("Example: GYNNY");

        print!("> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        stdin.lock().read_line(&mut input).unwrap();
        let input = input.trim();

        match input {
            "quit" | "exit" => {
                println!("Goodbye!");
                break;
            }
            "reset" => {
                println!("Resetting the solver...");
                filter = Filter::default();
                turn = 1;
                current_guess = get_best_first_guess().to_string();
                println!("Best first guess: {}", current_guess);
                continue;
            }
            feedback if feedback.len() == 5 => {
                let info_set = process_feedback(&current_guess, feedback);
                if let Some(info) = info_set {
                    filter.add_info_set(&info);
                    turn += 1;
                } else {
                    println!("Invalid feedback format. Please try again.");
                }
            }
            _ => {
                println!("Invalid input. Feedback must be exactly 5 characters (G, Y, or N).");
            }
        }
    }
}

/// Process user feedback into a set of Info objects
fn process_feedback(guess: &str, feedback: &str) -> Option<HashSet<Info>> {
    if feedback.len() != 5 || guess.len() != 5 {
        return None;
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
            _ => return None, // Invalid feedback character
        }
    }

    Some(info_set)
}
