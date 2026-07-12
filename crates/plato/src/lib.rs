use anyhow::{Result, bail};
use std::env::current_dir;
use std::path::{Component, Path, PathBuf};

pub(crate) mod config;
pub(crate) mod context;
pub(crate) mod fs;
pub(crate) mod names;
pub mod plugins;
pub(crate) mod process;
pub(crate) mod rendering;
pub(crate) mod setup;
pub(crate) mod source;
pub(crate) mod target;
pub(crate) mod util;
pub(crate) mod workspace;

use crate::config::Config;
use crate::config::group::apply_group_configs;
use crate::context::{ContextMap, ContextOverrides};
use crate::plugins::discovery::load_global_config;
use crate::setup::plan::SetupPlan;
use crate::setup::preflight::preflight_setup_plan;
use crate::setup::runner::{SetupRunnerContext, run_setup_plan};
use crate::source::TemplateRequest;
use crate::source::TemplateResolver;
use crate::source::git::TempCheckout;
pub use crate::target::ExistingTargetPolicy;

use crate::target::{TargetCommitOutcome, TargetTransaction};
use crate::util::open_config_file;
use crate::workspace::{WorkspaceRenderContext, render_workspace};

#[derive(Clone, Debug, Default)]
pub struct GitOptions {
    pub revision: Option<String>,
    pub subpath: Option<PathBuf>,
}

impl GitOptions {
    pub fn is_empty(&self) -> bool {
        self.revision.is_none() && self.subpath.is_none()
    }
}

#[derive(Clone, Debug)]
pub enum TemplateSource {
    Named {
        name: String,
        git_options: GitOptions,
    },
    Git {
        spec: String,
        options: GitOptions,
    },
    Path {
        path: PathBuf,
    },
}

#[derive(Clone, Debug, Default)]
pub struct ContextOverrideOptions {
    pub inferred: Vec<String>,
    pub strings: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct TemplateOptions {
    pub project_name: String,
    pub source: TemplateSource,
    pub groups: Vec<String>,
    pub context_overrides: ContextOverrideOptions,
}

#[derive(Clone, Debug)]
pub struct RunOptions {
    pub template: TemplateOptions,
    pub existing_target: ExistingTargetPolicy,
}

#[derive(Clone, Debug)]
pub struct ValidateOptions {
    pub template: TemplateOptions,
}

#[derive(Clone, Debug)]
pub struct RunSummary {
    pub target_path: PathBuf,
    pub files_written: usize,
    pub directories_created: usize,
    pub setup_steps_completed: usize,
    pub replaced_existing_target: bool,
}

#[derive(Clone, Debug)]
pub struct PluginInstallOptions {
    pub name: String,
    pub backend: plugins::install::PluginInstallBackend,
}

#[derive(Clone, Debug)]
pub struct PluginRegisterOptions {
    pub name: String,
    pub command: PathBuf,
}

struct PreparedTemplateContext {
    project_name: String,
    source_path: PathBuf,
    config: Config,
    context_overrides: ContextMap,
    source_cleanup: Option<TempCheckout>,
}

struct ExecutionContext {
    project_name: String,
    existing_target: ExistingTargetPolicy,
    source_path: PathBuf,
    target_path: PathBuf,
    config: Config,
    context_overrides: ContextMap,
    _source_cleanup: Option<TempCheckout>,
}

impl TryFrom<RunOptions> for ExecutionContext {
    type Error = anyhow::Error;

