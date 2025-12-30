use std::path::Path;

use indexmap::{IndexMap, IndexSet};

use crate::config::Provider;
use crate::themes::{Extra, Meta, Palette, RawScheme, Roles, Swatch};


pub(crate) trait Merge: Sized {
    fn merge(self, base: Self) -> Self;
}

macro_rules! impl_merge_for_all_fields {
    ($type:ty { $($field:ident),+ $(,)? }) => {
        impl Merge for $type {
            fn merge(self, base: Self) -> Self {
                Self {
                    $(
                        $field: self.$field.merge(base.$field),
                    )+
                }
            }
        }
    }
}

impl<T> Merge for Option<T> {
    fn merge(self, base: Self) -> Self {
        self.or(base)
    }
}

impl_merge_for_all_fields!(Meta {
    author,
    author_ascii,
    license,
    license_ascii,
    blurb,
    blurb_ascii,
});

impl Merge for Extra {
    fn merge(self, base: Self) -> Self {
        Self {
            rainbow: if self.rainbow.is_empty() {
                base.rainbow
            } else {
                self.rainbow
            },
        }
    }
}

impl Merge for Palette {
    fn merge(self, base: Self) -> Self {
        let mut palette = base.0;

        for swatch in self.0 {
            palette.replace(swatch);
        }

        Self(palette)
    }
}

impl Merge for Roles {
    fn merge(self, mut base: Self) -> Self {
        base.0.extend(self.0);

        base
    }
}

impl_merge_for_all_fields!(RawScheme {
    scheme,
    scheme_ascii,
    meta,
    palette,
    roles,
    extra,
});

impl Merge for Provider {
    fn merge(self, base: Self) -> Self {
        Self {
            host: self.host,
            blob_path: self.blob_path.merge(base.blob_path),
            raw_path: self.raw_path.merge(base.raw_path),
            branch: self.branch.merge(base.branch),
        }
    }
}


pub(crate) trait PathExt {
    fn has_extension(&self, ext: &str) -> bool;
    fn is_toml(&self) -> bool;
    fn is_jinja(&self) -> bool;
}

impl PathExt for Path {
    fn has_extension(&self, ext: &str) -> bool {
        self.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case(ext))
    }

    fn is_toml(&self) -> bool {
        self.has_extension("toml")
    }

    fn is_jinja(&self) -> bool {
        self.has_extension("jinja")
    }
}
