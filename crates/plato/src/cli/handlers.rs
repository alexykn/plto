use plato::plugins::install::PluginInstallBackend;
use plato::{
    ExistingTargetPolicy, PluginInstallOptions, PluginRegisterOptions, RunOptions, ValidateOptions,
};

use crate::cli::args::{Commands, InitArgs, PluginCommands, PluginInstallArgs, ValArgs};
use crate::cli::mapping::{map_init_source_args, map_validate_source_args};

pub(crate) fn run_command(command: Commands) -> anyhow::Result<()> {
    match command {
        Commands::Init(args) => run_init(args),
        Commands::Val(args) => run_validate(args),
        Commands::Config { template_name } => plato::edit_config(&template_name),
        Commands::List { verbose } => plato::display_templates(verbose),
        Commands::Plugin { command } => run_plugin(command),
    }
}

fn run_init(args: InitArgs) -> anyhow::Result<()> {
    let template = map_init_source_args(
        args.source,
        args.groups,
        args.set_values,
        args.set_string_values,
    )?;

    let summary = plato::run(RunOptions {
        template,
        existing_target: if args.force {
            ExistingTargetPolicy::Replace
        } else {
            ExistingTargetPolicy::Reject
        },
    })?;
    let action = if summary.replaced_existing_target {
        "Replaced"
    } else {
        "Created"
    };
    println!("{action} project at {}", summary.target_path.display());
    println!(
        "Rendered {} files and {} directories; completed {} setup steps.",
        summary.files_written, summary.directories_created, summary.setup_steps_completed
    );
    Ok(())
}

fn run_validate(args: ValArgs) -> anyhow::Result<()> {
    let template = map_validate_source_args(
        args.source,
        args.groups,
        args.set_values,
        args.set_string_values,
    )?;

    plato::validate(ValidateOptions { template })
}

fn run_plugin(command: PluginCommands) -> anyhow::Result<()> {
    match command {
        PluginCommands::List => plato::display_plugins(),
        PluginCommands::Install(args) => run_plugin_install(args),
        PluginCommands::Register { name, command } => {
            plato::register_plugin(PluginRegisterOptions { name, command })
        }
        PluginCommands::Remove { name } => plato::remove_plugin(&name),
    }
}

fn run_plugin_install(args: PluginInstallArgs) -> anyhow::Result<()> {
    let name = args.name.clone();
    let backend = select_plugin_install_backend(args)?;
    plato::install_plugin(PluginInstallOptions { name, backend })
}

pub(crate) fn select_plugin_install_backend(
    args: PluginInstallArgs,
) -> anyhow::Result<PluginInstallBackend> {
    if args.git.is_some() && args.path.is_some() {
        anyhow::bail!("--git and --path cannot be used together for plugin install");
    }

    let backend_flags = usize::from(args.cargo)
        + usize::from(args.git.is_some())
        + usize::from(args.uv_tool)
        + usize::from(args.pipx);
    if backend_flags > 1 {
        anyhow::bail!(
            "Choose at most one plugin install backend: --cargo, --git, --uv-tool, or --pipx"
        );
    }

    if let Some(path) = args.path {
        if args.uv_tool {
            return Ok(PluginInstallBackend::UvToolPath { path });
        }
        if args.pipx {
            return Ok(PluginInstallBackend::PipxPath { path });
        }
        return Ok(PluginInstallBackend::CargoPath { path });
    }

    if let Some(url) = args.git {
        return Ok(PluginInstallBackend::Git { url });
    }
    if args.uv_tool {
        return Ok(PluginInstallBackend::UvTool);
    }
    if args.pipx {
        return Ok(PluginInstallBackend::Pipx);
    }
    Ok(PluginInstallBackend::Cargo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn install_args() -> PluginInstallArgs {
        PluginInstallArgs {
            name: "uv".to_string(),
            cargo: false,
            git: None,
            path: None,
            uv_tool: false,
            pipx: false,
        }
    }

    #[test]
    fn defaults_plugin_install_to_cargo() {
        assert!(matches!(
            select_plugin_install_backend(install_args()).unwrap(),
            PluginInstallBackend::Cargo
        ));
    }

    #[test]
    fn selects_path_backends() {
        let mut args = install_args();
        args.path = Some(PathBuf::from("plugin"));
        assert!(matches!(
            select_plugin_install_backend(args).unwrap(),
            PluginInstallBackend::CargoPath { .. }
        ));

        let mut args = install_args();
        args.path = Some(PathBuf::from("plugin"));
        args.uv_tool = true;
        assert!(matches!(
            select_plugin_install_backend(args).unwrap(),
            PluginInstallBackend::UvToolPath { .. }
        ));
    }

    #[test]
    fn rejects_conflicting_plugin_install_backends() {
        let mut args = install_args();
        args.git = Some("https://example.com/plugin.git".to_string());
        args.path = Some(PathBuf::from("plugin"));
        assert!(select_plugin_install_backend(args).is_err());

        let mut args = install_args();
        args.cargo = true;
        args.pipx = true;
        assert!(select_plugin_install_backend(args).is_err());
    }
}
