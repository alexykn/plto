use anyhow::{Result, anyhow};

use plato::{ContextOverrideOptions, GitOptions, TemplateOptions, TemplateSource};

use crate::cli::args::TemplateSourceArgs;

const DEFAULT_VALIDATION_PROJECT_NAME: &str = "plato-validation";

pub(crate) fn map_init_source_args(
    args: TemplateSourceArgs,
    groups: Vec<String>,
    inferred: Vec<String>,
    strings: Vec<String>,
) -> Result<TemplateOptions> {
    let (source, project_name) = map_source(args, false)?;
    Ok(template_options(
        source,
        project_name,
        groups,
        inferred,
        strings,
    ))
}

pub(crate) fn map_validate_source_args(
    args: TemplateSourceArgs,
    groups: Vec<String>,
    inferred: Vec<String>,
    strings: Vec<String>,
) -> Result<TemplateOptions> {
    let (source, project_name) = map_source(args, true)?;
    Ok(template_options(
        source,
        project_name,
        groups,
        inferred,
        strings,
    ))
}

fn map_source(args: TemplateSourceArgs, validation: bool) -> Result<(TemplateSource, String)> {
    let TemplateSourceArgs {
        template_name,
        project_name,
        path,
        git,
        rev,
        subpath,
    } = args;

    let git_options = GitOptions {
        revision: rev,
        subpath,
    };
    if let Some(path) = path {
        return map_path_source(path, template_name, project_name, validation);
    }

    let name = template_name
        .ok_or_else(|| anyhow!("Without --path, pass a template name and project name."))?;
    let project_name = if validation {
        project_name.unwrap_or_else(|| DEFAULT_VALIDATION_PROJECT_NAME.to_string())
    } else {
        project_name
            .ok_or_else(|| anyhow!("Without --path, pass a template name and project name."))?
    };

    if git {
        return Ok((
            TemplateSource::Git {
                spec: name,
                options: git_options,
            },
            project_name,
        ));
    }
    Ok((TemplateSource::Named { name, git_options }, project_name))
}

fn map_path_source(
    path: std::path::PathBuf,
    first: Option<String>,
    second: Option<String>,
    validation: bool,
) -> Result<(TemplateSource, String)> {
    let project_name = match (first, second, validation) {
        (None, None, true) => DEFAULT_VALIDATION_PROJECT_NAME.to_string(),
        (None, None, false) => {
            return Err(anyhow!("With --path, pass exactly one project name."));
        }
        (Some(_), Some(_), _) => {
            return Err(anyhow!("With --path, pass at most one project name."));
        }
        (Some(name), None, _) | (None, Some(name), true) => name,
        (None, Some(_), false) => unreachable!("second positional without first is impossible"),
    };
    Ok((TemplateSource::Path { path }, project_name))
}

fn template_options(
    source: TemplateSource,
    project_name: String,
    groups: Vec<String>,
    inferred: Vec<String>,
    strings: Vec<String>,
) -> TemplateOptions {
    TemplateOptions {
        project_name,
        source,
        groups,
        context_overrides: ContextOverrideOptions { inferred, strings },
    }
}
