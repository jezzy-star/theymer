use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::output::FileStatus;
use crate::{
    Manifest, ManifestEntry, Scheme, SchemeName, Theme, ThemeName, manifest,
};


pub(super) type Index = Manifest<Entry>;

impl Index {
    pub(crate) fn check(
        &self,
        path: &Path,
        theme: &Theme,
        scheme: &Scheme,
        template: &minijinja::Template<'_, '_>,
    ) -> anyhow::Result<FileStatus> {
        let Some(entry) = self.get(path) else {
            return Ok(FileStatus::NotTracked);
        };

        manifest::check_status(path, &entry.render_hash, || {
            Ok(hash_theme(theme)? != entry.theme_hash
                || hash_scheme(scheme)? != entry.scheme_hash
                || hash_template(template) != entry.template_hash)
        })
    }

    pub(crate) fn create_entry(
        path: &Path,
        theme: &Theme,
        scheme: &Scheme,
        template: &minijinja::Template<'_, '_>,
        content: &str,
    ) -> anyhow::Result<Entry> {
        Ok(Entry {
            path: path.to_path_buf(),
            template: template.name().to_owned(),
            theme: theme.name.clone(),
            scheme: scheme.name.clone(),
            render_hash: manifest::hash(content),
            template_hash: hash_template(template),
            theme_hash: hash_theme(theme)?,
            scheme_hash: hash_scheme(scheme)?,
        })
    }
}


#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub path: PathBuf,
    pub theme: ThemeName,
    pub scheme: SchemeName,
    pub template: String,
    pub render_hash: String,
    pub theme_hash: String,
    pub scheme_hash: String,
    pub template_hash: String,
}

impl ManifestEntry for Entry {
    const FILENAME: &'static str = "index.json";
    const VERSION: u8 = 0;

    fn path(&self) -> &Path {
        &self.path
    }

    fn hash(&self) -> &str {
        &self.render_hash
    }
}


fn hash_theme(theme: &Theme) -> anyhow::Result<String> {
    let json = serde_json::to_string_pretty(theme)?;

    Ok(manifest::hash(&json))
}


fn hash_scheme(scheme: &Scheme) -> anyhow::Result<String> {
    let json = serde_json::to_string_pretty(scheme)?;

    Ok(manifest::hash(&json))
}


fn hash_template(template: &minijinja::Template<'_, '_>) -> String {
    manifest::hash(template.source())
}
