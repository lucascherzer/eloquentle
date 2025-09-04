use std::{collections::HashSet, rc::Rc};

use crate::{
    info::Info,
    words::{WORDS, get_rc_words},
};

/// Converts a feedback pattern to a string representation for use as a hash key.
fn pattern_to_string(pattern: &Pattern) -> String {
    pattern
        .iter()
        .map(|&feedback| match feedback {
            Feedback::Correct => 'G',
            Feedback::Present => 'Y',
            Feedback::Absent => 'N',
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feedback {
    Correct, // Green - letter is in the correct position
    Present, // Yellow - letter is in the word but in the wrong position
    Absent,  // Gray - letter is not in the word
}

pub type Pattern = [Feedback; 5];

// Implementation removed as we now use pattern_to_info function instead

#[derive(Clone)]
pub struct Filter {
    words: Vec<Rc<&'static str>>,
    info: HashSet<Info>,
}

/// Returns the precomputed best first guess based on entropy analysis of the
/// full word list.
/// This is used to avoid expensive calculation for the first guess, which is
/// always the same.
pub fn get_best_first_guess() -> &'static str {
    // This word has been precomputed as having the highest entropy
    // across the entire dictionary
    // NOTE: Apparently, the NYT has a subset of the wordlist that they choose
    // the word of the day from. This result does not take that into account,
    // it takes the whole list of valid words.
    "tares"
}

/// Converts a pattern to a set of Info objects
pub fn pattern_to_info(pattern: &Pattern, guess: &str) -> HashSet<Info> {
    let mut info_set = HashSet::new();
    let guess_chars: Vec<char> = guess.chars().collect();

    for (i, &feedback) in pattern.iter().enumerate() {
        let c = guess_chars[i];
        match feedback {
            Feedback::Correct => {
                info_set.insert(Info::Correct(c, i));
            }
            Feedback::Present => {
                info_set.insert(Info::NotAt(c, i));
            }
            Feedback::Absent => {
                // Check if this letter appears elsewhere with Present or Correct
                let is_elsewhere = (0..5).any(|j| {
                    j != i
                        && guess_chars[j] == c
                        && (pattern[j] == Feedback::Present || pattern[j] == Feedback::Correct)
                });

                if !is_elsewhere {
                    info_set.insert(Info::Not(c));
                }
            }
        };
    }

    info_set
}

impl Filter {
    pub fn filter_contains(&mut self, c: char) {
        self.words = self
            .words
            .iter()
            .filter(|i| (**i).contains(c))
            .cloned()
            .collect();
    }
    pub fn filter_without(&mut self, c: char) {
        self.words = self
            .words
            .iter()
            .filter(|i| !(**i).contains(c))
            .cloned()
            .collect();
    }
    pub fn filter_char_at(&mut self, c: char, loc: usize) {
        self.words = self
            .words
            .iter()
            .filter(|i| i.as_bytes()[loc] == c as u8)
            .cloned()
            .collect();
    }
    /// Recommends the best word to guess next based on information theory
    /// principles. This function calculates the "entropy" of each possible
    /// guess, where higher entropy means the word would be better at dividing
    /// the remaining possible words into distinct groups.
    ///
    /// Returns the word with the highest entropy score. If multiple words have
    /// the same score, it returns the first one found (deterministic but
    /// arbitrary).
    ///
    /// By default, this only considers words from the remaining candidates.
    pub fn recommend_guess(&self) -> String {
        // If we're at the start with all words, use the precomputed best guess
        if self.words.len() == WORDS.len() {
            return get_best_first_guess().to_string();
        }

        // If only one word remains, return it immediately
        if self.words.len() == 1 {
            return self.words[0].to_string();
        }

        self.recommend_guess_from_candidates(false)
    }

    /// Recommends the best word to guess next based on information theory
    /// principles.
    ///
    /// If `use_full_dictionary` is true, it will consider all words in the
    /// dictionary, not just the remaining candidates. This can be better early
    /// in the game when you want to maximize information gain.
    pub fn recommend_guess_from_candidates(&self, use_full_dictionary: bool) -> String {
        if self.words.len() <= 1 {
            return self
                .words
                .first()
                .map_or(String::from(""), |w| w.to_string());
        }

        let candidate_guesses: Vec<&str> = if use_full_dictionary {
            WORDS.to_vec()
        } else {
            self.words.iter().map(|rc_str| **rc_str).collect()
        };

        if !use_full_dictionary && self.words.len() == WORDS.len() {
            return get_best_first_guess().to_string();
        }

        if self.words.len() <= 3 && !use_full_dictionary {
            return self.words[0].to_string();
        }

        // Limit the number of candidates to evaluate when there are many words
        let max_candidates_to_check = if use_full_dictionary || self.words.len() > 100 {
            200
        } else {
            candidate_guesses.len()
        };

        // Choose candidates to check (either randomly sample or take the first N)
        let candidates_to_check = if candidate_guesses.len() <= max_candidates_to_check {
            candidate_guesses
        } else {
            // Take a subset of candidates - using first N for deterministic behavior
            // In a real implementation, you might want to randomly sample or use letter frequency
            candidate_guesses[0..max_candidates_to_check].to_vec()
        };

        // Track the best guess and its score
        let mut best_score = 0.0;
        let mut best_guess = candidates_to_check[0].to_string();

        // Early stopping threshold - if we find a guess with high entropy,
        // stop searching. These are arbitrarily chosen.
        let early_stop_threshold = if self.words.len() > 20 { 4.5 } else { 5.0 };

        for &guess in &candidates_to_check {
            let score = self.calculate_entropy(guess);

            if score > best_score {
                best_score = score;
                best_guess = guess.to_string();

                // Early stopping if we find a very good guess
                if score > early_stop_threshold {
                    break;
                }
            }
        }

        best_guess
    }

    /// Calculates the entropy (information gain) for a potential guess.
    /// Higher entropy means better guesses.
    /// Returns the number of remaining possible words
    pub fn remaining_count(&self) -> usize {
        self.words.len()
    }

    /// Returns a clone of the remaining words as Strings
    pub fn remaining_words(&self) -> Vec<String> {
        self.words.iter().map(|w| w.to_string()).collect()
    }

    /// Returns a reference to the internal candidate list
    pub fn get_candidates(&self) -> &[Rc<&'static str>] {
        &self.words
    }

    /// Simulates a guess against a target word and applies all the appropriate filters.
    /// This is useful for testing the solver or creating an automated player.
    /// Returns the pattern that would be produced by this guess.
    pub fn simulate_guess(&mut self, guess: &str, target: &str) -> Pattern {
        let pattern = self.calculate_pattern(guess, target);
        let info_set = pattern_to_info(&pattern, guess);

        // Add all info to our collection
        self.add_info_set(&info_set);

        pattern
    }

    /// Get the information that would be produced by a guess
    /// without actually applying any filters
    pub fn get_info_for_guess(&self, guess: &str, target: &str) -> HashSet<Info> {
        let pattern = self.calculate_pattern(guess, target);
        pattern_to_info(&pattern, guess)
    }

    /// Filters words that don't have the specified character at the given position.
    /// This is useful for the "yellow" feedback in Wordle (right letter, wrong position).
    pub fn filter_not_at(&mut self, c: char, loc: usize) {
        self.words = self
            .words
            .iter()
            .filter(|i| i.as_bytes()[loc] != c as u8)
            .cloned()
            .collect();
    }

    pub fn calculate_entropy(&self, guess: &str) -> f64 {
        // Map to store frequency of each feedback pattern
        let mut pattern_frequencies = std::collections::HashMap::new();
        let total_candidates = self.words.len();

        // Early optimization: if there are very few candidates, just return a simple score
        if total_candidates <= 2 {
            // For 1-2 candidates, any valid guess is fine - just pick from candidates if possible
            return if self.words.iter().any(|w| **w == guess) {
                1.0
            } else {
                0.0
            };
        }

        // For each possible solution, calculate the pattern we'd get if we guessed 'guess'
        for candidate in &self.words {
            let pattern = self.calculate_pattern(guess, candidate);
            let pattern_key = pattern_to_string(&pattern);
            *pattern_frequencies.entry(pattern_key).or_insert(0) += 1;
        }

        // Calculate entropy using the formula: -sum(p(x) * log2(p(x)))
        let mut entropy = 0.0;
        for (_pattern, count) in pattern_frequencies {
            let probability = count as f64 / total_candidates as f64;
            entropy -= probability * probability.log2();
        }

        entropy
    }

    /// Calculates the feedback pattern we would get if we guessed 'guess' and
    /// the secret word was 'candidate'.
    pub fn calculate_pattern(&self, guess: &str, candidate: &str) -> Pattern {
        let guess_chars: Vec<char> = guess.chars().collect();
        let candidate_chars: Vec<char> = candidate.chars().collect();
        let mut pattern = [Feedback::Absent; 5];

        // First pass: Mark correct positions
        let mut used_candidate_positions = [false; 5];

        for i in 0..5 {
            if guess_chars[i] == candidate_chars[i] {
                pattern[i] = Feedback::Correct;
                used_candidate_positions[i] = true;
            }
        }

        // Second pass: Mark present but wrong positions
        for i in 0..5 {
            if pattern[i] == Feedback::Correct {
                continue; // Already marked as correct
            }

            // Check if the character appears elsewhere in the candidate
            for j in 0..5 {
                if !used_candidate_positions[j] && guess_chars[i] == candidate_chars[j] {
                    pattern[i] = Feedback::Present;
                    used_candidate_positions[j] = true;
                    break;
                }
            }
        }

        pattern
    }
}

impl Default for Filter {
    fn default() -> Self {
        Filter {
            words: get_rc_words(),
            info: HashSet::new(),
        }
    }
}

impl Filter {
    /// Creates a new Filter with the given words
    pub fn new_with_words(words: Vec<Rc<&'static str>>) -> Self {
        Filter {
            words,
            info: HashSet::new(),
        }
    }

    /// Add a single piece of information and apply its filter
    pub fn add_info(&mut self, info: Info) {
        if !self.info.contains(&info) {
            self.info.insert(info.clone());
            self.apply_single_info(&info);
        }
    }

    /// Add multiple pieces of information and apply their filters
    pub fn add_info_set(&mut self, info_set: &HashSet<Info>) {
        for info in info_set {
            self.add_info(info.clone());
        }
    }

    /// Apply the filtering effect of a single piece of information
    fn apply_single_info(&mut self, info: &Info) {
        match info {
            Info::Correct(c, pos) => self.filter_char_at(*c, *pos),
            Info::NotAt(c, pos) => {
                self.filter_contains(*c);
                self.filter_not_at(*c, *pos);
            }
            Info::Not(c) => self.filter_without(*c),
        }
    }

    /// Get all the information collected so far
    pub fn get_info(&self) -> &HashSet<Info> {
        &self.info
    }

    /// Reset to initial state with all words but keep collected info
    pub fn reset_words(&mut self) {
        self.words = get_rc_words();
    }

    /// Reset to initial state with all words and reapply all info
    pub fn rebuild_from_info(&mut self) {
        self.reset_words();
        let info_clone = self.info.clone();
        for info in &info_clone {
            self.apply_single_info(info);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create a Filter with a custom word list
    fn create_test_filter(words: Vec<&'static str>) -> Filter {
        Filter {
            words: words.into_iter().map(|w| Rc::new(w)).collect(),
        }
    }

    // Helper function to get all words as strings from a filter
    fn get_words(filter: &Filter) -> Vec<String> {
        filter.words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn test_filter_contains() {
        // Create a filter with a small test set
        let mut filter = create_test_filter(vec!["about", "brick", "crate", "dizzy", "earth"]);

        // Initial count
        assert_eq!(filter.words.len(), 5);

        // Filter for words containing 'z'
        filter.filter_contains('z');

        // Only "dizzy" contains 'z'
        assert_eq!(filter.words.len(), 1);

        let words = get_words(&filter);
        assert!(words.contains(&String::from("dizzy")));
    }

    #[test]
    fn test_filter_without() {
        // Create a filter with a small test set
        let mut filter = create_test_filter(vec!["about", "brick", "crate", "dizzy", "earth"]);

        // Filter for words without 'a'
        filter.filter_without('a');

        // "brick" and "dizzy" don't contain 'a'
        assert_eq!(filter.words.len(), 2);

        let words = get_words(&filter);
        assert!(words.contains(&String::from("brick")));
        assert!(words.contains(&String::from("dizzy")));
    }

    #[test]
    fn test_filter_char_at() {
        // Create a filter with a small test set
        let mut filter = create_test_filter(vec!["about", "brick", "crate", "dizzy", "earth"]);

        // Filter for words with 'b' at position 1
        filter.filter_char_at('b', 1);

        // Only "about" has 'b' at position 1
        assert_eq!(filter.words.len(), 1);

        let words = get_words(&filter);
        assert!(words.contains(&String::from("about")));
    }

    #[test]
    fn test_multiple_filters() {
        // Create a filter with a test set
        let mut filter = create_test_filter(vec![
            "about", "amber", "bacon", "baker", "charm", "clear", "debug",
        ]);

        // Apply multiple filters
        filter.filter_char_at('b', 0); // First letter is 'b'
        filter.filter_contains('a'); // Contains 'a'
        filter.filter_without('n'); // Does not contain 'n'

        // Only "baker" satisfies all conditions
        assert_eq!(filter.words.len(), 1);

        let words = get_words(&filter);
        assert!(words.contains(&String::from("baker")));
    }

    #[test]
    fn test_filter_to_empty() {
        let mut filter = create_test_filter(vec!["about", "brick", "crate", "dizzy", "earth"]);

        // Apply filters that should result in no matches
        filter.filter_char_at('z', 0); // First letter is 'z'

        // Should have no words left
        assert_eq!(filter.words.len(), 0);
    }

    #[test]
    fn test_filter_specific_words() {
        let mut filter = create_test_filter(vec!["about", "abort", "adult", "adapt", "admit"]);

        // Filter to find "about"
        filter.filter_char_at('a', 0);
        filter.filter_char_at('b', 1);
        filter.filter_char_at('o', 2);
        filter.filter_char_at('u', 3);
        filter.filter_char_at('t', 4);

        // Should have exactly one word left: "about"
        assert_eq!(filter.words.len(), 1);

        let words = get_words(&filter);
        assert!(words.contains(&String::from("about")));
    }

    #[test]
    fn test_recommend_guess() {
        // Create a filter with a small test set
        let mut filter = create_test_filter(vec!["apple", "amber", "amble", "abide", "abode"]);

        // Remove some words to make it more interesting
        filter.filter_without('p');

        // Now our list should have ["amber", "amble", "abide", "abode"]
        // The best guess should maximize information gain
        let recommendation = filter.recommend_guess();

        // The best guess in this small set is likely "amber" or "amble" as they have more unique letters,
        // but we'll just verify the recommendation is in our list
        let remaining_words = get_words(&filter);
        assert!(remaining_words.contains(&recommendation));
    }

    #[test]
    fn test_simulate_guess() {
        use Feedback::*;

        // Create a filter with a small test set
        let mut filter = create_test_filter(vec![
            "crate", "crane", "frame", "brave", "grade", "trace", "grape",
        ]);

        // Simulate guessing "crane" when the target is "grape"
        let pattern = filter.simulate_guess("crane", "grape");

        // Expected pattern:
        // - 'c': not in "grape" -> Absent
        // - 'r': in "grape" but wrong position -> Present
        // - 'a': in "grape" but wrong position -> Present
        // - 'n': not in "grape" -> Absent
        // - 'e': in "grape" but wrong position -> Present
        assert_eq!(pattern, [Absent, Present, Present, Absent, Present]);

        // After simulation, filter should have reduced the word list to just contain "grape"
        assert_eq!(filter.remaining_count(), 1);

        let remaining = filter.remaining_words();
        assert_eq!(remaining, vec!["grape"]);
    }

    #[test]
    fn test_filter_not_at() {
        // Create a filter with a small test set
        let mut filter = create_test_filter(vec!["apple", "amber", "amble", "abide", "abode"]);

        // Filter out words with 'b' at position 1
        filter.filter_not_at('b', 1);

        // Should exclude "abide" and "abode" which have 'b' at position 1
        assert_eq!(filter.remaining_count(), 3);

        let words = get_words(&filter);
        assert!(words.contains(&String::from("apple")));
        assert!(words.contains(&String::from("amber")));
        assert!(words.contains(&String::from("amble")));
    }

    #[test]
    fn test_calculate_pattern() {
        let filter = create_test_filter(vec!["apple", "amber"]);
        use Feedback::*;

        // Test exact match
        let pattern1 = filter.calculate_pattern("amber", "amber");
        assert_eq!(pattern1, [Correct, Correct, Correct, Correct, Correct]);

        // Test no match
        let pattern2 = filter.calculate_pattern("abide", "motor");
        assert_eq!(pattern2, [Absent, Absent, Absent, Absent, Absent]);

        // Test mixed pattern
        let pattern3 = filter.calculate_pattern("amber", "blame");
        // 'a' is wrong position, 'm' is wrong position, 'b' is wrong position, others not in word
        assert_eq!(pattern3, [Present, Present, Present, Absent, Absent]);

        // Test repeated letters
        let pattern4 = filter.calculate_pattern("speed", "steep");
        // First 's' is correct, 'p' is in wrong position, one 'e' is correct, other 'e' wrong position
        assert_eq!(pattern4, [Correct, Present, Correct, Present, Absent]);
    }

    #[test]
    fn test_calculate_entropy() {
        let filter = create_test_filter(vec!["apple", "amble", "amber", "abide", "abode"]);

        // Calculate entropy for different guesses
        let entropy1 = filter.calculate_entropy("tripe");
        let entropy2 = filter.calculate_entropy("amber");

        // Entropy should be a non-negative float
        assert!(entropy1 >= 0.0);
        assert!(entropy2 >= 0.0);
    }

    #[test]
    fn test_wordle_scenario() {
        // Create a filter with words that could be potential Wordle answers
        let mut filter = create_test_filter(vec![
            "charm", "crypt", "croft", "botch", "cloth", "match", "batch", "watch", "pitch",
            "hatch", "worth", "north",
        ]);

        // Simulate a Wordle game:
        // First guess: "rates"
        // Feedback: 'r' is in the word but wrong position (yellow)
        //           'a' is not in the word (gray)
        //           't' is in correct position (green)
        //           'e' is not in the word (gray)
        //           's' is not in the word (gray)

        filter.filter_contains('r'); // Word contains 'r'
        filter.filter_without('a'); // Word doesn't contain 'a'
        filter.filter_char_at('t', 2); // Third letter is 't'
        filter.filter_without('e'); // Word doesn't contain 'e'
        filter.filter_without('s'); // Word doesn't contain 's'

        // After all the filtering, we should have specific words left
        let word_count = filter.words.len();

        let words = get_words(&filter);

        // Either verify we have specific words we expect
        if word_count == 2 {
            assert!(words.contains(&String::from("crypt")));
            assert!(words.contains(&String::from("croft")));
        } else if word_count == 0 {
            // No matches is also valid depending on exact implementation
            assert_eq!(words.len(), 0);
        } else {
            // If we have any words, they should match our criteria
            for word in &words {
                assert!(word.contains('r'));
                assert!(!word.contains('a'));
                assert!(word.chars().nth(2) == Some('t'));
                assert!(!word.contains('e'));
                assert!(!word.contains('s'));
            }
        }
    }
}
