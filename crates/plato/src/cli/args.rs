use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "plto")]
#[command(about = "Scaffolds projects from ~/.config/plato", long_about = None)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    /// Initialize new project from a configured template, --git spec, or --path
    Init(InitArgs),
    /// Validate a template in memory without creating a project or running setup
    Val(ValArgs),
    /// Open a configured template override or local plato.toml in editor
    Config {
        /// The name of the template (e.g., py3.12)
        template_name: String,
    },
    /// List configured templates
    List {
        /// Show source details for every configured template
        #[arg(short, long)]
        verbose: bool,
    },
    /// Manage setup plugins
    Plugin {
        #[command(subcommand)]
        command: PluginCommands,
    },
}

#[derive(Subcommand, Debug)]
pub(crate) enum PluginCommands {
    /// List discovered plugins
    List,
    /// Install a plugin using cargo by default
    Install(PluginInstallArgs),
    /// Register an explicit plugin executable path in global config
    Register {
        /// Plugin name, e.g. uv for plto-plugin-uv
        name: String,
        /// Path to the plugin executable
        #[arg(long)]
        command: PathBuf,
    },
    /// Remove an explicit plugin registry entry from global config
    Remove {
        /// Plugin name to remove from the registry
        name: String,
    },
}

#[derive(Args, Debug)]
pub(crate) struct PluginInstallArgs {
    /// Plugin name, e.g. uv installs the cargo crate plto-plugin-uv
    pub(crate) name: String,

    /// Install with cargo from crates.io
    #[arg(long)]
    pub(crate) cargo: bool,

    /// Install a cargo plugin from a git repository
    #[arg(long)]
    pub(crate) git: Option<String>,

    /// Install a plugin from a local package/crate path
    #[arg(long)]
    pub(crate) path: Option<PathBuf>,

    /// Install a Python plugin with uv tool install
    #[arg(long = "uv-tool")]
    pub(crate) uv_tool: bool,

    /// Install a Python plugin with pipx install
    #[arg(long)]
    pub(crate) pipx: bool,
}

#[derive(Args, Debug)]
pub(crate) struct TemplateSourceArgs {
    /// Configured template name, or Git spec when --git is passed
    pub(crate) template_name: Option<String>,

    /// The project directory name; when --path is used, supply this as the first positional
    pub(crate) project_name: Option<String>,

    /// Provide an explicit path to load the template from
    #[arg(short, long, conflicts_with_all = ["git", "rev", "subpath"])]
    pub(crate) path: Option<PathBuf>,

    /// Treat the template argument as an ad-hoc Git remote spec
    #[arg(long, conflicts_with = "path")]
    pub(crate) git: bool,

    /// Git branch, tag, or commit to use for remote templates
    #[arg(long, conflicts_with = "path")]
    pub(crate) rev: Option<String>,

    /// Subpath inside a remote repository to use as the template root
    #[arg(long, conflicts_with = "path")]
    pub(crate) subpath: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub(crate) struct InitArgs {
    #[command(flatten)]
    pub(crate) source: TemplateSourceArgs,

    /// Replace an existing target directory transactionally
    #[arg(short, long)]
    pub(crate) force: bool,

    /// Apply an optional template group such as docker or ci
    #[arg(short = 'g', long = "group")]
    pub(crate) groups: Vec<String>,

    /// Override template context with inferred value typing: key=value
    #[arg(short = 's', long = "set")]
    pub(crate) set_values: Vec<String>,

    /// Override template context as a string: key=value
    #[arg(long = "set-string")]
    pub(crate) set_string_values: Vec<String>,
}

#[derive(Args, Debug)]
pub(crate) struct ValArgs {
    #[command(flatten)]
    pub(crate) source: TemplateSourceArgs,

    /// Apply an optional template group such as docker or ci
    #[arg(short = 'g', long = "group")]
    pub(crate) groups: Vec<String>,

    /// Override template context with inferred value typing: key=value
    #[arg(short = 's', long = "set")]
    pub(crate) set_values: Vec<String>,

    /// Override template context as a string: key=value
    #[arg(long = "set-string")]
    pub(crate) set_string_values: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_group_and_set_shorthands_for_init() {
        let cli = Cli::parse_from([
            "plato",
            "init",
            "py-api",
            "my-api",
            "-g",
            "docker",
            "-s",
            "port=8000",
            "--set-string",
            "version=1.0",
        ]);

        let Commands::Init(args) = cli.command else {
            panic!("expected init command");
        };
        assert_eq!(args.groups, ["docker"]);
        assert_eq!(args.set_values, ["port=8000"]);
        assert_eq!(args.set_string_values, ["version=1.0"]);
    }

    #[test]
    fn parses_group_and_set_shorthands_for_validation() {
        let cli = Cli::parse_from([
            "plato",
            "val",
            "py-api",
            "-g",
            "docker",
            "-s",
            "docker=true",
        ]);

        let Commands::Val(args) = cli.command else {
            panic!("expected val command");
        };
        assert_eq!(args.groups, ["docker"]);
        assert_eq!(args.set_values, ["docker=true"]);
    }
}
