use std::path::Path;
use std::result::Result as StdResult;
use std::{fs, io};
use walkdir::WalkDir;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::ProjectType;
use crate::extensions::Merge as _;
use crate::output::{Ascii, Unicode};


pub(crate) mod schemes;

mod config;
mod names;
mod roles;
mod swatches;

pub(crate) use self::config::Config;
pub(crate) use self::names::{Error as NameError, Validated as ValidatedName};
pub(crate) use self::roles::{
    Error as RoleError, Kind as RoleKind, Name as RoleName,
    Resolved as ResolvedRole, ResolvedRoles, Roles, Value as RoleValue,
};
pub(crate) use self::schemes::{
    Error as SchemeError, Extra, Meta, Name as SchemeName, Raw as RawScheme,
    ResolvedExtra, Scheme,
};
pub(crate) use self::swatches::{
    Error as SwatchError, Name as SwatchName, Palette, Swatch,
};


const BASE_FILENAME: &str = "theme.toml";


pub(crate) type Name = ValidatedName<"theme", Unicode>;
type AsciiName = ValidatedName<"theme", Ascii>;


// TODO: add error for empty schemes dir in multi-scheme themes
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error(
        "theme '{theme}' has neither a `{BASE_FILENAME}` nor a schemes \
         directory (`{schemes_dir}`)"
    )]
    MissingThemeBaseAndSchemesDir { theme: String, schemes_dir: String },

    #[error("failed to read directory `{0}` (invalid utf-8?)")]
    ReadingDir(String),

    #[error("failed to read theme file `{path}`: {src}")]
    Reading { path: String, src: io::Error },

    #[error("failed to parse theme file `{path}`: {src}")]
    Parsing {
        path: String,
        src: Box<toml::de::Error>,
    },
}


#[derive(Debug, Clone, Serialize)]
pub(crate) struct Theme {
    #[serde(rename(serialize = "theme"))]
    pub name: Name,

    #[serde(rename(serialize = "theme_ascii"))]
    pub name_ascii: AsciiName,

    #[serde(skip)]
    pub schemes: IndexMap<SchemeName, Scheme>,

    #[serde(skip)]
    pub config: Option<Config>,
}


#[derive(Debug, Deserialize)]
struct Base {
    name_ascii: Option<String>,

    #[serde(flatten)]
    raw_scheme: RawScheme,
}


#[expect(
    clippy::enum_variant_names,
    reason = "false positive -- this naming pattern makes the most sense"
)]
enum Type {
    SingleScheme,
    MultiScheme,
}


pub(crate) fn load_all(
    config: &crate::Config,
) -> crate::Result<IndexMap<Name, Theme>> {
    discover_themes(config)?
        .into_iter()
        .map(|name| {
            let theme = load(name, config)?;

            Ok((theme.name.clone(), theme))
        })
        .collect()
}


// TODO: rewrite this to be cleaner
pub(crate) fn load(name: Name, config: &crate::Config) -> crate::Result<Theme> {
    let themes_dir = config
        .project
        .root
        .join(&config.dirs.themes)
        .join(name.as_str());
    let theme_config = config::load(&themes_dir, &name, config)?;

    let schemes_dir = theme_config.as_ref().map_or_else(
        || config.dirs.schemes.clone(),
        |tc| tc.dirs.schemes.clone(),
    );

    let base_path = themes_dir.join(BASE_FILENAME);

    let theme_type = if schemes_dir.exists() && schemes_dir.is_dir() {
        Type::MultiScheme
    } else if base_path.exists() {
        Type::SingleScheme
    } else {
        return Err(Error::MissingThemeBaseAndSchemesDir {
            theme: name.to_string(),
            schemes_dir: schemes_dir.display().to_string(),
        }
        .into());
    };

    let base = if base_path.exists() {
        Some(load_base(&base_path)?)
    } else {
        None
    };

    let name_ascii = if let Some(base) = &base
        && let Some(ascii) = &base.name_ascii
    {
        AsciiName::parse(ascii)?
    } else {
        name.to_ascii()?
    };

    let schemes = match theme_type {
        Type::SingleScheme => {
            if let Some(base) = base {
                let mut schemes = IndexMap::new();
                let scheme = base.raw_scheme.into_scheme(name.as_str())?;

                schemes.insert(scheme.name.clone(), scheme);

                schemes
            } else {
                return Err(Error::MissingThemeBaseAndSchemesDir {
                    theme: name.to_string(),
                    schemes_dir: schemes_dir.display().to_string(),
                }
                .into());
            }
        }
        Type::MultiScheme => {
            if schemes_dir.exists() && schemes_dir.is_dir() {
                let base_scheme = base.as_ref().map(|b| &b.raw_scheme);

                load_schemes(&schemes_dir, base_scheme)?
            } else if let Some(base) = base {
                let mut schemes = IndexMap::new();
                let scheme = base.raw_scheme.into_scheme(name.as_str())?;

                schemes.insert(scheme.name.clone(), scheme);

                schemes
            } else {
                return Err(Error::MissingThemeBaseAndSchemesDir {
                    theme: name.to_string(),
                    schemes_dir: schemes_dir.display().to_string(),
                }
                .into());
            }
        }
    };

    Ok(Theme {
        name,
        name_ascii,
        schemes,
        config: theme_config,
    })
}


fn discover_themes(config: &crate::Config) -> crate::Result<Vec<Name>> {
    match config.project.r#type {
        ProjectType::Monotheme => {
            let raw_name = config
                .project
                .root
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| {
                    Error::ReadingDir(config.project.root.display().to_string())
                })?;
            let name = Name::parse(raw_name)?;

            Ok(vec![name])
        }

        ProjectType::Polytheme => {
            let themes_dir = config.project.root.join(&config.dirs.themes);

            fs::read_dir(&themes_dir)?
                .filter_map(StdResult::ok)
                .filter(|entry| entry.path().is_dir())
                .map(|entry| {
                    let path = entry.path();
                    let raw_name =
                        path.file_name().and_then(|n| n.to_str()).ok_or_else(
                            || Error::ReadingDir(path.display().to_string()),
                        )?;
                    let name = Name::parse(raw_name)?;

                    Ok(name)
                })
                .collect()
        }
    }
}


fn load_schemes(
    dir: &Path,
    base: Option<&RawScheme>,
) -> crate::Result<IndexMap<SchemeName, Scheme>> {
    let mut schemes = IndexMap::new();

    for entry in WalkDir::new(dir)
        .max_depth(1)
        .into_iter()
        .filter_map(StdResult::ok)
    {
        let path = entry.path();

        if !path.is_file() || path.extension() != Some("toml".as_ref()) {
            continue;
        }

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| Error::ReadingDir(path.display().to_string()))?;
        let mut raw = schemes::load_raw(path)?;

        if let Some(base) = base {
            raw = raw.merge(base.clone());
        }

        let scheme = raw.into_scheme(name)?;

        schemes.insert(scheme.name.clone(), scheme);
    }

    Ok(schemes)
}


fn load_base(path: &Path) -> crate::Result<Base> {
    let content = fs::read_to_string(path).map_err(|src| Error::Reading {
        path: path.display().to_string(),
        src,
    })?;

    let root: toml::Table =
        toml::from_str(&content).map_err(|src| Error::Parsing {
            path: path.display().to_string(),
            src: Box::new(src),
        })?;

    let name_ascii = root
        .get("name_ascii")
        .and_then(|v| v.as_str())
        .map(String::from);

    let raw_scheme = schemes::load_raw(path)?;

    Ok(Base {
        name_ascii,
        raw_scheme,
    })
}
