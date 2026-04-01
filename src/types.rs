//! Core types for the 1# language.
//!
//! The 1# language uses only two symbols: `1` and `#`.

/// A symbol in the 1# alphabet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symbol {
    One,
    Hash,
}

impl Symbol {
    /// Converts the symbol to its character representation.
    pub fn to_char(self) -> char {
        match self {
            Symbol::One => '1',
            Symbol::Hash => '#',
        }
    }
}

/// A word is a sequence of symbols representing register contents.
pub type Word = Vec<Symbol>;

/// Converts a word to its string representation.
pub fn word_to_string(word: &Word) -> String {
    word.iter().map(|s| s.to_char()).collect()
}

/// Converts a backwards binary word to decimal.
///
/// In backwards binary: LSB is first, 1=1, #=0
/// Example: #11 = 011 reversed = 110 binary = 6 decimal
pub fn word_to_decimal(word: &Word) -> Option<u128> {
    if word.is_empty() {
        return Some(0);
    }
    let mut value: u128 = 0;
    for (i, symbol) in word.iter().enumerate() {
        if i >= 128 {
            return None; // Too large
        }
        if *symbol == Symbol::One {
            value |= 1u128 << i;
        }
    }
    Some(value)
}

/// Formats a word as both backwards binary and decimal.
pub fn format_word(word: &Word) -> String {
    let bb = word_to_string(word);
    if bb.is_empty() {
        return "(empty) = 0".to_string();
    }
    match word_to_decimal(word) {
        Some(dec) => format!("{} = {}", bb, dec),
        None => format!("{} = (overflow)", bb),
    }
}

/// Converts a string to a word, filtering only valid symbols.
pub fn string_to_word(s: &str) -> Word {
    s.chars()
        .filter_map(|c| match c {
            '1' => Some(Symbol::One),
            '#' => Some(Symbol::Hash),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_to_char() {
        assert_eq!(Symbol::One.to_char(), '1');
        assert_eq!(Symbol::Hash.to_char(), '#');
    }

    #[test]
    fn test_word_to_string() {
        let word = vec![Symbol::One, Symbol::Hash, Symbol::One];
        assert_eq!(word_to_string(&word), "1#1");
    }

    #[test]
    fn test_word_to_decimal() {
        // Empty = 0
        assert_eq!(word_to_decimal(&vec![]), Some(0));
        // 1 = 1
        assert_eq!(word_to_decimal(&vec![Symbol::One]), Some(1));
        // #1 = 10 binary = 2
        assert_eq!(word_to_decimal(&vec![Symbol::Hash, Symbol::One]), Some(2));
        // 11 = 11 binary = 3
        assert_eq!(word_to_decimal(&vec![Symbol::One, Symbol::One]), Some(3));
        // #11 = 110 binary = 6
        assert_eq!(
            word_to_decimal(&vec![Symbol::Hash, Symbol::One, Symbol::One]),
            Some(6)
        );
    }

    #[test]
    fn test_string_to_word() {
        let word = string_to_word("1#1");
        assert_eq!(word.len(), 3);
        assert_eq!(word[0], Symbol::One);
        assert_eq!(word[1], Symbol::Hash);
        assert_eq!(word[2], Symbol::One);
    }
}
