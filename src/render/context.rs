use std::collections::BTreeMap;
use std::sync::Arc;

use indexmap::IndexMap;

use super::Color;
use crate::Result;
use crate::output::{Special, Style, TextStyle};
use crate::themes::{
    Meta, ResolvedExtra, ResolvedRole, RoleName, Scheme, Swatch,
};

pub(crate) fn build(
    scheme: &Scheme,
    special: &Special,
    style: &Arc<Style>,
    current_swatch: Option<&str>,
) -> Result<BTreeMap<String, minijinja::Value>> {
    let mut ctx = BTreeMap::new();

    let mut groups: BTreeMap<String, BTreeMap<String, minijinja::Value>> =
        BTreeMap::new();

    let swatch_roles = map_swatches_to_roles(scheme);

    insert_meta(&mut ctx, scheme, style);

    insert_palette(&mut ctx, scheme, &swatch_roles, style);

    for (role_name, resolved_role) in &scheme.resolved_roles {
        insert_role(
            &mut ctx,
            scheme,
            &mut groups,
            role_name,
            resolved_role,
            style,
        )?;
    }

    for (group_name, group_map) in groups {
        ctx.insert(group_name, minijinja::Value::from(group_map));
    }

    if let Some(resolved_extra) = &scheme.resolved_extra {
        insert_extra(&mut ctx, resolved_extra, style);
    }

    if let Some(name) = current_swatch {
        insert_current_swatch(&mut ctx, scheme, name, &swatch_roles, style)?;
    }

    insert_special(&mut ctx, special);

    insert_set_test_roles(&mut ctx, scheme);

    Ok(ctx)
}

fn ascii_fallback(
    unicode: Option<&String>,
    ascii: Option<&String>,
) -> Option<String> {
    ascii
        .cloned()
        .or_else(|| unicode.map(|s| deunicode::deunicode(s)))
}

fn insert_meta(
    ctx: &mut BTreeMap<String, minijinja::Value>,
    scheme: &Scheme,
    style: &Arc<Style>,
) {
    ctx.insert(
        "scheme".to_owned(),
        minijinja::Value::from_serialize(&scheme.name),
    );

    ctx.insert(
        "scheme_ascii".to_owned(),
        minijinja::Value::from_serialize(&scheme.name_ascii),
    );

    let author_ascii = ascii_fallback(
        scheme.meta.author.as_ref(),
        scheme.meta.author_ascii.as_ref(),
    );

    let license_ascii = ascii_fallback(
        scheme.meta.license.as_ref(),
        scheme.meta.license_ascii.as_ref(),
    );

    let blurb_ascii = ascii_fallback(
        scheme.meta.blurb.as_ref(),
        scheme.meta.blurb_ascii.as_ref(),
    );

    let meta_ctx = if style.text == TextStyle::Ascii {
        Meta {
            author: author_ascii.clone(),
            author_ascii,
            license: license_ascii.clone(),
            license_ascii,
            blurb: blurb_ascii.clone(),
            blurb_ascii,
        }
    } else {
        scheme.meta.clone()
    };

    ctx.insert(
        "meta".to_owned(),
        minijinja::Value::from_serialize(&meta_ctx),
    );
}

fn map_swatches_to_roles(scheme: &Scheme) -> IndexMap<String, Vec<String>> {
    let mut map: IndexMap<String, Vec<String>> = IndexMap::new();

    for swatch in &scheme.palette {
        map.insert(swatch.name.to_string(), Vec::new());
    }

    for (role_name, resolved_role) in &scheme.resolved_roles {
        if let Some(roles) = map.get_mut(&resolved_role.swatch) {
            roles.push(role_name.to_string());
        }
    }

    map
}

fn insert_palette(
    ctx: &mut BTreeMap<String, minijinja::Value>,
    scheme: &Scheme,
    swatch_roles: &IndexMap<String, Vec<String>>,
    style: &Arc<Style>,
) {
    let palette: Vec<minijinja::Value> = scheme
        .palette
        .iter()
        .map(|swatch| {
            let name = swatch.name.to_string();

            let roles = swatch_roles.get(&name).cloned().unwrap_or_default();

            minijinja::Value::from_serialize(Color::swatch(
                swatch.hex().to_string(),
                name,
                swatch.ascii.to_string(),
                swatch.rgb(),
                roles,
                Arc::clone(style),
            ))
        })
        .collect();

    ctx.insert("palette".to_owned(), minijinja::Value::from(palette));
}

