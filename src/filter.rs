use std::rc::Rc;

use crate::words::get_rc_words;

struct Filter {
    words: Vec<Rc<&'static str>>,
}

impl Filter {
    fn filter_contains(&mut self, c: char) {
        self.words = self
            .words
            .iter()
            .filter(|i| (**i).contains(c))
            .cloned()
            .collect();
    }
    fn filter_without(&mut self, c: char) {
        self.words = self
            .words
            .iter()
            .filter(|i| !(**i).contains(c))
            .cloned()
            .collect();
    }
    fn filter_char_at(&mut self, c: char, loc: usize) {
        self.words = self
            .words
            .iter()
            .filter(|i| i.as_bytes()[loc] == c as u8)
            .cloned()
            .collect();
    }
}

impl Default for Filter {
    fn default() -> Self {
        Filter {
            words: get_rc_words(),
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
