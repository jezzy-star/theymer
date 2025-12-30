use std::fmt::{Display, Formatter, Result as FmtResult};
use std::marker::PhantomData;
use std::result::Result as StdResult;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use unicode_normalization::UnicodeNormalization as _;

use crate::Result;
use crate::output::{Ascii, Unicode};

const MAX_NAME_LENGTH: usize = 255;

const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6",
    "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6",
    "LPT7", "LPT8", "LPT9",
];

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("invalid {context} name `{name}`: {reason}")]
    Invalid {
        context: String,
        name: String,
        reason: String,
    },
    #[error("invalid {context} ascii name `{name}`: {reason}")]
    InvalidAscii {
        context: String,
        name: String,
        reason: String,
    },
    // TODO: offer suggestions on how to fix
    #[error(
        "invalid generated ascii fallback `{ascii_name}` for {context} \
         `{display_name}`: {reason}"
    )]
    GeneratingAscii {
        context: String,
        display_name: String,
        ascii_name: String,
        reason: String,
    },
}

fn is_reserved(name: &str) -> bool {
    let upper = name.to_uppercase();

    WINDOWS_RESERVED.iter().any(|&reserved| upper == reserved)
}

fn is_safe(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '_'
}

pub(crate) fn normalize_and_validate(
    name: &str,
    context: &str,
) -> Result<String> {
    let normalized = name.nfc().collect::<String>();

    if normalized.is_empty() {
        return Err(Error::Invalid {
            context: context.to_owned(),
            name: name.to_owned(),
            reason: "empty".to_owned(),
        }
        .into());
    }

    if !normalized.chars().all(is_safe) {
        return Err(Error::Invalid {
            context: context.to_owned(),
            name: name.to_owned(),
            reason: "contains character that's not a unicode letter, number, \
                     `-` or `_`"
                .to_owned(),
        }
        .into());
    }

    if normalized.len() > MAX_NAME_LENGTH {
        return Err(Error::Invalid {
            context: context.to_owned(),
            name: name.to_owned(),
            reason: format!(
                "too long ({} characters; max is {MAX_NAME_LENGTH})",
                normalized.len()
            ),
        }
        .into());
    }

    if is_reserved(&normalized) {
        return Err(Error::Invalid {
            context: context.to_owned(),
            name: name.to_owned(),
            reason: "uses reserved windows name".to_owned(),
        }
        .into());
    }

    Ok(normalized)
}

pub(crate) fn validate_set_ascii(name: &str, context: &str) -> Result<()> {
    if !name.is_ascii() {
        // TODO: which character(s)?
        return Err(Error::InvalidAscii {
            context: context.to_owned(),
            name: name.to_owned(),
            reason: "contains non-ascii character(s)".to_owned(),
        }
        .into());
    }

    Ok(())
}

fn validate_auto_ascii(
    display_name: &str,
    ascii_name: &str,
    context: &str,
) -> Result<()> {
    if ascii_name.len() > MAX_NAME_LENGTH {
        return Err(Error::GeneratingAscii {
            context: context.to_owned(),
            display_name: display_name.to_owned(),
            ascii_name: ascii_name.to_owned(),
            reason: format!(
                "too long ({} characters; max is {MAX_NAME_LENGTH})",
                ascii_name.len()
            ),
        }
        .into());
    }

    if is_reserved(ascii_name) {
        return Err(Error::GeneratingAscii {
            context: context.to_owned(),
            display_name: display_name.to_owned(),
            ascii_name: ascii_name.to_owned(),
            reason: "uses reserved windows name".to_owned(),
        }
        .into());
    }

    if ascii_name.ends_with('.') {
        return Err(Error::GeneratingAscii {
            context: context.to_owned(),
            display_name: display_name.to_owned(),
            ascii_name: ascii_name.to_owned(),
            reason: "ends with `.`".to_owned(),
        }
        .into());
    }

    Ok(())
}

#[expect(clippy::assigning_clones, reason = "can't use `clone_into`")]
pub(crate) fn to_ascii(display_name: &str, context: &str) -> Result<String> {
    let mut ascii_name = deunicode::deunicode(display_name);

    let mut last_was_sep = false;
    ascii_name = ascii_name
        .chars()
        .filter_map(|c| match c {
            c if c.is_ascii_alphanumeric() => {
                last_was_sep = false;
                Some(c)
            }
            '-' | '_' => {
                if last_was_sep {
                    None
                } else {
                    last_was_sep = true;
                    Some(c)
                }
            }
            ' ' | '/' | ':' | ',' | ';' | '|' | '+' => {
                if last_was_sep {
                    None
                } else {
                    last_was_sep = true;
                    Some('-')
                }
            }
            _ => {
                if last_was_sep {
                    None
                } else {
                    last_was_sep = true;
                    Some('_')
                }
            }
        })
        .collect::<String>();

    ascii_name = ascii_name.trim_matches(|c| c == '-' || c == '_').to_owned();

    if ascii_name.is_empty() {
        return Err(Error::GeneratingAscii {
            context: context.to_owned(),
            display_name: display_name.to_owned(),
            ascii_name,
            reason: "transliteration produced no valid filename characters"
                .to_owned(),
        }
        .into());
    }

    validate_auto_ascii(display_name, &ascii_name, context)?;

    Ok(ascii_name)
}

pub(crate) trait TextKind {
    fn validate(normalized: &str, domain: &str) -> Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Validated<const DOMAIN: &'static str, K> {
    inner: String,
    _kind: PhantomData<K>,
}

impl<const DOMAIN: &'static str, K: TextKind> Validated<DOMAIN, K> {
    pub(crate) fn parse(s: &str) -> Result<Self> {
        s.parse()
    }

    #[must_use]
    pub(crate) fn as_str(&self) -> &str {
        &self.inner
    }
}

impl<const DOMAIN: &'static str> Validated<DOMAIN, Unicode> {
    pub(crate) fn to_ascii(&self) -> Result<Validated<DOMAIN, Ascii>> {
        let ascii_string = to_ascii(&self.inner, DOMAIN)?;

        Ok(Validated {
            inner: ascii_string,
            _kind: PhantomData,
        })
    }
}

impl<const DOMAIN: &'static str, K: TextKind> FromStr for Validated<DOMAIN, K> {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self> {
        let normalized = normalize_and_validate(s, DOMAIN)?;

        K::validate(&normalized, DOMAIN)?;

        Ok(Self {
            inner: normalized,
            _kind: PhantomData,
        })
    }
}

impl<const DOMAIN: &'static str, K> Display for Validated<DOMAIN, K> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", &self.inner)
    }
}

impl<const DOMAIN: &'static str, K> Serialize for Validated<DOMAIN, K> {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.inner)
    }
}

impl<'de, const DOMAIN: &'static str, K: TextKind> Deserialize<'de>
    for Validated<DOMAIN, K>
{
    fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

impl TextKind for Unicode {
    fn validate(_normalized: &str, _domain: &str) -> Result<()> {
        Ok(())
    }
}

impl TextKind for Ascii {
    fn validate(normalized: &str, domain: &str) -> Result<()> {
        validate_set_ascii(normalized, domain)
    }
}
