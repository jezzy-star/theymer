#![feature(adt_const_params)]
#![feature(default_field_values)]
#![feature(more_qualified_paths)]
#![feature(multiple_supertrait_upcastable)]
#![feature(must_not_suspend)]
#![feature(non_exhaustive_omitted_patterns_lint)]
#![feature(stmt_expr_attributes)]
#![feature(str_as_str)]
#![feature(strict_provenance_lints)]
#![feature(supertrait_item_shadowing)]
#![feature(unsized_const_params)]
#![allow(missing_docs, reason = "todo: better documentation")]
#![allow(clippy::missing_docs_in_private_items, reason = "todo: documentation")]
#![allow(clippy::missing_errors_doc, reason = "todo: documentation")]
#![expect(
    incomplete_features,
    reason = "`unsized_const_params` is useful but not finalized yet"
)]
#![expect(
    clippy::redundant_pub_crate,
    reason = "seems to be broken for `pub(crate)` errors"
)]


use std::io;
use std::result::Result as StdResult;


pub mod cli;
pub mod config;

pub(crate) mod themes;

mod extensions;
mod manifest;
mod output;
mod render;
mod templates;

pub use self::config::{Config, ProjectType};

pub(crate) use self::manifest::{Entry as ManifestEntry, Manifest};
pub(crate) use self::themes::{Name as ThemeName, Scheme, SchemeName, Theme};

use self::config::Error as ConfigError;
use self::manifest::Error as ManifestError;
use self::output::UpstreamError;
use self::templates::{DirectiveError, ProviderError};
use self::themes::{
    Error as ThemeError, NameError, RoleError, SchemeError, SwatchError,
};


pub type Result<T> = StdResult<T, Error>;

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
#[expect(
    private_interfaces,
    reason = "this is fine for this kind of error type I think?"
)]
pub enum Error {
    #[error("configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("manifest error: {0}")]
    Manifest(#[from] ManifestError),

    #[error("theme error: {0}")]
    Theme(#[from] ThemeError),

    #[error("scheme error: {0}")]
    Scheme(#[from] SchemeError),

    #[error("palette error: {0}")]
    Swatch(#[from] SwatchError),

    #[error("role error: {0}")]
    Role(#[from] RoleError),

    #[error("name validation error: {0}")]
    Name(#[from] NameError),

    #[error("error processing template: {0}")]
    Template(#[source] anyhow::Error),

    #[error("directive error: {0}")]
    Directive(#[from] DirectiveError),

    #[error("git provider error: {0}")]
    Provider(#[from] ProviderError),

    #[error("error rendering: {0}")]
    Rendering(#[source] anyhow::Error),

    #[error("upstream error: {0}")]
    Upstream(#[from] UpstreamError),

    #[error("internal error in {module}: {reason}! this is a bug!")]
    InternalBug {
        module: &'static str,
        reason: String,
    },

    #[error("file system error: {0}")]
    Io(#[from] io::Error),
}

impl Error {
    pub(crate) fn template(err: impl Into<anyhow::Error>) -> Self {
        Self::Template(err.into())
    }

    pub(crate) fn rendering(err: impl Into<anyhow::Error>) -> Self {
        Self::Rendering(err.into())
    }
}
