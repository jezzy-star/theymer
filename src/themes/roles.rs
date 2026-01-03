//! Semantic color vocabulary to define how a scheme's palette is used.
//!
//! _Roles_ are a collection of common semantic components of themeable
//! interfaces that sit as an intermediary abstraction layer between the *what*
//! — the colors used by the scheme — and a template's rendered output by
//! describing *how* the palette gets rendered into concrete key/value pairs.
//! For instance, the cutiepro scheme maps **blackboard** (`#181716`) to the
//! `bg` role:
//!
//! ```toml
//! [palette]
//! blackboard = "#181716"
//!
//! [roles]
//! bg = "$blackboard"
//! ```
//!
//! In the Helix port template, `ui.background` gets assigned to the resolved
//! color value of `bg`:
//!
//! ```toml
//! # before rendering:
//! "ui.background" = "{{ bg }}"
//!
//! # rendered with cutiepro:
//! "ui.background" = "#181716"
//! ```
//!
//! ...while in kitty's configuration the same thing is expressed as:
//!
//! ```shell
//! # before rendering:
//! background  {{ bg }}
//!
//! # rendered with cutiepro:
//! background  #181716
//! ```

#![allow(
    non_camel_case_types,
    reason = "`define_roles` macro generates lowercase variants"
)]

use indexmap::IndexMap;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::result::Result as StdResult;
use std::str::FromStr;

use owo_colors::OwoColorize as _;
use owo_colors::Stream::Stdout;
use serde::{Deserialize, Deserializer, Serialize};

use super::SwatchName;

macro_rules! define_roles {
    // parse group
    (@parse
        [$( $roles:tt )*]
        [$( $groups:tt )*]
        $group:ident { $( $group_roles:literal ),* $( , )? }
        $( , $($rest:tt)* )?
    ) => {
        define_roles!(@parse
            [$($roles)* $(concat!(stringify!($group), ".", $group_roles),)*]
            [$($groups)* $group]
            $($($rest)*)?
        );
    };

    // parse role name
    (@parse
        [$( $roles:tt )*]
        [$( $groups:tt )*]
        $role:literal
        $(, $($rest:tt)*)?
    ) => {
        define_roles!(@parse
            [$($roles)* $role ,]
            [$($groups)*]
            $($($rest)*)?
        );
    };

    // base case
    (@parse
        [$( $roles:tt )*]
        [$( $groups:ident )*]
    ) => {
        const ROLES: &[&str] = &[
            $($roles)*
        ];

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub(crate) enum Group {
            Root,
            $($groups,)*
        }

        impl Name {
            pub(crate) fn group(&self) -> Group {
                match self.0 {
                    $(
                        s if s.starts_with(concat!(stringify!($groups), "."))
                            => Group::$groups,
                    )*
                    _ => Group::Root,
                }
            }
        }
    };

    // entry
    ( $( $item:tt )* ) => {
        define_roles!(@parse [] [] $($item)*);
    };
}

define_roles! {
    "bg",
    "bg_alt",

    "fg",
    "fg_alt",

    "toolbar",
    "toolbar_popup",
    "toolbar_alt",

    "select",
    "select_2nd",
    "select_alt",

    "accent",
    "accent_2nd",
    "accent_separator",
    "accent_popup",
    "accent_linenum",

    "inactive",
    "focus",

    "guide",
    "guide_inlay",
    "guide_linenum",
    "guide_ruler",
    "guide_whitespace",

    "match",
    "error",
    "warning",
    "info",
    "hint",

    debug {
        "active",
        "breakpoint",
        "frameline",
    },

    mode {
        "normal",
        "normal_2nd",

        "insert",
        "insert_2nd",

        "select",
        "select_2nd",
    },

    syntax {
        "variable",
        "variable_builtin",
        "variable_parameter",
        "variable_member",

        "keyword",
        "keyword_operator",
        "keyword_function",
        "keyword_conditional",
        "keyword_repeat",
        "keyword_import",
        "keyword_return",
        "keyword_exception",
        "keyword_directive",
        "keyword_storage",

        "type",
        "type_builtin",
        "type_variant",

        "function",
        "function_builtin",
        "function_method",
        "function_macro",

        "constant",
        "constant_builtin",
        "constant_boolean",
        "constant_number",
        "constant_character",

        "label",
        "constructor",
        "string",
        "attribute",
        "namespace",

        "tag",
        "tag_builtin",

        "comment",
        "comment_doc",

        "operator",

        "punctuation",

        "special",
        "special_function",
        "special_character",
        "special_string",
        "special_punctuation",
    },

    diff {
        "plus",
        "minus",

        "delta",
        "delta_moved",
        "delta_conflict",
    },

    markup {
        "heading",
        "heading_2nd",
        "heading_3rd",
        "heading_4th",
        "heading_5th",
        "heading_6th",

        "list",
        "list_numbered",
        "list_checked",
        "list_unchecked",

        "link",
        "link_text",

        "bold",
        "italic",
        "strikethrough",
        "quote",
        "raw",
    },

    ansi {
        "black",
        "black_bright",

        "red",
        "red_bright",

        "green",
        "green_bright",

        "yellow",
        "yellow_bright",

        "blue",
        "blue_bright",

        "magenta",
        "magenta_bright",

        "cyan",
        "cyan_bright",

        "white",
        "white_bright",
    },
}

const VARIANT_SEPARATOR: char = '_';

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Roles(pub IndexMap<Name, Value>);

