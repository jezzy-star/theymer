use std::path::Path;
use std::result::Result as StdResult;
use std::{fs, io};

use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use super::{
    Palette, ResolvedRole, ResolvedRoles, RoleError, RoleKind, RoleName,
    RoleValue, Roles, Swatch, SwatchError, ValidatedName, roles,
};
use crate::Result;
use crate::extensions::PathExt as _;
use crate::output::{Ascii, Unicode};


const MAX_META_FIELD_LENGTH: usize = 1000;


pub(crate) type Name = ValidatedName<"scheme", Unicode>;
pub(crate) type AsciiName = ValidatedName<"scheme", Ascii>;


// TODO: move most of these to `super::load`?
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("role `{role}` references non-existent swatch `{swatch}`")]
    UndefinedSwatch { role: String, swatch: String },

    #[error("invalid meta field `{field}`: {reason}")]
    InvalidMeta { field: String, reason: String },

    #[error("invalid structure in `{path}`: {reason}")]
    InvalidStructure { path: String, reason: String },

    #[error("invalid toml syntax in `{path}`: {src}")]
    ParsingRaw {
        path: String,
        src: Box<toml::de::Error>,
    },

    #[error("failed to deserialize `{section}` section in `{path}`: {src}")]
    Deserializing {
        section: String,
        path: String,
        src: Box<toml::de::Error>,
    },

    #[error("failed to read scheme `{path}`: {src}")]
    Reading { path: String, src: io::Error },
}

#[non_exhaustive]
#[derive(Debug, Serialize)]
pub(crate) struct Scheme {
    #[serde(rename(serialize = "scheme"))]
    pub name: Name,

    #[serde(rename(serialize = "scheme_ascii"))]
    pub name_ascii: AsciiName,

    pub meta: Meta,
    pub palette: Palette,

    #[serde(skip)]
    pub roles: Roles,

    #[serde(flatten)]
    pub resolved_roles: ResolvedRoles,

    pub extra: Option<Extra>,
    pub resolved_extra: Option<ResolvedExtra>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Raw {
    pub scheme: Option<Name>,
    pub scheme_ascii: Option<AsciiName>,
    pub meta: Meta,
    pub palette: Palette,
    pub roles: Roles,
    pub extra: Option<Extra>,
}

impl Raw {
    pub(crate) fn into_scheme(self, fallback_name: &str) -> Result<Scheme> {
        let resolved_roles = self.resolve_roles()?;
        let resolved_extra = self
            .extra
            .as_ref()
            .map(|extra| {
                Self::resolve_extra(extra, &self.palette, &resolved_roles)
            })
            .transpose()?;
        let (scheme, scheme_ascii) = Self::names(&self, fallback_name)?;

        Ok(Scheme {
            name: scheme,
            name_ascii: scheme_ascii,
            meta: self.meta.clone(),
            palette: self.palette,
            roles: self.roles,
            resolved_roles,
            extra: self.extra,
            resolved_extra,
        })
    }

    fn resolve_roles(&self) -> Result<ResolvedRoles> {
        let mut resolved_roles = ResolvedRoles::new();
        let mut missing: Vec<String> = Vec::new();

        for base_role in roles::base() {
            if !self.roles.contains_role(&base_role) {
                missing.push(base_role.to_string());
            }
        }

        if !missing.is_empty() {
            return Err(RoleError::MissingRequired(format!(
                "missing required roles: {}",
                missing.join(", ")
            ))
            .into());
        }

        for role in roles::iter() {
            let mut visited = IndexSet::new();
            match self.resolve_role(role, &mut visited) {
                Ok(resolved) => {
                    resolved_roles.insert(role, resolved);
                }
                Err(e) => return Err(e),
            }
        }

        Ok(resolved_roles)
    }

    fn resolve_role(
        &self,
        role: RoleName,
        visited: &mut IndexSet<RoleName>,
    ) -> Result<ResolvedRole> {
        if !visited.insert(role) {
            let mut chain: Vec<String> =
                visited.iter().map(ToString::to_string).collect();
            chain.push(role.to_string());

            return Err(crate::Error::Role(RoleError::CircularReference(
                chain,
            )));
        }

        match self.roles.get(&role) {
            Some(RoleValue::Swatch(display_name)) => {
                let swatch = self
                    .palette
                    .get(display_name.as_str())
                    .ok_or_else(|| Error::UndefinedSwatch {
                        role: role.to_string(),
                        swatch: display_name.to_string(),
                    })?;

                Ok(Self::resolved_role_from(swatch))
            }
            Some(RoleValue::Role(role_name)) => {
                self.resolve_role(*role_name, visited)
            }
            None => match role.classify() {
                RoleKind::Base(_name) => {
                    Err(RoleError::MissingRequired(role.to_string()).into())
                }
                RoleKind::Optional { base } => self.resolve_role(base, visited),
            },
        }
    }

