use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{Error, Name};
use crate::config::{self, FILENAME};


#[derive(Debug, Clone)]
pub(crate) struct Config {
    pub inherit: bool,
    pub dirs: ResolvedDirs,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(default)]
struct Raw {
    pub inherit: bool,
    pub dirs: Dirs,
}


#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ResolvedDirs {
    pub schemes: PathBuf,
    pub templates: PathBuf,
    pub render: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(default)]
struct Dirs {
    pub schemes: String,
    pub templates: String,
    pub render: String,
}

impl Default for Dirs {
    fn default() -> Self {
        Self {
            schemes: "schemes".to_owned(),
            templates: "templates".to_owned(),
            render: "render".to_owned(),
        }
    }
}


pub(crate) fn load(
    themes_dir: &Path,
    name: &Name,
    config: &crate::Config,
) -> crate::Result<Option<Config>> {
    let path = themes_dir.join(FILENAME);
    let content = fs::read_to_string(&path).map_err(|src| Error::Reading {
        path: path.display().to_string(),
        src,
    })?;

    let raw: Raw = config::parse(content.as_str())?;

    Ok(Some(Config {
        inherit: raw.inherit,
        dirs: ResolvedDirs {
            schemes: config::expand_and_resolve(&raw.dirs.schemes, themes_dir)?,
            templates: config::expand_and_resolve(
                &raw.dirs.templates,
                themes_dir,
            )?,
            render: if raw.inherit {
                config.project.render_all_into.as_ref().map_or_else(
                    || themes_dir.join(&raw.dirs.render),
                    |dir| dir.join(name.as_str()),
                )
            } else {
                themes_dir.join(&raw.dirs.render)
            },
        },
    }))
}
