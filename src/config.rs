use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::result::Result as StdResult;
use std::{env, fs, io};

use indexmap::IndexMap;
use log::debug;
use serde::Deserialize;

use crate::extensions::Merge as _;


pub(crate) const FILENAME: &str = "theymer.toml";


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
    pub project: ResolvedProject,
    pub dirs: ResolvedDirs,

    #[serde(rename(serialize = "provider"))]
    pub providers: Vec<Provider>,
}


#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(default)]
struct Raw {
    pub strip_directives: Vec<Vec<String>>,
    pub project: Option<Project>,
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
            project: None,
            dirs: Dirs::default(),
            providers: default_providers(),
        }
    }
}


#[non_exhaustive]
#[derive(Debug, Deserialize)]
pub struct ResolvedProject {
    pub r#type: ProjectType,
    pub render_all_into: Option<PathBuf>,
    pub root: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Project {
    pub polytheme: bool,

    #[serde(default)]
    pub render_all_into: Option<String>,
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

const fn detect_project_type(project_table: Option<&Project>) -> ProjectType {
    if let Some(project) = project_table
        && project.polytheme
    {
        return ProjectType::Polytheme;
    }

    ProjectType::Monotheme
}


#[non_exhaustive]
#[derive(Debug, Deserialize)]
pub struct ResolvedDirs {
    pub themes: PathBuf,
    pub schemes: PathBuf,
    pub templates: PathBuf,
}

#[non_exhaustive]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(default)]
pub struct Dirs {
    pub themes: String,
    pub schemes: String,
    pub templates: String,
}

impl Default for Dirs {
    fn default() -> Self {
        Self {
            themes: "themes".to_owned(),
            schemes: "schemes".to_owned(),
            templates: "templates".to_owned(),
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


pub(crate) fn load() -> Result<Config> {
    let cwd = env::current_dir().map_err(|src| Error::Reading { src })?;

    let root = find_project_root(&cwd)?;

    debug!("using project root `{}`", root.display());

    let path = root.join(FILENAME);
    let content =
        fs::read_to_string(&path).map_err(|src| Error::Reading { src })?;

    // FIXME: remove once all code is updated to use absolute paths based on
    // `config.project_root`
    env::set_current_dir(&root).map_err(|src| Error::ChangingDir {
        cwd: cwd.display().to_string(),
        root: root.display().to_string(),
        src,
    })?;

    let raw: Raw = parse(content.as_str())?;

    Ok(Config {
        strip_directives: raw.strip_directives,
        project: ResolvedProject {
            r#type: detect_project_type(raw.project.as_ref()),
            render_all_into: raw
                .project
                .as_ref()
                .and_then(|p| p.render_all_into.as_ref())
                .filter(|s| !s.is_empty())
                .map(|s| expand_and_resolve(s, &root))
                .transpose()?,
            root: root.clone(),
        },
        dirs: ResolvedDirs {
            themes: expand_and_resolve(&raw.dirs.themes, &root)?,
            schemes: expand_and_resolve(&raw.dirs.schemes, &root)?,
            templates: expand_and_resolve(&raw.dirs.templates, &root)?,
        },
        providers: merge_providers_with_defaults(&raw.providers),
    })
}


pub(crate) fn parse<T>(content: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned + Default,
{
    if content.trim().is_empty() {
        return Ok(T::default());
    }

    toml::from_str(content).map_err(|src| Error::Parsing { src: Box::new(src) })
}


pub(crate) fn expand_and_resolve(path: &str, root: &Path) -> Result<PathBuf> {
    let expanded =
        shellexpand::full(path)
            .map(Cow::into_owned)
            .map_err(|src| Error::ExpandingPath {
                path: path.to_owned(),
                src,
            })?;

    if Path::new(&expanded).is_absolute() {
        Ok(PathBuf::from(expanded))
    } else {
        Ok(root.join(expanded))
    }
}


fn find_project_root(cwd: &Path) -> Result<PathBuf> {
    cwd.ancestors()
        .find(|dir| dir.join(FILENAME).exists())
        .map(PathBuf::from)
        .ok_or_else(|| Error::NoProjectRoot {
            cwd: cwd.display().to_string(),
        })
}