fn rgb(
    scheme: &Scheme,
    role_name: &RoleName,
    resolved_role: &ResolvedRole,
) -> Result<(u8, u8, u8)> {
    scheme
        .palette
        .get(resolved_role.swatch.as_str())
        .ok_or_else(|| crate::Error::InternalBug {
            module: "schemes",
            reason: format!(
                "resolved role `{role_name}` references missing swatch `${}`",
                &resolved_role.swatch
            ),
        })
        .map(Swatch::rgb)
}

fn insert_grouped_role(
    role_obj: Color,
    groups: &mut BTreeMap<String, BTreeMap<String, minijinja::Value>>,
    group: &str,
    key: &str,
) {
    groups
        .entry(group.to_owned())
        .or_default()
        .insert(key.to_owned(), minijinja::Value::from_object(role_obj));
}

fn insert_role(
    ctx: &mut BTreeMap<String, minijinja::Value>,
    scheme: &Scheme,
    groups: &mut BTreeMap<String, BTreeMap<String, minijinja::Value>>,
    role_name: &RoleName,
    resolved_role: &ResolvedRole,
    style: &Arc<Style>,
) -> Result<()> {
    let parts: Vec<&str> = role_name.as_str().split('.').collect();

    let rgb = rgb(scheme, role_name, resolved_role)?;

    let obj = Color::role(
        resolved_role.hex.clone(),
        resolved_role.swatch.clone(),
        resolved_role.ascii.clone(),
        rgb,
        Arc::clone(style),
    );

    match parts.as_slice() {
        [key] => {
            ctx.insert((*key).to_owned(), minijinja::Value::from_object(obj));
        }
        [group, key] => {
            insert_grouped_role(obj, groups, group, key);
        }
        _ => {
            return Err(crate::Error::InternalBug {
                module: "schemes",
                reason: format!(
                    "role {role_name} not formatted like `[group.]role`"
                ),
            });
        }
    }

    Ok(())
}

fn insert_extra(
    ctx: &mut BTreeMap<String, minijinja::Value>,
    resolved_extra: &ResolvedExtra,
    style: &Arc<Style>,
) {
    let rainbow: Vec<minijinja::Value> = resolved_extra
        .rainbow
        .iter()
        .map(|r| {
            let color = Color::role(
                r.hex.clone(),
                r.swatch.clone(),
                r.ascii.clone(),
                r.rgb,
                Arc::clone(style),
            );

            minijinja::Value::from_object(color)
        })
        .collect();

    ctx.insert("rainbow".to_owned(), minijinja::Value::from(rainbow));
}

fn insert_current_swatch(
    ctx: &mut BTreeMap<String, minijinja::Value>,
    scheme: &Scheme,
    swatch_name: &str,
    swatch_roles: &IndexMap<String, Vec<String>>,
    style: &Arc<Style>,
) -> Result<()> {
    let swatch = scheme.palette.get(swatch_name).ok_or_else(|| {
        crate::Error::InternalBug {
            module: "schemes",
            reason: format!(
                "current swatch `{swatch_name}` not in palette, but we should \
                 only be receiving valid swatch names"
            ),
        }
    })?;

    let roles = swatch_roles.get(swatch_name).cloned().unwrap_or_default();

    let obj = Color::swatch(
        swatch.hex().to_string(),
        swatch.name.to_string(),
        swatch.ascii.to_string(),
        swatch.rgb(),
        roles,
        Arc::clone(style),
    );

    ctx.insert("swatch".to_owned(), minijinja::Value::from_object(obj));

    Ok(())
}

fn insert_set_test_roles(
    ctx: &mut BTreeMap<String, minijinja::Value>,
    scheme: &Scheme,
) {
    let mut set_roles: Vec<String> =
        scheme.roles.set_roles().map(ToString::to_string).collect();

    if let Some(r) = &scheme.resolved_extra
        && !r.rainbow.is_empty()
    {
        set_roles.push("rainbow".to_owned());
    }

    ctx.insert("_set".to_owned(), minijinja::Value::from(set_roles));
}

fn insert_special(
    ctx: &mut BTreeMap<String, minijinja::Value>,
    special: &Special,
) {
    let mut special_map = BTreeMap::new();

    special_map.insert(
        "upstream_file".to_owned(),
        minijinja::Value::from(special.upstream_file.as_deref().unwrap_or("")),
    );

    special_map.insert(
        "upstream_repo".to_owned(),
        minijinja::Value::from(special.upstream_repo.as_deref().unwrap_or("")),
    );

    ctx.insert("special".to_owned(), minijinja::Value::from(special_map));
}