    fn resolve_extra(
        extra: &Extra,
        palette: &Palette,
        resolved_roles: &ResolvedRoles,
    ) -> Result<ResolvedExtra> {
        let rainbow = extra
            .rainbow
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let value = RoleValue::parse(s).map_err(|_src| {
                    crate::Error::Role(RoleError::Undefined(format!(
                        "extra.rainbow[{i}]"
                    )))
                })?;

                Self::resolve_value(
                    &value,
                    palette,
                    resolved_roles,
                    &format!("`extra.rainbow[{i}]`"),
                )
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(ResolvedExtra { rainbow })
    }

    fn resolved_role_from(swatch: &Swatch) -> ResolvedRole {
        ResolvedRole {
            hex: swatch.hex().to_string(),
            swatch: swatch.name.to_string(),
            ascii: swatch.ascii.to_string(),
            rgb: swatch.rgb(),
        }
    }

    fn resolve_value(
        value: &RoleValue,
        palette: &Palette,
        resolved_roles: &ResolvedRoles,
        role: &str,
    ) -> Result<ResolvedRole> {
        match value {
            RoleValue::Swatch(name) => {
                let swatch = palette.get(name.as_str()).ok_or_else(|| {
                    Error::UndefinedSwatch {
                        role: role.to_owned(),
                        swatch: name.to_string(),
                    }
                })?;

                Ok(Self::resolved_role_from(swatch))
            }
            RoleValue::Role(name) => {
                resolved_roles.get(name).cloned().ok_or_else(|| {
                    crate::Error::Role(RoleError::Undefined(format!(
                        "`{role}` -> `{name}`"
                    )))
                })
            }
        }
    }

    fn names(&self, fallback_name: &str) -> Result<(Name, AsciiName)> {
        let name = match self.scheme.clone() {
            Some(name) => name,
            None => Name::parse(fallback_name)?,
        };

        let name_ascii = match self.scheme_ascii.clone() {
            Some(ascii) => ascii,
            None => name.to_ascii()?,
        };

        Ok((name, name_ascii))
    }

    fn parse_roles(roles_val: &toml::Value, path: &String) -> Result<Roles> {
        let mut roles = Roles::new();

        let table =
            roles_val
                .as_table()
                .ok_or_else(|| Error::InvalidStructure {
                    path: path.to_owned(),
                    reason: "`roles` must be a table".to_owned(),
                })?;

        for (key, val) in table {
            if let Some(nested_table) = val.as_table() {
                for (nested_key, nested_val) in nested_table {
                    let full_key = format!("{key}.{nested_key}");
                    Self::parse_role(&full_key, nested_val, path, &mut roles)?;
                }
            } else {
                Self::parse_role(key, val, path, &mut roles)?;
            }
        }

        Ok(roles)
    }

    fn parse_role(
        role_key: &str,
        val: &toml::Value,
        path: &str,
        parsed: &mut Roles,
    ) -> Result<()> {
        let role_name =
            role_key.parse().map_err(|_src| Error::InvalidStructure {
                path: path.to_owned(),
                reason: format!("invalid role name: `{role_key}`"),
            })?;
        let val_str = val.as_str().ok_or_else(|| Error::InvalidStructure {
            path: path.to_owned(),
            reason: format!("role `{role_key}` must be a string"),
        })?;

        parsed.insert(role_name, RoleValue::parse(val_str)?);

        Ok(())
    }

    fn parse_extra(val: &toml::Value, path: &str) -> Result<Extra> {
        let table = val.as_table().ok_or_else(|| Error::InvalidStructure {
            path: path.to_owned(),
            reason: "`roles` must be a table".to_owned(),
        })?;

        let rainbow = match table.get("rainbow") {
            Some(val) => {
                let arr =
                    val.as_array().ok_or_else(|| Error::InvalidStructure {
                        path: path.to_owned(),
                        reason: "`extra.rainbow` must be an array".to_owned(),
                    })?;

                arr.iter()
                    .enumerate()
                    .map(|(i, v)| {
                        v.as_str()
                            .ok_or_else(|| {
                                Error::InvalidStructure {
                                    path: path.to_owned(),
                                    reason: format!(
                                        "`extra.rainbow[{i}]` must be a string"
                                    ),
                                }
                                .into()
                            })
                            .map(ToString::to_string)
                    })
                    .collect::<Result<Vec<_>>>()?
            }
            None => Vec::new(),
        };

        Ok(Extra { rainbow })
    }

