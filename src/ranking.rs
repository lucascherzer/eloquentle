//! Word ranking algorithms for Wordle solver
//!
//! This module contains various algorithms to rank potential guess words.
//! Each algorithm implements the `RankingStrategy` trait, allowing them
//! to be used interchangeably.

use crate::filter::{Feedback, Pattern};
use crate::info::Info;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// A trait for word ranking strategies
pub trait RankingStrategy {
    /// Ranks all possible guesses and returns them in order from best to worst
    fn rank_words<'a>(
        &self,
        candidates: &[Rc<&'static str>],
        possible_guesses: &[&'a str],
    ) -> Vec<&'a str>;
}

/// A strategy that ranks words based on letter frequency
pub struct LetterFrequencyStrategy;

impl RankingStrategy for LetterFrequencyStrategy {
    fn rank_words<'a>(
        &self,
        candidates: &[Rc<&'static str>],
        possible_guesses: &[&'a str],
    ) -> Vec<&'a str> {
        // If there are 3 or fewer candidates, just return them
        if candidates.len() <= 3 && candidates.len() > 0 {
            return candidates.iter().map(|rc| **rc).collect();
        }

        // Count letter frequencies by position
        let mut pos_frequency: [HashMap<char, usize>; 5] = Default::default();

        // Count overall letter frequencies
        let mut letter_frequency: HashMap<char, usize> = HashMap::new();

        // Process all candidate words
        for word in candidates {
            for (pos, ch) in word.chars().enumerate() {
                *pos_frequency[pos].entry(ch).or_insert(0) += 1;
                *letter_frequency.entry(ch).or_insert(0) += 1;
            }
        }

        // Score each possible guess based on letter frequencies
        let mut scored_guesses: Vec<(&str, usize)> = possible_guesses
            .iter()
            .map(|&word| {
                // Calculate word score using positional and overall letter frequencies
                let mut score = 0;
                let mut seen = HashSet::new();

                for (pos, ch) in word.chars().enumerate() {
                    // Add positional score (more weight)
                    if let Some(freq) = pos_frequency[pos].get(&ch) {
                        score += freq * 3; // Positional match is worth more
                    }

                    // Add overall frequency score (only for unique letters to avoid double counting)
                    if seen.insert(ch) {
                        if let Some(freq) = letter_frequency.get(&ch) {
                            score += freq;
                        }
                    }
                }

                (word, score)
            })
            .collect();

        // Sort by score in descending order
        scored_guesses.sort_by(|a, b| b.1.cmp(&a.1));

        // Return just the words, without scores
        scored_guesses.into_iter().map(|(word, _)| word).collect()
    }
}

/// A strategy that uses entropy (information theory) to rank words
pub struct EntropyStrategy;

impl EntropyStrategy {
    /// Calculates the entropy score for a word
    fn calculate_entropy(&self, candidates: &[Rc<&'static str>], guess: &str) -> f64 {
        if candidates.len() <= 1 {
            return 0.0;
        }

        // Map to store frequency of each feedback pattern
        let mut pattern_frequencies: HashMap<String, usize> = HashMap::new();
        let total_candidates = candidates.len();

        // For each possible solution, calculate the pattern we'd get if we guessed 'guess'
        for candidate in candidates {
            let pattern = self.calculate_pattern(guess, candidate.as_ref());
            *pattern_frequencies
                .entry(self.pattern_to_string(&pattern))
                .or_insert(0) += 1;
        }

        // Calculate entropy using the formula: -sum(p(x) * log2(p(x)))
        let mut entropy = 0.0;
        for (_, count) in pattern_frequencies {
            let probability = count as f64 / total_candidates as f64;
            entropy -= probability * probability.log2();
        }

        entropy
    }

    /// Converts a pattern to a string for use as a hash key
    fn pattern_to_string(&self, pattern: &Pattern) -> String {
        pattern
            .iter()
            .map(|&f| match f {
                Feedback::Correct => 'G',
                Feedback::Present => 'Y',
                Feedback::Absent => 'N',
            })
            .collect()
    }

    /// Calculates the feedback pattern for a guess against a candidate
    fn calculate_pattern(&self, guess: &str, candidate: &str) -> Pattern {
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

    /// Calculates the feedback as a set of Info for a guess against a candidate
    fn calculate_info(&self, guess: &str, candidate: &str) -> HashSet<Info> {
        let pattern = self.calculate_pattern(guess, candidate);
        let guess_chars: Vec<char> = guess.chars().collect();
        let mut info_set = HashSet::new();

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
            }
        }

        info_set
    }
}

impl RankingStrategy for EntropyStrategy {
    fn rank_words<'a>(
        &self,
        candidates: &[Rc<&'static str>],
        possible_guesses: &[&'a str],
    ) -> Vec<&'a str> {
        // If there are very few candidates, just return them
        if candidates.len() <= 3 && candidates.len() > 0 {
            return candidates.iter().map(|rc| **rc).collect();
        }

        // Limit the number of candidates to evaluate for performance reasons
        let max_candidates_to_check = if candidates.len() > 100 {
            // When many candidates remain, limit evaluation
            150
        } else {
            // Otherwise check all candidates
            possible_guesses.len()
        };

        // Choose candidates to check
        let candidates_to_check = if possible_guesses.len() <= max_candidates_to_check {
            possible_guesses.to_vec()
        } else {
            possible_guesses[0..max_candidates_to_check].to_vec()
        };

        // Calculate entropy for each candidate
        let mut scored_guesses: Vec<(&str, f64)> = candidates_to_check
            .iter()
            .map(|&word| {
                let entropy = self.calculate_entropy(candidates, word);
                (word, entropy)
            })
            .collect();

        // Sort by entropy in descending order
        scored_guesses.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Return just the words, without scores
        scored_guesses.into_iter().map(|(word, _)| word).collect()
    }
}

/// A hybrid strategy that starts with letter frequency and switches to entropy
/// when the candidate list is small enough
pub struct HybridStrategy {
    frequency_strategy: LetterFrequencyStrategy,
    entropy_strategy: EntropyStrategy,
    entropy_threshold: usize,
}

impl HybridStrategy {
    /// Creates a new hybrid strategy
    pub fn new(entropy_threshold: usize) -> Self {
        Self {
            frequency_strategy: LetterFrequencyStrategy,
            entropy_strategy: EntropyStrategy,
            entropy_threshold,
        }
    }
}

impl Default for HybridStrategy {
    fn default() -> Self {
        Self::new(100) // Switch to entropy when 100 or fewer candidates remain
    }
}

impl RankingStrategy for HybridStrategy {
    fn rank_words<'a>(
        &self,
        candidates: &[Rc<&'static str>],
        possible_guesses: &[&'a str],
    ) -> Vec<&'a str> {
        if candidates.len() <= self.entropy_threshold {
            self.entropy_strategy
                .rank_words(candidates, possible_guesses)
        } else {
            self.frequency_strategy
                .rank_words(candidates, possible_guesses)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    fn create_test_candidates(words: Vec<&'static str>) -> Vec<Rc<&'static str>> {
        words.into_iter().map(|w| Rc::new(w)).collect()
    }

    #[test]
    fn test_letter_frequency_strategy() {
        let candidates =
            create_test_candidates(vec!["salet", "crate", "crane", "slant", "trace", "slate"]);

        let possible_guesses = vec!["salet", "crate", "crane", "slant", "trace", "slate"];

        let strategy = LetterFrequencyStrategy;
        let ranked_words = strategy.rank_words(&candidates, &possible_guesses);

        // Make sure we get back all words
        assert_eq!(ranked_words.len(), possible_guesses.len());

        // Words with common letters like 'a', 'e', 't' should rank highly
        assert!(ranked_words.contains(&"slate"));
        assert!(ranked_words.contains(&"salet"));
    }

    #[test]
    fn test_entropy_strategy() {
        let candidates =
            create_test_candidates(vec!["salet", "crate", "crane", "slant", "trace", "slate"]);

        let possible_guesses = vec!["salet", "crate", "crane", "slant", "trace", "slate"];

        let strategy = EntropyStrategy;
        let ranked_words = strategy.rank_words(&candidates, &possible_guesses);

        // Make sure we get back all words
        assert_eq!(ranked_words.len(), possible_guesses.len());

        // We can't easily predict which word will have highest entropy,
        // but we can check that the function runs successfully
        assert!(!ranked_words.is_empty());
    }

    #[test]
    fn test_calculate_info() {
        let strategy = EntropyStrategy;
        let info = strategy.calculate_info("trace", "crate");

        // 't' is not at position 0 (it's at position 4 instead)
        assert!(info.contains(&Info::NotAt('t', 0)));
        // 'r' is not at position 1 (it's at position 1 instead)
        assert!(info.contains(&Info::Correct('r', 1)));
        // 'a' is not at position 2 (it's at position 2 instead)
        assert!(info.contains(&Info::Correct('a', 2)));
        // 'c' is not at position 3 (it's at position 0 instead)
        assert!(info.contains(&Info::NotAt('c', 3)));
        // 'e' is not at position 4 (it's at position 4 instead)
        assert!(info.contains(&Info::Correct('e', 4)));
    }

    #[test]
    fn test_hybrid_strategy() {
        let candidates =
            create_test_candidates(vec!["salet", "crate", "crane", "slant", "trace", "slate"]);

        let possible_guesses = vec!["salet", "crate", "crane", "slant", "trace", "slate"];

        // With a threshold of 10, we should use entropy since we have 6 candidates
        let strategy = HybridStrategy::new(10);
        let ranked_words = strategy.rank_words(&candidates, &possible_guesses);

        // Make sure we get back all words
        assert_eq!(ranked_words.len(), possible_guesses.len());
        assert!(!ranked_words.is_empty());

        // With a threshold of 5, we should use frequency since we have 6 candidates
        let strategy = HybridStrategy::new(5);
        let ranked_words = strategy.rank_words(&candidates, &possible_guesses);

        // Make sure we get back all words
        assert_eq!(ranked_words.len(), possible_guesses.len());
        assert!(!ranked_words.is_empty());
    }
}
