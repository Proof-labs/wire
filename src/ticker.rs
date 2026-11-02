//! The market ticker rule, defined once for the engine and every client.
//!
//! `CreateMarket` refuses a ticker this rule refuses, and the gateway, the SDK
//! and the Web Admin call the same function before signing or forwarding, so
//! a typo fails where it is typed. A market cannot be renamed: DevNet's market
//! 6 was created as `TEST3.` before the rule existed (Proof-labs/exchange#1351).
//!
//! Tickers the engine derives itself (`<underlying>-<id>-CPY`, `EVT-<id>-EBY`)
//! are bounded by length only: an underlying created before the rule may not
//! follow it.

use core::fmt;

use crate::types::MAX_TICKER_LEN;

/// Why [`validate_ticker`] refused a ticker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickerRefusal {
    /// The ticker is empty.
    Empty,
    /// The ticker is longer than [`MAX_TICKER_LEN`] bytes.
    TooLong { len: usize },
    /// The first character is not a capital letter `A`–`Z`.
    BadStart { found: char },
    /// A character other than `A`–`Z`, `0`–`9` or `-`, at this character index.
    BadChar { index: usize, found: char },
}

impl fmt::Display for TickerRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("ticker is empty"),
            Self::TooLong { len } => {
                write!(
                    f,
                    "ticker is {len} bytes, over the {MAX_TICKER_LEN}-byte limit"
                )
            }
            Self::BadStart { found } => {
                write!(
                    f,
                    "ticker must start with a capital letter A-Z, not {found:?}"
                )
            }
            Self::BadChar { index, found } => write!(
                f,
                "ticker may hold only A-Z, 0-9 and '-', found {found:?} at character {index}"
            ),
        }
    }
}

/// A market ticker: 1 to [`MAX_TICKER_LEN`] bytes of capital letters `A`–`Z`,
/// digits `0`–`9` and hyphens, starting with a capital letter. `BTC`, `TEST2`
/// and `BTC-1-CPY` pass; `TEST3.`, `test3`, `TEST 3`, `1BTC` and `` do not.
pub fn validate_ticker(ticker: &str) -> Result<(), TickerRefusal> {
    if ticker.len() > MAX_TICKER_LEN {
        return Err(TickerRefusal::TooLong { len: ticker.len() });
    }
    let mut chars = ticker.chars().enumerate();
    match chars.next() {
        None => return Err(TickerRefusal::Empty),
        Some((_, c)) if c.is_ascii_uppercase() => {}
        Some((_, found)) => return Err(TickerRefusal::BadStart { found }),
    }
    for (index, found) in chars {
        if !(found.is_ascii_uppercase() || found.is_ascii_digit() || found == '-') {
            return Err(TickerRefusal::BadChar { index, found });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_capitals_digits_and_hyphens_starting_with_a_letter() {
        for ok in ["A", "BTC", "TEST2", "BTC-1-CPY", "EVT-1-EBY", "OI-GW-PROBE"] {
            assert_eq!(validate_ticker(ok), Ok(()), "{ok}");
        }
        assert_eq!(validate_ticker(&"X".repeat(MAX_TICKER_LEN)), Ok(()));
    }

    #[test]
    fn refuses_each_way_a_ticker_can_be_wrong_by_name() {
        assert_eq!(validate_ticker(""), Err(TickerRefusal::Empty));
        assert_eq!(
            validate_ticker(&"X".repeat(MAX_TICKER_LEN + 1)),
            Err(TickerRefusal::TooLong {
                len: MAX_TICKER_LEN + 1
            })
        );
        assert_eq!(
            validate_ticker("test3"),
            Err(TickerRefusal::BadStart { found: 't' })
        );
        assert_eq!(
            validate_ticker("1BTC"),
            Err(TickerRefusal::BadStart { found: '1' })
        );
        assert_eq!(
            validate_ticker("-BTC"),
            Err(TickerRefusal::BadStart { found: '-' })
        );
        assert_eq!(
            validate_ticker("TEST3."),
            Err(TickerRefusal::BadChar {
                index: 5,
                found: '.'
            })
        );
        assert_eq!(
            validate_ticker("TEST 3"),
            Err(TickerRefusal::BadChar {
                index: 4,
                found: ' '
            })
        );
        assert_eq!(
            validate_ticker("Btc"),
            Err(TickerRefusal::BadChar {
                index: 1,
                found: 't'
            })
        );
        assert_eq!(
            validate_ticker("BTCÉ"),
            Err(TickerRefusal::BadChar {
                index: 3,
                found: 'É'
            })
        );
    }

    #[test]
    fn says_what_is_wrong_in_the_message() {
        assert_eq!(
            validate_ticker("TEST3.").map_err(|e| e.to_string()),
            Err("ticker may hold only A-Z, 0-9 and '-', found '.' at character 5".to_owned())
        );
    }
}
