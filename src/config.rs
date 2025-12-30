use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::result::Result as StdResult;
use std::{env, fs, io};

use indexmap::IndexMap;
use log::debug;
use serde::Deserialize;

use crate::extensions::Merge as _;

const FILENAME: &str = "theymer.toml";

type Result<T> = StdResult<T, Error>;

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("failed to find `{FILENAME}` in `{cwd}` or any parent directory")]
    NoProjectRoot { cwd: String },

    #[error("failed to read `{FILENAME}`: {src}")]
    Reading { src: io::Error },

    #[error("failed to parse `{FILENAME}`: {src}")]
    Parsing { src: Box<toml::de::Error> },

    #[error("failed to expand path `{path}`: {src}")]
    ExpandingPath {
        path: String,
        src: shellexpand::LookupError<env::VarError>,
    },

    #[error("failed to move from `{cwd}` to project root `{root}`: {src}")]
    ChangingDir {
        cwd: String,
        root: String,
        src: io::Error,
    },
}

#[non_exhaustive]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub strip_directives: Vec<Vec<String>>,
    pub dirs: Dirs,

    #[serde(rename(serialize = "provider"))]
    pub providers: Vec<Provider>,

    pub project_type: ProjectType,
    pub project_root: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(default)]
struct Raw {
    pub strip_directives: Vec<Vec<String>>,
    pub dirs: Dirs,

    #[serde(rename(serialize = "provider"))]
    pub providers: Vec<Provider>,
}

impl Default for Raw {
    fn default() -> Self {
        Self {
            // TODO: figure out a design where defaults can be extended by the
            // user instead of completely overridden
            strip_directives: vec![vec!["#:tombi".to_owned()]],
            dirs: Dirs::default(),
            providers: default_providers(),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(default)]
pub struct Dirs {
    pub themes: String,
    pub schemes: String,
    pub templates: String,
    pub render: String,
}

impl Default for Dirs {
    fn default() -> Self {
        Self {
            themes: "themes".to_owned(),
            schemes: "schemes".to_owned(),
            templates: "templates".to_owned(),
            render: "render".to_owned(),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    pub host: String,
    pub blob_path: Option<String>,
    pub raw_path: Option<String>,
    pub branch: Option<String>,
}

fn default_providers() -> Vec<Provider> {
    vec![
        Provider {
            host: "github.com".to_owned(),
            blob_path: Some(
                "{host}/{owner}/{repo}/blob/{ref}/{file}".to_owned(),
            ),
            raw_path: Some(
                "raw.githubusercontent.com/{owner}/{repo}/{ref}/{file}"
                    .to_owned(),
            ),
            branch: None,
        },
        Provider {
            host: "gitlab.com".to_owned(),
            blob_path: Some(
                "{host}/{owner}/{repo}/-/blob/{ref}/{file}".to_owned(),
            ),
            raw_path: Some(
                "{host}/{owner}/{repo}/-/raw/{ref}/{file}".to_owned(),
            ),
            branch: None,
        },
        Provider {
            host: "codeberg.org".to_owned(),
            blob_path: Some(
                "{host}/{owner}/{repo}/src/branch/{ref}/{file}".to_owned(),
            ),
            raw_path: Some(
                "{host}/{owner}/{repo}/raw/branch/{ref}/{file}".to_owned(),
            ),
            branch: None,
        },
        Provider {
            host: "bitbucket.org".to_owned(),
            blob_path: Some(
                "{host}/{owner}/{repo}/src/{ref}/{file}".to_owned(),
            ),
            raw_path: Some("{host}/{owner}/{repo}/raw/{ref}/{file}".to_owned()),
            branch: None,
        },
    ]
}

fn merge_providers_with_defaults(user_providers: &[Provider]) -> Vec<Provider> {
    let mut providers: IndexMap<String, Provider> = default_providers()
        .into_iter()
        .map(|h| (h.host.clone(), h))
        .collect();

    for user_provider in user_providers {
        if let Some(default) = providers.get(&user_provider.host) {
            let merged = user_provider.clone().merge(default.clone());

            providers.insert(merged.host.clone(), merged);
        } else {
            providers.insert(user_provider.host.clone(), user_provider.clone());
        }
    }

    providers.into_values().collect()
}

#[expect(
    clippy::exhaustive_enums,
    reason = "unlikely to add more project types"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ProjectType {
    Monotheme,
    Polytheme,
}

fn detect_project_type(project_root: &Path, themes_dir: &str) -> ProjectType {
    if project_root.join(themes_dir).exists() {
        return ProjectType::Polytheme;
    }

    ProjectType::Monotheme
}

pub(crate) fn load() -> Result<Config> {
    let cwd = env::current_dir().map_err(|src| Error::Reading { src })?;

    let project_root = find_project_root(&cwd)?;

    debug!("using project root `{}`", project_root.display());

    let config_path = project_root.join(FILENAME);
    let content = fs::read_to_string(&config_path)
        .map_err(|src| Error::Reading { src })?;

    // FIXME: remove once all code is updated to use absolute paths based on
    // `config.project_root`
    env::set_current_dir(&project_root).map_err(|src| Error::ChangingDir {
        cwd: cwd.display().to_string(),
        root: project_root.display().to_string(),
        src,
    })?;

    let raw: Raw = parse(content.as_str())?;

    Ok(Config {
        strip_directives: raw.strip_directives,
        dirs: Dirs {
            themes: expand_and_resolve(&raw.dirs.themes, &project_root)?,
            schemes: expand_and_resolve(&raw.dirs.schemes, &project_root)?,
            templates: expand_and_resolve(&raw.dirs.templates, &project_root)?,
            render: expand_and_resolve(&raw.dirs.render, &project_root)?,
        },
        providers: merge_providers_with_defaults(&raw.providers),
        project_type: detect_project_type(&project_root, &raw.dirs.themes),
        project_root,
    })
}

fn find_project_root(cwd: &Path) -> Result<PathBuf> {
    cwd.ancestors()
        .find(|dir| dir.join(FILENAME).exists())
        .map(PathBuf::from)
        .ok_or_else(|| Error::NoProjectRoot {
            cwd: cwd.display().to_string(),
        })
}

fn parse(content: &str) -> Result<Raw> {
    if content.trim().is_empty() {
        return Ok(Raw::default());
    }

    toml::from_str(content).map_err(|src| Error::Parsing { src: Box::new(src) })
}

fn expand_and_resolve(path: &str, project_root: &Path) -> Result<String> {
    shellexpand::full(path)
        .map(Cow::into_owned)
        .map_err(|src| Error::ExpandingPath {
            path: path.to_owned(),
            src,
        })
        .map(|expanded| {
            if Path::new(&expanded).is_absolute() {
                expanded
            } else {
                project_root.join(expanded).display().to_string()
            }
        })
}