    fn try_from(options: RunOptions) -> Result<Self, Self::Error> {
        let prepared = prepare_template_context(options.template)?;
        let target_path = target_path_for_project(&prepared.project_name)?;
        Ok(Self {
            project_name: prepared.project_name,
            existing_target: options.existing_target,
            source_path: prepared.source_path,
            target_path,
            config: prepared.config,
            context_overrides: prepared.context_overrides,
            _source_cleanup: prepared.source_cleanup,
        })
    }
}

fn target_path_for_project(project_name: &str) -> Result<PathBuf> {
    let mut components = Path::new(project_name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        bail!("Project name {project_name:?} must be a single relative directory name");
    }
    Ok(current_dir()?.join(project_name))
}

fn validate_setup_sources(
    setup_plan: &SetupPlan,
    rendered: &workspace::rendered::RenderedWorkspace,
) -> Result<()> {
    for step in &setup_plan.steps {
        if !rendered.contains_directory(&step.source_path) {
            bail!(
                "Setup step for plugin {} uses source_path {}, but that directory is not rendered",
                step.plugin,
                step.source_path.display()
            );
        }
    }
    Ok(())
}

fn prepare_template_context(options: TemplateOptions) -> Result<PreparedTemplateContext> {
    let TemplateOptions {
        project_name,
        source,
        groups,
        context_overrides,
    } = options;
    let resolver = TemplateResolver::from_global_config()?;
    let prepared_source = match source {
        TemplateSource::Path { path } => resolver.prepare(TemplateRequest::Path { path })?,
        TemplateSource::Git { spec, options } => resolver.prepare(TemplateRequest::Git {
            spec,
            cli_rev: options.revision,
            cli_subpath: options.subpath,
        })?,
        TemplateSource::Named { name, git_options } => {
            resolver.prepare(TemplateRequest::Named {
                name,
                cli_rev: git_options.revision,
                cli_subpath: git_options.subpath,
            })?
        }
    };

    let mut config = prepared_source.config;
    apply_group_configs(&mut config, &prepared_source.source_path, &groups)?;
    let context_overrides =
        ContextOverrides::parse(&context_overrides.inferred, &context_overrides.strings)?
            .into_values();

    Ok(PreparedTemplateContext {
        project_name,
        source_path: prepared_source.source_path,
        config,
        context_overrides,
        source_cleanup: prepared_source.cleanup,
    })
}

/// Opens the selected template config in the user's editor.
///
/// # Errors
/// Returns an error if the global config cannot be loaded, the template cannot be found,
/// or the editor cannot be started successfully.
pub fn edit_config(template_name: &str) -> Result<()> {
    let resolver = TemplateResolver::from_global_config()?;
    let config_path = resolver.config_path_for(template_name)?;
    open_config_file(&config_path)
}

/// Displays all configured templates.
///
/// # Errors
/// Returns an error if the global config cannot be loaded or the template registry cannot be built.
pub fn display_templates(verbose: bool) -> Result<()> {
    let resolver = TemplateResolver::from_global_config()?;
    let output = resolver.format_templates(verbose);
    if output.is_empty() {
        println!(
            "No templates configured. Add entries under [templates] in ~/.config/plato/config.toml."
        );
    } else {
        print!("{output}");
    }
    Ok(())
}

/// Run the CLI.
///
/// # Errors
/// Returns an error if argument parsing, template loading, filesystem access,
/// template rendering, or project setup fails.
pub fn run(options: RunOptions) -> Result<RunSummary> {
    let exec_ctx = ExecutionContext::try_from(options)?;
    let render_ctx = WorkspaceRenderContext::from(&exec_ctx);
    let rendered = render_workspace(&render_ctx)?;
    let setup_plan = SetupPlan::from_config(&exec_ctx.config, &exec_ctx.target_path)?;
    validate_setup_sources(&setup_plan, &rendered)?;
    let global_config = load_global_config()?;
    let resolved_setup = preflight_setup_plan(&global_config, &setup_plan)?;

    let mut target =
        TargetTransaction::begin(exec_ctx.target_path.clone(), exec_ctx.existing_target)?;
    let initialization = (|| {
        let write_summary = rendered.write_to(target.path())?;
        run_setup_plan(
            &resolved_setup,
            &SetupRunnerContext {
                project_name: exec_ctx.project_name.clone(),
                target_path: target.path().to_path_buf(),
                template_path: exec_ctx.source_path.clone(),
                template_context: render_ctx.template_context,
                dry_run: false,
                verbose: false,
            },
        )?;
        Ok(write_summary)
    })();
    let write_summary = match initialization {
        Ok(summary) => summary,
        Err(error) => return Err(target.rollback_error(error)),
    };
    let target_outcome = target.commit()?;
    Ok(RunSummary {
        target_path: exec_ctx.target_path,
        files_written: write_summary.files_written,
        directories_created: write_summary.directories_created,
        setup_steps_completed: resolved_setup.steps.len(),
        replaced_existing_target: matches!(target_outcome, TargetCommitOutcome::Replaced),
    })
}

/// Validate a template without writing a project or running setup commands.
///
/// # Errors
/// Returns an error if source resolution, rendering, or setup-plan validation fails.
pub fn validate(options: ValidateOptions) -> Result<()> {
    let prepared = prepare_template_context(options.template)?;
    let render_ctx = WorkspaceRenderContext::from(&prepared);
    let rendered = render_workspace(&render_ctx)?;
    let target_path = target_path_for_project(&prepared.project_name)?;
    let setup_plan = SetupPlan::from_config(&prepared.config, &target_path)?;
    validate_setup_sources(&setup_plan, &rendered)?;
    println!("Validation passed.");
    Ok(())
}

/// Installs a setup plugin.
///
/// # Errors
/// Returns an error if plugin installation fails for the selected backend.
pub fn install_plugin(options: PluginInstallOptions) -> Result<()> {
    let PluginInstallOptions { name, backend } = options;
    plugins::install::install_plugin(&name, backend)
}

/// Registers an explicit plugin executable in global config.
///
/// # Errors
/// Returns an error if global config cannot be updated or the plugin name is invalid.
pub fn register_plugin(options: PluginRegisterOptions) -> Result<()> {
    let PluginRegisterOptions { name, command } = options;
    plugins::registry::register_plugin(&name, &command)
}

/// Removes an explicit plugin registry entry from global config.
///
/// # Errors
/// Returns an error if global config cannot be updated or the plugin name is invalid.
pub fn remove_plugin(name: &str) -> Result<()> {
    plugins::registry::remove_plugin(name)
}

/// Displays discovered and registered plugins.
///
/// # Errors
/// Returns an error if global config or the managed plugin directory cannot be read.
pub fn display_plugins() -> Result<()> {
    let global_config = load_global_config()?;
    let mut entries = Vec::new();
    for (name, entry) in &global_config.plugin_registry {
        let source = entry.source.as_deref().unwrap_or("manual");
        entries.push((
            name.clone(),
            0_u8,
            format!("registry:{source}"),
            entry.command.display().to_string(),
        ));
    }
    let managed_dir = plugins::paths::managed_plugin_bin_dir()?;
    if managed_dir.exists() {
        for entry in std::fs::read_dir(&managed_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("plato-plugin-") {
                entries.push((
                    plugin_name_from_executable(&name),
                    1,
                    "managed".to_string(),
                    entry.path().display().to_string(),
                ));
            }
        }
    }
    for path in plugins::discovery::discover_path_plugins() {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        entries.push((
            plugin_name_from_executable(name),
            2,
            "path".to_string(),
            path.display().to_string(),
        ));
    }
    entries.sort();
    entries.dedup();
    if entries.is_empty() {
        println!("No plugins discovered. Install one with: plato plugin install <name>");
        return Ok(());
    }
    let mut effective = std::collections::BTreeSet::new();
    for (name, _, source, command) in entries {
        let marker = if effective.insert(name.clone()) {
            "effective"
        } else {
            "shadowed"
        };
        println!("{name}\t{marker}\t{source}\t{command}");
    }
    Ok(())
}

fn plugin_name_from_executable(file_name: &str) -> String {
    let name = file_name.trim_start_matches("plato-plugin-");
    name.strip_suffix(std::env::consts::EXE_SUFFIX)
        .unwrap_or(name)
        .to_string()
}

impl From<&PreparedTemplateContext> for WorkspaceRenderContext {
    fn from(ctx: &PreparedTemplateContext) -> Self {
        Self {
            template_context: workspace::build_template_context_parts(
                &ctx.project_name,
                &ctx.config,
                ctx.context_overrides.clone(),
            ),
            path_replacements: ctx.config.path.replace.clone(),
            path_excludes: ctx.config.path.exclude.clone(),
            source_path: ctx.source_path.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_project_names_that_escape_the_current_directory() {
        assert!(target_path_for_project("../outside").is_err());
        assert!(target_path_for_project("nested/project").is_err());
        assert!(target_path_for_project("/").is_err());
        assert!(target_path_for_project("project").is_ok());
    }
}
