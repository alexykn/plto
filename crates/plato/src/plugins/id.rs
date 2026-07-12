use anyhow::{Result, bail};
use std::ffi::OsString;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct PluginId(String);

impl PluginId {
    pub(crate) fn parse(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.is_empty()
            || !value
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
        {
            bail!(
                "Invalid plugin name {value:?}: plugin names may contain only ASCII letters, numbers, '_' and '-'"
            );
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn crate_name(&self) -> String {
        format!("plato-plugin-{}", self.0)
    }
}

pub(crate) fn executable_names(base: &str) -> Vec<OsString> {
    #[cfg(windows)]
    {
        let extensions =
            std::env::var_os("PATHEXT").unwrap_or_else(|| OsString::from(".COM;.EXE;.BAT;.CMD"));
        let mut names = extensions
            .to_string_lossy()
            .split(';')
            .filter(|extension| !extension.is_empty())
            .map(|extension| OsString::from(format!("{base}{extension}")))
            .collect::<Vec<_>>();
        names.push(OsString::from(base));
        return names;
    }

    #[cfg(not(windows))]
    vec![OsString::from(base)]
}

impl Display for PluginId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
