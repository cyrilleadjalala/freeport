//! `--interactive`: ask before stopping a process.

use std::io::{self, BufRead, Write};

/// `y`, `yes`, and the French `o`, `oui` (any case) mean yes; anything else, no.
pub fn is_yes(answer: &str) -> bool {
    matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes" | "o" | "oui"
    )
}

/// Writes `question`, reads one line. End of input counts as "no".
pub fn confirm(question: &str, input: &mut impl BufRead, out: &mut impl Write) -> io::Result<bool> {
    write!(out, "{question}")?;
    out.flush()?;
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(false);
    }
    Ok(is_yes(&line))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_english_and_french_yes() {
        for a in ["y", "Y", "yes", "YES\n", " oui ", "o", "Oui"] {
            assert!(is_yes(a), "{a:?}");
        }
        for a in ["", "n", "no", "non", "yep", "y es", "\n"] {
            assert!(!is_yes(a), "{a:?}");
        }
    }

    #[test]
    fn confirm_reads_one_line_and_prints_the_question() {
        let mut out = Vec::new();
        assert!(confirm("Stop? ", &mut "y\nrest".as_bytes(), &mut out).unwrap());
        assert_eq!(String::from_utf8(out).unwrap(), "Stop? ");
        assert!(!confirm("Stop? ", &mut "n\n".as_bytes(), &mut Vec::new()).unwrap());
    }

    #[test]
    fn end_of_input_is_no() {
        assert!(!confirm("Stop? ", &mut "".as_bytes(), &mut Vec::new()).unwrap());
    }
}