impl Roles {
    pub(crate) fn new() -> Self {
        Self(IndexMap::new())
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&Name, &Value)> {
        self.0.iter()
    }

    pub(crate) fn get(&self, name: &Name) -> Option<&Value> {
        self.0.get(name)
    }

    pub(crate) fn set_roles(&self) -> indexmap::map::Keys<'_, Name, Value> {
        self.0.keys()
    }

    pub(crate) fn contains_role(&self, name: &Name) -> bool {
        self.0.contains_key(name)
    }

    pub(crate) fn insert(&mut self, name: Name, value: Value) -> Option<Value> {
        self.0.insert(name, value)
    }

    pub(crate) fn extend(&mut self, other: IndexMap<Name, Value>) {
        self.0.extend(other);
    }
}

impl<'a> IntoIterator for &'a Roles {
    type IntoIter = indexmap::map::Iter<'a, Name, Value>;
    type Item = (&'a Name, &'a Value);

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResolvedRoles(IndexMap<Name, Resolved>);

impl ResolvedRoles {
    pub(crate) fn new() -> Self {
        Self(IndexMap::new())
    }

    pub(crate) fn get(&self, name: &Name) -> Option<&Resolved> {
        self.0.get(name)
    }

    pub(crate) fn insert(
        &mut self,
        name: Name,
        role: Resolved,
    ) -> Option<Resolved> {
        self.0.insert(name, role)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&Name, &Resolved)> {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a ResolvedRoles {
    type IntoIter = indexmap::map::Iter<'a, Name, Resolved>;
    type Item = (&'a Name, &'a Resolved);

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

type Result<T> = StdResult<T, Error>;

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("undefined role `{0}`")]
    Undefined(String),

    #[error("circular role reference: {}", format_circular_chain(.0))]
    CircularReference(Vec<String>),

    #[error("required role `{0}` missing")]
    MissingRequired(String),
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub(crate) enum Kind {
    Base(Name),
    Optional { base: Name },
}

impl Kind {
    pub(crate) const fn base(&self) -> &Name {
        match self {
            Self::Base(name) => name,
            Self::Optional { base } => base,
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum Value {
    Swatch(SwatchName),
    Role(Name),
}

impl Value {
    pub(crate) fn parse(val: &str) -> Result<Self> {
        if let Some(swatch_name) = val.strip_prefix('$') {
            let display_name =
                SwatchName::parse(swatch_name).map_err(|_err| {
                    Error::Undefined(format!(
                        "invalid swatch reference: `${swatch_name}`"
                    ))
                })?;
            Ok(Self::Swatch(display_name))
        } else {
            let role_name = val.parse()?;
            Ok(Self::Role(role_name))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub(crate) struct Name(&'static str);

impl Name {
    #[expect(clippy::unreachable, reason = "guaranteed by role design")]
    #[must_use]
    pub(crate) fn classify(&self) -> Kind {
        if is_base(self.as_str()) {
            Kind::Base(*self)
        } else {
            let base_str = extract_base(self.as_str());
            let base_role = ROLES
                .iter()
                .find(|&&role| role == base_str)
                .copied()
                .map_or_else(
                    || unreachable!("optional roles always have a base"),
                    Self,
                );

            Kind::Optional { base: base_role }
        }
    }

    #[must_use]
    pub(crate) const fn as_str(&self) -> &str {
        self.0
    }
}

impl Display for Name {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Name {
    type Err = Error;

    fn from_str(s: &str) -> StdResult<Self, Self::Err> {
        ROLES
            .iter()
            .find(|&&role| role == s)
            .copied()
            .map(Self)
            .ok_or_else(|| Error::Undefined(s.to_owned()))
    }
}

impl<'de> Deserialize<'de> for Name {
    fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        s.parse().map_err(serde::de::Error::custom)
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct Resolved {
    pub swatch: String,
    pub ascii: String,
    pub hex: String,
    pub rgb: (u8, u8, u8),
}

pub(crate) fn iter() -> impl Iterator<Item = Name> {
    ROLES.iter().copied().map(Name)
}

pub(crate) fn base() -> impl Iterator<Item = Name> {
    ROLES.iter().copied().filter(|&s| is_base(s)).map(Name)
}

fn format_circular_chain(roles: &[String]) -> String {
    roles
        .iter()
        .enumerate()
        .map(|(i, role)| {
            if i == roles.len() - 1 {
                format!(
                    "`{}`",
                    role.if_supports_color(Stdout, |text| text
                        .red()
                        .underline()
                        .to_string())
                )
            } else {
                format!("`{role}`")
            }
        })
        .collect::<Vec<_>>()
        .join(" -> ")
}

fn extract_base(role: &str) -> &str {
    if let Some((base, _)) = role.rsplit_once(VARIANT_SEPARATOR) {
        return base;
    }

    role
}

fn is_base(role: &str) -> bool {
    extract_base(role) == role
}

#[cfg(test)]
mod tests {
    use pretty_assertions::{assert_eq, assert_ne};

    use super::*;

    fn create_invalid_role_names() -> Vec<&'static str> {
        vec![
            "selection",
            "42",
            "acc ent",
            "alt_accent",
            "syntax.keyword_function_alt",
        ]
    }

    #[test]
    fn roles_are_valid() {
        let roles = iter()
            .map(|s| s.as_str().parse())
            .collect::<Result<Vec<Name>>>()
            .unwrap_or_else(|e| panic!("invalid `ROLE` name {e}"));
    }

    fn role_name_errors_on_invalid_name() {}
}
