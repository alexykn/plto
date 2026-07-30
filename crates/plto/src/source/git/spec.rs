use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

use crate::config::{GitProvider, GlobalConfig};
use crate::fs::path::reject_parent_components;

#[derive(Debug, Clone)]
pub(crate) struct GitTemplateSpec {
    pub(crate) url: String,
    pub(crate) revision: Option<String>,
    pub(crate) subpath: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub(crate) struct GitSpecParts {
    pub(crate) url: String,
    pub(crate) inline_revision: Option<String>,
}

pub(crate) fn parse_git_spec_parts(raw: &str, config: &GlobalConfig) -> Result<GitSpecParts> {
    let (spec, inline_revision) = split_inline_revision(raw)?;

    if is_rejected_local_spec(spec) {
        bail!("Local Git repository specs are not supported. Use --path for local templates.");
    }

    if is_url(spec) {
        reject_embedded_credentials(spec)?;
        validate_url_shape(spec)?;
        return Ok(GitSpecParts {
            url: spec.to_string(),
            inline_revision,
        });
    }

    if let Some((provider, path)) = split_provider_shorthand(spec) {
        validate_remote_path(path)?;
        return Ok(GitSpecParts {
            url: build_provider_url(provider, path, config),
            inline_revision,
        });
    }

    if spec.starts_with("git@") {
        validate_scp_like(spec)?;
        return Ok(GitSpecParts {
            url: spec.to_string(),
            inline_revision,
        });
    }

    if let Some((host, path)) = split_host_shorthand(spec) {
        validate_host(host)?;
        validate_remote_path(path)?;
        let url = build_host_url(host, path);
        eprintln!("WARNING: Interpreting {spec:?} as SSH remote {url:?} by adding git@.");
        return Ok(GitSpecParts {
            url,
            inline_revision,
        });
    }

    if spec.contains('/') {
        validate_remote_path(spec)?;
        return Ok(GitSpecParts {
            url: build_provider_url(config.plto.default_git_provider, spec, config),
            inline_revision,
        });
    }

    bail!("Git template spec {raw:?} is not a supported Git remote syntax")
}

pub(crate) fn merge_git_template_spec(
    raw: &str,
    config: &GlobalConfig,
    configured_rev: Option<&str>,
    configured_subpath: Option<&Path>,
    cli_rev: Option<&str>,
    cli_subpath: Option<&Path>,
) -> Result<GitTemplateSpec> {
    let parts = parse_git_spec_parts(raw, config)?;

    Ok(GitTemplateSpec {
        url: parts.url,
        revision: cli_rev
            .map(str::to_string)
            .or(parts.inline_revision)
            .or_else(|| configured_rev.map(str::to_string)),
        subpath: cli_subpath
            .map(Path::to_path_buf)
            .or_else(|| configured_subpath.map(Path::to_path_buf)),
    })
}

fn split_inline_revision(raw: &str) -> Result<(&str, Option<String>)> {
    let Some((spec, revision)) = raw.rsplit_once('#') else {
        return Ok((raw, None));
    };
    if revision.is_empty() {
        bail!("Git template revision after '#' must not be empty");
    }
    Ok((spec, Some(revision.to_string())))
}

fn is_url(spec: &str) -> bool {
    spec.starts_with("https://") || spec.starts_with("http://") || spec.starts_with("ssh://")
}

fn is_rejected_local_spec(spec: &str) -> bool {
    spec.starts_with("file://")
        || spec.starts_with('/')
        || spec.starts_with("./")
        || spec.starts_with("../")
}

fn split_provider_shorthand(spec: &str) -> Option<(GitProvider, &str)> {
    let (provider, path) = spec.split_once(':')?;
    let provider = match provider {
        "github" => GitProvider::Github,
        "gitlab" => GitProvider::Gitlab,
        "bitbucket" => GitProvider::Bitbucket,
        _ => return None,
    };
    Some((provider, path))
}

fn split_host_shorthand(spec: &str) -> Option<(&str, &str)> {
    let (host, path) = spec.split_once(':')?;
    if host.is_empty() || path.is_empty() || !path.contains('/') {
        return None;
    }
    Some((host, path))
}

fn validate_url_shape(url: &str) -> Result<()> {
    if url.starts_with("ssh://") || url.starts_with("http://") || url.starts_with("https://") {
        return Ok(());
    }
    bail!("Unsupported Git URL syntax")
}

fn reject_embedded_credentials(url: &str) -> Result<()> {
    let Some((scheme, rest)) = url.split_once("://") else {
        return Ok(());
    };
    if !matches!(scheme, "http" | "https" | "ssh") {
        return Ok(());
    }
    let authority = rest.split('/').next().unwrap_or_default();
    let Some(userinfo) = authority.split_once('@').map(|(userinfo, _)| userinfo) else {
        return Ok(());
    };
    if userinfo.contains(':') || scheme != "ssh" {
        bail!(
            "Git template URLs must not contain embedded credentials. Use SSH remotes or a Git credential helper instead."
        );
    }
    Ok(())
}

fn validate_scp_like(spec: &str) -> Result<()> {
    let Some(rest) = spec.strip_prefix("git@") else {
        bail!("SSH Git remotes must start with git@");
    };
    let Some((host, path)) = rest.split_once(':') else {
        bail!("SSH Git remote must use git@host:path syntax");
    };
    validate_host(host)?;
    validate_remote_path(path)
}

fn validate_host(host: &str) -> Result<()> {
    if host.trim().is_empty() || host.contains('/') || host.contains('@') || host.contains(':') {
        bail!("Invalid Git host shorthand host {host:?}");
    }
    Ok(())
}

fn validate_remote_path(path: &str) -> Result<()> {
    let path = Path::new(path);
    reject_parent_components(path, "Git remote path")?;
    if path.components().count() < 2 {
        bail!(
            "Git remote shorthand must include owner and repository, got {:?}",
            path.display().to_string()
        );
    }
    Ok(())
}

fn build_provider_url(provider: GitProvider, path: &str, config: &GlobalConfig) -> String {
    let host = config
        .plto
        .git_hosts
        .get(provider)
        .unwrap_or_else(|| public_host(provider));
    build_host_url(host, path)
}

fn build_host_url(host: &str, path: &str) -> String {
    if Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("git"))
    {
        return format!("git@{host}:{path}");
    }
    format!("git@{host}:{path}.git")
}

