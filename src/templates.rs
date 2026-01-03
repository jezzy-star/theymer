use std::fs;
use std::path::Path;
use std::result::Result as StdResult;

use anyhow::Context as _;
use indexmap::IndexMap;
use walkdir::WalkDir;

use crate::extensions::PathExt as _;
use crate::{Config, Error, Result};


pub(crate) mod directives;
pub(crate) mod providers;

pub(crate) use self::directives::{Directives, Error as DirectiveError};
pub(crate) use self::providers::{
    Error as ProviderError, Resolved as ResolvedProvider,
};


pub(crate) const SET_TEST_OBJECT: &str = "_set";
pub(crate) const JINJA_TEMPLATE_SUFFIX: &str = ".jinja";
pub(crate) const SKIP_RENDERING_PREFIX: char = '_';


#[derive(Debug)]
pub(crate) struct Loader {
    pub env: minijinja::Environment<'static>,
    pub providers: Vec<ResolvedProvider>,
    pub directives: IndexMap<String, Directives>,
}

impl Loader {
    pub(crate) fn init(config: &Config) -> Result<Self> {
        Self::load(config)
    }

    pub(crate) fn with_directives(
        &self,
    ) -> anyhow::Result<
        IndexMap<&str, (minijinja::Template<'_, '_>, &Directives)>,
    > {
        let mut map: IndexMap<
            &str,
            (minijinja::Template<'_, '_>, &Directives),
        > = IndexMap::new();
        for (name, t) in self.env.templates() {
            let directives = self.directives.get(name).ok_or_else(|| {
                Error::InternalBug {
                    module: "templates",
                    reason: format!(
                        "template `{name}` in jinja env but missing from \
                         directives map"
                    ),
                }
            })?;
            map.insert(name, (t, directives));
        }

        Ok(map)
    }

    pub(crate) fn resolve_blob(&self, url: &str) -> Result<String> {
        Ok(providers::resolve_blob(url, &self.providers)?)
    }

    fn load(config: &Config) -> Result<Self> {
        let mut env = minijinja::Environment::new();

        env.set_undefined_behavior(minijinja::UndefinedBehavior::SemiStrict);
        env.set_trim_blocks(true);
        env.set_lstrip_blocks(true);

        env.add_test("set", Self::create_set_test);

        env.add_filter("code", |s: String| -> String { format!("`{s}`") });

        let directives = Self::templates_with_directives(
            &mut env,
            &config.dirs.templates,
            &config.strip_directives,
        )?;

        let providers = providers::resolve(&config.providers)?;

        Ok(Self {
            env,
            providers,
            directives,
        })
    }

    fn create_set_test(
        state: &minijinja::State<'_, '_>,
        value: &minijinja::Value,
    ) -> StdResult<bool, minijinja::Error> {
        let role_name = value.as_str().ok_or_else(|| {
            minijinja::Error::new(
                minijinja::ErrorKind::InvalidOperation,
                "`set` test requires a string argument",
            )
        })?;

        match state.lookup("_set") {
            Some(set) => {
                if let Ok(items) = set.try_iter() {
                    for item in items {
                        if let Some(name) = item.as_str()
                            && name == role_name
                        {
                            return Ok(true);
                        }
                    }
                }

                Ok(false)
            }
            None => Err(minijinja::Error::new(
                minijinja::ErrorKind::UndefinedError,
                "`_set` missing from context",
            )),
        }
    }

    fn templates_with_directives(
        env: &mut minijinja::Environment<'static>,
        dir: &Path,
        strip_patterns: &[Vec<String>],
    ) -> Result<IndexMap<String, Directives>> {
        let mut directives_map = IndexMap::new();

        for entry in WalkDir::new(dir).into_iter().filter_map(StdResult::ok) {
            let path = entry.path();

            if path.is_jinja() {
                let name = path
                    .strip_prefix(dir)
                    .with_context(|| {
                        format!(
                            "stripping directory prefix from template path \
                             `{}`",
                            path.display()
                        )
                    })
                    .map_err(Error::template)?
                    .to_string_lossy()
                    .replace('\\', "/");

                let raw_src = fs::read_to_string(path)
                    .with_context(|| {
                        format!("reading template `{}`", path.display())
                    })
                    .map_err(Error::template)?;

                let (directives, filtered) = Directives::from_template(
                    &name,
                    &raw_src,
                    strip_patterns,
                    path.to_string_lossy().as_str(),
                )
                .map_err(Error::Directive)?;

                directives_map.insert(name.clone(), directives);

                env.add_template_owned(name.clone(), filtered)
                    .with_context(|| format!("compiling template `{name}`"))
                    .map_err(Error::template)?;
            }
        }

        Ok(directives_map)
    }
}
