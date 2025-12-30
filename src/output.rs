pub(crate) mod format;
pub(crate) mod strategy;
pub(crate) mod style;
pub(crate) mod upstream;

pub(crate) use self::format::format;
pub(crate) use self::strategy::{Decision, FileStatus, Write as WriteMode};
pub(crate) use self::style::{Ascii, ColorStyle, Style, TextStyle, Unicode};
pub(crate) use self::upstream::{Error as UpstreamError, Special, Upstream};
