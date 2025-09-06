//! Game logic for Wordle
//!
//! This module contains the implementation of Wordle game mechanics,
//! separate from the solving algorithms.

use crate::info::Info;
use std::collections::HashSet;

/// Represents a Wordle game instance
pub struct WordleGame {
    target: String,
}

impl WordleGame {
    /// Create a new Wordle game with the given target word
    pub fn new(target: String) -> Self {
        // Validate that the target is a 5-letter word
        assert_eq!(target.len(), 5, "Target word must be exactly 5 letters");
        Self { target }
    }

    /// Process a guess and return feedback as a set of Info
    pub fn get_feedback(&self, guess: &str) -> HashSet<Info> {
        // Validate that the guess is a 5-letter word
        assert_eq!(guess.len(), 5, "Guess must be exactly 5 letters");

        let mut info_set = HashSet::new();
        let target_chars: Vec<char> = self.target.chars().collect();
        let guess_chars: Vec<char> = guess.chars().collect();

        // Track which positions in the target have been matched
        let mut used_target_positions = [false; 5];

        // First pass: Mark correct positions
        for (i, &guess_char) in guess_chars.iter().enumerate() {
            if guess_char == target_chars[i] {
                info_set.insert(Info::Correct(guess_char, i));
                used_target_positions[i] = true;
            }
        }

        // Second pass: Mark wrong positions
        for (i, &guess_char) in guess_chars.iter().enumerate() {
            // Skip if this position was already marked as correct
            if guess_chars[i] == target_chars[i] {
                continue;
            }

            // Check if the letter appears elsewhere in the target
            let mut found = false;
            for (j, &target_char) in target_chars.iter().enumerate() {
                if !used_target_positions[j] && guess_char == target_char {
                    info_set.insert(Info::NotAt(guess_char, i));
                    used_target_positions[j] = true;
                    found = true;
                    break;
                }
            }

            // If the letter doesn't appear (unused) anywhere in the target, mark as not in word
            if !found {
                // Check if this letter already appears in the word and has been marked
                let already_accounted_for = (0..5).any(|j| {
                    j != i
                        && guess_chars[j] == guess_char
                        && (info_set.contains(&Info::Correct(guess_char, j))
                            || info_set.contains(&Info::NotAt(guess_char, j)))
                });

                if !already_accounted_for {
                    info_set.insert(Info::Not(guess_char));
                }
            }
        }

        info_set
    }

    /// Check if a guess is correct
    pub fn is_correct_guess(&self, guess: &str) -> bool {
        guess == self.target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let game = WordleGame::new("hello".to_string());
        let feedback = game.get_feedback("hello");

        assert_eq!(feedback.len(), 5);
        for i in 0..5 {
            assert!(feedback.contains(&Info::Correct("hello".chars().nth(i).unwrap(), i)));
        }
    }

    #[test]
    fn test_no_match() {
        let game = WordleGame::new("hello".to_string());
        let feedback = game.get_feedback("bumpy");

        assert!(feedback.contains(&Info::Not('b')));
        assert!(feedback.contains(&Info::Not('u')));
        assert!(feedback.contains(&Info::Not('m')));
        assert!(feedback.contains(&Info::Not('p')));
        assert!(feedback.contains(&Info::Not('y')));
    }

    #[test]
    fn test_partial_match() {
        let game = WordleGame::new("hello".to_string());
        let feedback = game.get_feedback("below");

        // 'b' is not in the word
        assert!(feedback.contains(&Info::Not('b')));
        // 'e' is correct at position 1
        assert!(feedback.contains(&Info::Correct('e', 1)));
        // 'l' is correct at position 2
        assert!(feedback.contains(&Info::Correct('l', 2)));
        // 'o' is correct at position 3
        assert!(feedback.contains(&Info::NotAt('o', 3)));
        // 'w' is not in the word
        assert!(feedback.contains(&Info::Not('w')));
    }

    #[test]
    fn test_repeated_letters() {
        let game = WordleGame::new("hello".to_string());
        let feedback = game.get_feedback("label");

        assert!(feedback.contains(&Info::NotAt('l', 0)));
        assert!(feedback.contains(&Info::Not('a')));
        assert!(feedback.contains(&Info::Not('b')));
        assert!(feedback.contains(&Info::NotAt('e', 3)));
        assert!(feedback.contains(&Info::NotAt('l', 4)));
    }

    #[test]
    fn test_is_correct_guess() {
        let game = WordleGame::new("hello".to_string());
        assert!(game.is_correct_guess("hello"));
        assert!(!game.is_correct_guess("world"));
    }
}