fn public_host(provider: GitProvider) -> &'static str {
    match provider {
        GitProvider::Github => "github.com",
        GitProvider::Gitlab => "gitlab.com",
        GitProvider::Bitbucket => "bitbucket.org",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_provider_shorthand() {
        let config = GlobalConfig::default();
        let spec = parse_git_spec_parts("owner/repo", &config).unwrap();
        assert_eq!(spec.url, "git@github.com:owner/repo.git");
    }

    #[test]
    fn parses_inline_revision() {
        let config = GlobalConfig::default();
        let spec = parse_git_spec_parts("gitlab:owner/repo#v1", &config).unwrap();
        assert_eq!(spec.url, "git@gitlab.com:owner/repo.git");
        assert_eq!(spec.inline_revision.as_deref(), Some("v1"));
    }

    #[test]
    fn rejects_empty_inline_revision() {
        let config = GlobalConfig::default();
        let error = parse_git_spec_parts("gitlab:owner/repo#", &config).unwrap_err();
        assert!(error.to_string().contains("must not be empty"));
    }

    #[test]
    fn supports_nested_gitlab_groups() {
        let config = GlobalConfig::default();
        let spec = parse_git_spec_parts("gitlab:owner/group/repo", &config).unwrap();
        assert_eq!(spec.url, "git@gitlab.com:owner/group/repo.git");
    }

    #[test]
    fn supports_host_shorthand() {
        let config = GlobalConfig::default();
        let spec = parse_git_spec_parts("gitlab.company.test:owner/group/repo", &config).unwrap();
        assert_eq!(spec.url, "git@gitlab.company.test:owner/group/repo.git");
    }

    #[test]
    fn supports_scp_like_ssh() {
        let config = GlobalConfig::default();
        let spec =
            parse_git_spec_parts("git@gitlab.company.test:owner/group/repo.git", &config).unwrap();
        assert_eq!(spec.url, "git@gitlab.company.test:owner/group/repo.git");
    }

    #[test]
    fn rejects_embedded_http_credentials_without_leaking_url() {
        let config = GlobalConfig::default();
        let error = parse_git_spec_parts("https://user:secret@example.com/org/repo.git", &config)
            .unwrap_err();
        assert!(error.to_string().contains("embedded credentials"));
        assert!(!error.to_string().contains("secret"));
    }

    #[test]
    fn rejects_embedded_ssh_password() {
        let config = GlobalConfig::default();
        let error = parse_git_spec_parts("ssh://user:secret@example.com/org/repo.git", &config)
            .unwrap_err();
        assert!(error.to_string().contains("embedded credentials"));
    }

    #[test]
    fn allows_ssh_user_without_password() {
        let config = GlobalConfig::default();
        let spec = parse_git_spec_parts("ssh://git@example.com/org/repo.git", &config).unwrap();
        assert_eq!(spec.url, "ssh://git@example.com/org/repo.git");
    }

    #[test]
    fn rejects_parent_component_but_allows_dotdot_name() {
        let config = GlobalConfig::default();
        parse_git_spec_parts("github:foo..bar/repo", &config).unwrap();
        let error = parse_git_spec_parts("github:owner/../repo", &config).unwrap_err();
        assert!(error.to_string().contains("'..'"));
    }

    #[test]
    fn cli_revision_overrides_inline_and_config() {
        let config = GlobalConfig::default();
        let spec = merge_git_template_spec(
            "owner/repo#inline",
            &config,
            Some("configured"),
            None,
            Some("cli"),
            None,
        )
        .unwrap();
        assert_eq!(spec.revision.as_deref(), Some("cli"));
    }
}
