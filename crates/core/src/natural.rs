//! Ordering for names that contain numbers: "Issue 2" before "Issue 10".

use std::cmp::Ordering;
use std::iter::Peekable;

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// The letters of a text without their case and without their accents:
/// those of `Élodie` and of `elodie` are the same.
pub(crate) fn folded(text: &str) -> impl Iterator<Item = char> {
    text.nfkd()
        .filter(|letter| !is_combining_mark(*letter))
        .flat_map(char::to_lowercase)
}

/// The letters of a text that has none but plain ones, without their case.
fn plain(text: &str) -> impl Iterator<Item = char> {
    text.chars().map(|letter| letter.to_ascii_lowercase())
}

/// Compares two names ignoring case and accents and reading runs of digits
/// as numbers.
///
/// Names that differ only in case, in accents or in leading zeros are still
/// told apart, so the order is total and stable.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    // This orders every list by name, and nearly every name is plain
    // letters, which have no accents to look for.
    let by_letters = if a.is_ascii() && b.is_ascii() {
        compare(plain(a), plain(b))
    } else {
        // Without its accent, `Élan` sits with the names in E and not after
        // those in Z.
        compare(folded(a), folded(b))
    };
    by_letters.then_with(|| a.cmp(b))
}

/// Compares two runs of folded letters, reading runs of digits as numbers.
fn compare(left: impl Iterator<Item = char>, right: impl Iterator<Item = char>) -> Ordering {
    let (mut left, mut right) = (left.peekable(), right.peekable());
    loop {
        let ordering = match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                compare_numbers(&take_digits(&mut left), &take_digits(&mut right))
            }
            (Some(x), Some(y)) => {
                left.next();
                right.next();
                x.cmp(&y)
            }
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
}

fn take_digits(chars: &mut Peekable<impl Iterator<Item = char>>) -> String {
    let mut digits = String::new();
    while let Some(digit) = chars.next_if(char::is_ascii_digit) {
        digits.push(digit);
    }
    digits
}

/// Compares digit strings of any length by value.
fn compare_numbers(a: &str, b: &str) -> Ordering {
    let a = a.trim_start_matches('0');
    let b = b.trim_start_matches('0');
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(mut names: Vec<&str>) -> Vec<&str> {
        names.sort_by(|a, b| natural_cmp(a, b));
        names
    }

    #[test]
    fn numbers_sort_by_value() {
        assert_eq!(
            sorted(vec![
                "Issue 10",
                "Issue 2",
                "Issue 1",
                "Issue 002b",
                "issue 3"
            ]),
            ["Issue 1", "Issue 2", "Issue 002b", "issue 3", "Issue 10"]
        );
        assert_eq!(
            sorted(vec!["p10.jpg", "p9.jpg", "p1.jpg"]),
            ["p1.jpg", "p9.jpg", "p10.jpg"]
        );
    }

    #[test]
    fn case_is_ignored_but_the_order_is_total() {
        assert_eq!(
            sorted(vec!["batman", "Aquaman", "Batman"]),
            ["Aquaman", "Batman", "batman"]
        );
        assert_eq!(natural_cmp("a", "a"), Ordering::Equal);
        assert_ne!(natural_cmp("a01", "a1"), Ordering::Equal);
        assert_eq!(natural_cmp("a", "ab"), Ordering::Less);
    }

    #[test]
    fn an_accent_does_not_move_a_name_from_its_letter() {
        assert_eq!(
            sorted(vec!["Zot", "Élan Road", "Eclipse", "Elan Road", "Fables"]),
            ["Eclipse", "Elan Road", "Élan Road", "Fables", "Zot"]
        );
        assert_ne!(natural_cmp("Élan", "Elan"), Ordering::Equal);
    }

    #[test]
    fn very_long_numbers_do_not_overflow() {
        let small = "9".repeat(30);
        let large = format!("1{}", "0".repeat(30));
        assert_eq!(natural_cmp(&small, &large), Ordering::Less);
    }
}
