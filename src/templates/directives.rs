use std::path::Path;
use std::result::Result as StdResult;
use std::sync::Arc;

use indexmap::{IndexMap, IndexSet};
use itertools::Itertools as _;

use self::DirectiveType::{Other, Theymer};
use crate::extensions::PathExt as _;
use crate::output::{ColorStyle, Style, TextStyle};

type Result<T> = StdResult<T, Error>;

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("incomplete theymer directive `{directive}` in `{path}`")]
    Incomplete { directive: String, path: String },

    #[error(
        "unknown theymer directive{} `{}` in `{path}`",
        if .directives.len() > 1 { "s" } else { "" },
        format_list(.directives)
    )]
    Unknown {
        directives: Vec<String>,
        path: String,
    },

    #[error(
        "invalid value `{value}` for directive `{directive}` in `{path}`: \
         expected `true` or `false`"
    )]
    InvalidBool {
        value: String,
        directive: String,
        path: String,
    },
}

fn format_list(directives: &[String]) -> String {
    directives.iter().map(|d| format!("`{d}`")).join(", ")
}

#[non_exhaustive]
#[derive(Debug)]
pub(crate) struct Directives {
    pub style: Arc<Style>,
    pub source: Option<String>,
    pub passthrough: IndexSet<String>,
}

impl Directives {
    pub(crate) fn from_template(
        name: &str,
        content: &str,
        strip_patterns: &[Vec<String>],
        path: &str,
    ) -> Result<(Self, String)> {
        let mut theymer = IndexMap::new();
        let mut passthrough = IndexSet::new();
        let mut content_lines = Vec::new();

        for line in content.lines() {
            let classified = Self::classify(line, strip_patterns, path)?;

            match classified {
                LineType::Directive(Theymer { key, val }) => {
                    theymer.insert(key, val);
                }
                LineType::Directive(Other(text)) => {
                    passthrough.insert(Self::canonicalize(&text));
                }
                LineType::Content => {
                    content_lines.push(line);
                }
            }
        }

        let style = Arc::new(Self::extract_style(&mut theymer, name)?);
        let source = theymer.shift_remove("source");

        // TODO: refactor into own function
        if !theymer.is_empty() {
            let unknown: Vec<_> =
                theymer.keys().map(ToOwned::to_owned).collect();

            return Err(Error::Unknown {
                directives: unknown,
                path: path.to_owned(),
            });
        }

        passthrough.sort_unstable();

        let filtered = Self::trim_ends(&content_lines);

        Ok((
            Self {
                style,
                source,
                passthrough,
            },
            filtered,
        ))
    }

    pub(crate) fn make_header(&self, output_path: &Path) -> String {
        let mut directives = self.passthrough.clone();

        if output_path.is_toml() {
            let format_disabled_str = "#:tombi format.disabled = true";

            let canonical = Self::canonicalize(format_disabled_str);

            if !directives
                .iter()
                .any(|d| Self::canonicalize(d) == canonical)
            {
                directives.insert(format_disabled_str.to_owned());
            }
        }

        directives.sort_unstable();

        if directives.is_empty() {
            String::new()
        } else {
            format!("{}\n\n", directives.into_iter().join("\n"))
        }
    }

    fn classify(
        line: &str,
        strip_patterns: &[Vec<String>],
        path: &str,
    ) -> Result<LineType> {
        let trimmed = line.trim();

        if trimmed.is_empty()
            || trimmed.starts_with("##")
            || !trimmed.starts_with('#')
        {
            return Ok(LineType::Content);
        }

        // TODO: support theymer directives inside jinja comments
        if let Some(part) = trimmed.strip_prefix("#theymer:") {
            if let Some((k, v)) = part.trim().split_once('=') {
                return Ok(LineType::Directive(Theymer {
                    key: k.trim().to_owned(),
                    val: v.trim().to_owned(),
                }));
            }

            return Err(Error::Incomplete {
                directive: part.trim().to_owned(),
                path: path.to_owned(),
            });
        }

        if Self::matches(strip_patterns, trimmed) {
            return Ok(LineType::Directive(Other(trimmed.to_owned())));
        }

        Ok(LineType::Content)
    }

    fn trim_ends(content: &[&str]) -> String {
        let Some(start) = content.iter().position(|l| !l.trim().is_empty())
        else {
            return String::new();
        };

        let end = content
            .iter()
            .rposition(|l| !l.trim().is_empty())
            .map_or(content.len(), |i| i + 1);

        #[expect(
            clippy::indexing_slicing,
            reason = "start and end are always within bounds"
        )]
        content[start..end].join("\n")
    }

    fn canonicalize(directive: &str) -> String {
        directive
            .to_lowercase()
            .split('=')
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(" = ")
    }

    fn extract_style(
        raw: &mut IndexMap<String, String>,
        path: &str,
    ) -> Result<Style> {
        let mut style = Style::default();

        if let Some(v) = raw.shift_remove("render_swatch_names") {
            style.color = if Self::parse_bool("render_swatch_names", &v, path)?
            {
                ColorStyle::Name
            } else {
                ColorStyle::Hex
            };
        }

        if let Some(v) = raw.shift_remove("render_as_ascii") {
            style.text = if Self::parse_bool("render_as_ascii", &v, path)? {
                TextStyle::Ascii
            } else {
                TextStyle::Unicode
            }
        }

        Ok(style)
    }

    fn parse_bool(directive: &str, value: &str, path: &str) -> Result<bool> {
        match value {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(Error::InvalidBool {
                value: value.to_owned(),
                directive: directive.to_owned(),
                path: path.to_owned(),
            }),
        }
    }

    fn matches(patterns: &[Vec<String>], line: &str) -> bool {
        patterns.iter().any(|p| {
            p.iter()
                .try_fold(0, |from, part| {
                    line.get(from..)
                        .and_then(|s| s.find(part))
                        .map(|pos| from + pos + part.len())
                })
                .is_some()
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
enum DirectiveType {
    Theymer { key: String, val: String },
    Other(String),
}

#[derive(Debug, Clone, PartialEq)]
enum LineType {
    Content,
    Directive(DirectiveType),
}