    fn parse_palette(val: &toml::Value, path: &str) -> Result<Palette> {
        let table = val.as_table().ok_or_else(|| Error::Deserializing {
            section: "palette".to_owned(),
            path: path.to_owned(),
            src: Box::new(<toml::de::Error as serde::de::Error>::custom(
                "palette must be a table",
            )),
        })?;

        let mut palette = Palette::new();

        for (display_key, v) in table {
            let swatch = Swatch::parse(display_key, v)?;
            palette.insert(swatch);
        }

        Self::check_ascii_collisions(&palette)?;
        Self::check_case_collisions(&palette)?;

        Ok(palette)
    }

    fn check_ascii_collisions(palette: &Palette) -> Result<()> {
        let mut ascii_to_display: IndexMap<String, Vec<String>> =
            IndexMap::new();

        for swatch in palette {
            ascii_to_display
                .entry(swatch.ascii.to_string())
                .or_default()
                .push(swatch.name.to_string());
        }

        for (ascii_name, display_names) in ascii_to_display {
            if display_names.len() > 1 {
                return Err(SwatchError::AsciiNameCollision {
                    ascii_name,
                    display_names,
                }
                .into());
            }
        }

        Ok(())
    }

    fn check_case_collisions(palette: &Palette) -> Result<()> {
        let mut lowercase_to_original: IndexMap<String, Vec<String>> =
            IndexMap::new();

        for swatch in palette {
            let lowercase = swatch.name.as_str().to_lowercase();
            lowercase_to_original
                .entry(lowercase)
                .or_default()
                .push(swatch.name.to_string());
        }

        for (_lowercase, original_names) in lowercase_to_original {
            if original_names.len() > 1 {
                return Err(SwatchError::NameCaseCollision {
                    names: original_names,
                }
                .into());
            }
        }

        Ok(())
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct Meta {
    pub author: Option<String>,
    pub author_ascii: Option<String>,
    pub license: Option<String>,
    pub license_ascii: Option<String>,
    pub blurb: Option<String>,
    pub blurb_ascii: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct Extra {
    #[serde(default)]
    pub rainbow: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResolvedExtra {
    pub rainbow: Vec<ResolvedRole>,
}

pub(crate) fn load_all(dir: &str) -> Result<IndexMap<String, Scheme>> {
    let mut schemes = IndexMap::new();

    for entry in WalkDir::new(dir).into_iter().filter_map(StdResult::ok) {
        let path = entry.path();

        if path.is_toml() {
            let name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| crate::Error::InternalBug {
                    module: "schemes",
                    reason: format!(
                        "attempted to load scheme with corrupted path `{}`",
                        path.display(),
                    ),
                })?;

            let scheme = load(name, path)?;

            schemes.insert(name.to_owned(), scheme);
        }
    }

    Ok(schemes)
}

pub(crate) fn load(name: &str, path: &Path) -> Result<Scheme> {
    let raw = load_raw(path)?;

    raw.into_scheme(name)
}

pub(crate) fn load_raw(path: &Path) -> Result<Raw> {
    let path_str = path.display().to_string();

    let content =
        fs::read_to_string(&path_str).map_err(|src| Error::Reading {
            path: path_str.clone(),
            src,
        })?;

    let root: toml::Table =
        toml::from_str(&content).map_err(|src| Error::ParsingRaw {
            path: path_str.clone(),
            src: Box::new(src),
        })?;

    let scheme: Option<Name> = root
        .get("scheme")
        .map(|v| {
            let s = v.as_str().ok_or_else(|| Error::Deserializing {
                section: "scheme".to_owned(),
                path: path_str.clone(),
                src: Box::new(<toml::de::Error as serde::de::Error>::custom(
                    "`scheme` must be a string",
                )),
            })?;
            Name::parse(s).map_err(|e| Error::Deserializing {
                section: "scheme".to_owned(),
                path: path_str.clone(),
                src: Box::new(<toml::de::Error as serde::de::Error>::custom(
                    format!("{e}"),
                )),
            })
        })
        .transpose()?;

    let scheme_ascii: Option<AsciiName> = root
        .get("scheme_ascii")
        .map(|v| {
            let s = v.as_str().ok_or_else(|| Error::Deserializing {
                section: "scheme_ascii".to_owned(),
                path: path_str.clone(),
                src: Box::new(<toml::de::Error as serde::de::Error>::custom(
                    "`scheme_ascii` must be a string",
                )),
            })?;
            AsciiName::parse(s).map_err(|e| Error::Deserializing {
                section: "scheme_ascii".to_owned(),
                path: path_str.clone(),
                src: Box::new(<toml::de::Error as serde::de::Error>::custom(
                    format!("{e}"),
                )),
            })
        })
        .transpose()?;

    let meta: Meta = root
        .get("meta")
        .map(|v| {
            v.clone().try_into().map_err(|src| Error::Deserializing {
                section: "meta".to_owned(),
                path: path_str.clone(),
                src: Box::new(src),
            })
        })
        .transpose()?
        .unwrap_or_default();

    // TODO: refactor into `validate_meta`
    validate_meta_field("author", meta.author.as_ref())?;
    validate_meta_field("author_ascii", meta.author_ascii.as_ref())?;
    validate_meta_field("license", meta.license.as_ref())?;
    validate_meta_field("license_ascii", meta.license_ascii.as_ref())?;
    validate_meta_field("blurb", meta.blurb.as_ref())?;
    validate_meta_field("blurb_ascii", meta.blurb_ascii.as_ref())?;

    let palette_val =
        root.get("palette").ok_or_else(|| Error::Deserializing {
            section: "palette".to_owned(),
            path: path_str.clone(),
            src: Box::new(
                <toml::de::Error as serde::de::Error>::missing_field("palette"),
            ),
        })?;

    let palette = Raw::parse_palette(palette_val, &path_str)?;

    let roles_val = root.get("roles").ok_or_else(|| Error::Deserializing {
        section: "roles".to_owned(),
        path: path_str.clone(),
        src: Box::new(<toml::de::Error as serde::de::Error>::missing_field(
            "roles",
        )),
    })?;

    let roles = Raw::parse_roles(roles_val, &path_str)?;

    let extra = match root.get("extra") {
        Some(val) => Some(Raw::parse_extra(val, &path_str)?),
        None => None,
    };

    let raw = Raw {
        scheme,
        scheme_ascii,
        meta,
        palette,
        roles,
        extra,
    };

    Ok(raw)
}

fn validate_meta_field(name: &str, value: Option<&String>) -> Result<()> {
    if let Some(text) = value
        && text.len() > MAX_META_FIELD_LENGTH
    {
        return Err(Error::InvalidMeta {
            field: name.to_owned(),
            reason: format!(
                "too long ({} characters; max is {MAX_META_FIELD_LENGTH})",
                text.len()
            ),
        }
        .into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io::Write as _;

    use indoc::indoc;
    use minijinja::Value as JinjaValue;
    use tempfile::NamedTempFile;

    use super::*;

    fn create_temp_scheme_file(toml: &str) -> NamedTempFile {
        let mut temp =
            NamedTempFile::new().expect("failed to create temp file");
        temp.write_all(toml.as_bytes())
            .expect("failed to write temp file");

        temp
    }

    fn scheme_from_toml(name: &str, toml: &str) -> Result<Scheme> {
        let temp = create_temp_scheme_file(toml);

        load(name, temp.path())
    }

    fn assert_role_hex_equals(
        context: &BTreeMap<String, JinjaValue>,
        role: &str,
        expected: &str,
    ) {
        let obj = context
            .get(role)
            .unwrap_or_else(|| panic!("role `{role}` not found in context`"));
        let actual = obj
            .get_attr("hex")
            .unwrap_or_else(|_| panic!("role `{role}` missing `hex` field"));

        assert_eq!(
            actual,
            expected.into(),
            "role `{role}` has wrong hex value"
        );
    }

    fn assert_nested_role_hex_equals(
        context: &BTreeMap<String, JinjaValue>,
        group: &str,
        role: &str,
        expected: &str,
    ) {
        let group_obj = context
            .get(group)
            .unwrap_or_else(|| panic!("group `{group}` not found in context"));
        let role_obj = group_obj.get_attr(role).unwrap_or_else(|_| {
            panic!("role `{group}.{role}` not found in context")
        });
        let actual = role_obj.get_attr("hex").unwrap_or_else(|_| {
            panic!("role `{group}.{role}` missing `hex` field")
        });

        assert_eq!(
            actual,
            expected.into(),
            "role `{group}.{role}` has wrong hex value"
        );
    }

    fn minimal_valid_scheme() -> &'static str {
        indoc! {r##"
            [palette]
            black = "#000"
            red = "#ff0000"
            green = "#00ff00"
            yellow = "#ffff00"
            blue = "#0000ff"
            magenta = "#ff00ff"
            cyan = "#00ffff"
            white = "#fff"

            [roles.ansi]
            black = "$black"
            red = "$red"
            green = "$green"
            yellow = "$yellow"
            blue = "$blue"
            magenta = "$magenta"
            cyan = "$cyan"
            white = "$white"
            "##}
    }
}
