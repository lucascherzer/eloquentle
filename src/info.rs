/// An atomic unit of information about the wordle solution.
#[derive(Hash, PartialEq, Eq, Debug, Clone)]
pub enum Info {
    /// The letter is in the correct place (Green in Wordle)
    Correct(char, usize),
    /// The letter is in the word but not at this position (Yellow in Wordle)
    NotAt(char, usize),
    /// The letter is not in the word at all (Gray in Wordle)
    Not(char),
}

// Implementation notes:
// 1. The Info enum represents atomic pieces of knowledge about the target word
// 2. Each piece can be applied independently as a filter on the word list
// 3. We can store all Info in a HashSet for deduplication and efficient operations
// 4. NotAt implies the letter exists in the word (just not at that position)

impl std::fmt::Display for Info {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Info::Correct(c, pos) => write!(f, "{}@{} (Correct)", c, pos),
            Info::NotAt(c, pos) => write!(f, "{}!@{} (Wrong position)", c, pos),
            Info::Not(c) => write!(f, "{} (Not in word)", c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_info_in_hashset() {
        let mut info_set = HashSet::new();

        // Add some information
        info_set.insert(Info::Correct('a', 0));
        info_set.insert(Info::NotAt('b', 1));
        info_set.insert(Info::Not('c'));

        // Test deduplication
        assert!(info_set.insert(Info::Not('d'))); // Should return true (new element)
        assert!(!info_set.insert(Info::Not('c'))); // Should return false (duplicate)

        // Check contents
        assert_eq!(info_set.len(), 4);
        assert!(info_set.contains(&Info::Correct('a', 0)));
        assert!(info_set.contains(&Info::NotAt('b', 1)));
        assert!(info_set.contains(&Info::Not('c')));
        assert!(info_set.contains(&Info::Not('d')));
    }
}
