//! Port arguments: a single port (`3000`) or an inclusive range (`3000-3010`).

use crate::i18n::Lang;
use std::collections::BTreeMap;

/// Largest range accepted, to avoid scanning (and stopping) by accident.
pub const MAX_RANGE: usize = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortSpec {
    Single(u16),
    Range(u16, u16),
}

fn port(s: &str) -> Option<u16> {
    s.trim().parse::<u16>().ok().filter(|p| *p != 0)
}

pub fn parse(s: &str) -> Option<PortSpec> {
    match s.split_once('-') {
        Some((a, b)) => {
            let (a, b) = (port(a)?, port(b)?);
            (a <= b && usize::from(b - a) < MAX_RANGE).then_some(PortSpec::Range(a, b))
        }
        None => port(s).map(PortSpec::Single),
    }
}

/// clap value parser.
pub fn parse_spec(s: &str) -> Result<PortSpec, String> {
    parse(s).ok_or_else(|| {
        Lang::detect().pick(
            format!(
                "'{s}' is not a valid port or range (1-65535, e.g. 3000 or 3000-3010, at most {MAX_RANGE} ports)"
            ),
            format!(
                "'{s}' n'est pas un port ou une plage valide (1-65535, ex. 3000 ou 3000-3010, {MAX_RANGE} ports au plus)"
            ),
        )
    })
}

/// Sorted, deduplicated ports; `true` when the port was given explicitly (not
/// only through a range).
pub fn expand(specs: &[PortSpec]) -> Vec<(u16, bool)> {
    let mut m: BTreeMap<u16, bool> = BTreeMap::new();
    for s in specs {
        match *s {
            PortSpec::Single(p) => {
                m.insert(p, true);
            }
            PortSpec::Range(a, b) => {
                for p in a..=b {
                    m.entry(p).or_insert(false);
                }
            }
        }
    }
    m.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_and_range() {
        assert_eq!(parse("3000"), Some(PortSpec::Single(3000)));
        assert_eq!(parse("3000-3010"), Some(PortSpec::Range(3000, 3010)));
        assert_eq!(parse("65535"), Some(PortSpec::Single(65535)));
        assert_eq!(parse("8080-8080"), Some(PortSpec::Range(8080, 8080)));
    }

    #[test]
    fn rejects_invalid() {
        for bad in [
            "", "0", "65536", "-1", "abc", "10-5", "0-10", "1-65535", "3000-", "-3000", "1-2-3",
        ] {
            assert_eq!(parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn range_size_is_capped() {
        assert!(parse("1-1000").is_some());
        assert!(parse("1-1001").is_none());
    }

    #[test]
    fn expand_sorts_dedups_and_marks_explicit() {
        let specs = [
            PortSpec::Range(3002, 3004),
            PortSpec::Single(3003),
            PortSpec::Single(80),
            PortSpec::Range(3003, 3005),
        ];
        assert_eq!(
            expand(&specs),
            vec![
                (80, true),
                (3002, false),
                (3003, true),
                (3004, false),
                (3005, false)
            ]
        );
    }
}
