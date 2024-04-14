use std::path::PathBuf;

use clap::{
    crate_authors, crate_description, crate_name, value_parser, Arg,
    ArgMatches, Command,
};
use tracing::Level;

pub fn parse() -> ArgMatches {
    let log_level = Arg::new("log_level")
        .short('l')
        .long("log-level")
        .value_parser(value_parser!(Level))
        .global(true)
        .help("Set the log level for stdout");

    let config_file = Arg::new("config_file")
        .short('c')
        .long("config-file")
        .value_parser(value_parser!(PathBuf))
        .global(true)
        .default_value("$XDG_CONFIG_HOME/projectfinder.toml");

    let init_command = Command::new("init")
        .about("Initialize a new config file in the current directory");

    // Manage workspaces
    let create_workspace = Command::new("create")
        .aliases(["c"])
        .about("create a new workspace")
        .args([Arg::new("name")
            .help("Name of the workspace to be created")
            .required(true)]);
    let remove_workspace = Command::new("remove")
        .aliases(["rm", "r"])
        .about("remove a workspace")
        .args([Arg::new("name")
            .help("Name of the workspace to be deleted")
            .required(true)]);
    // The workspace commands span
    let workspace_span = Command::new("workspace")
        .aliases(["ws", "w"])
        .about("Manage your workspaces")
        .subcommands([create_workspace, remove_workspace])
        .subcommand_required(true);

    let add_directory = Command::new("add")
        .aliases(["a"])
        .about("add a directory to a workspace")
        .args([
            Arg::new("name").help("Name of a workspace").required(true),
            Arg::new("project_dir")
                .help("Path to a directory containing projects")
                .value_parser(value_parser!(PathBuf))
                .required(true),
        ]);
    let remove_directory = Command::new("remove")
        .aliases(["rm", "r"])
        .about("remove a directory from a workspace")
        .args([
            Arg::new("name").help("Name of a workspace").required(true),
            Arg::new("project_dir")
                .help("Path to a directory containing projects")
                .value_parser(value_parser!(PathBuf))
                .required(true),
        ]);
    // The workspace directory commands span
    let directory_span = Command::new("directory")
        .aliases(["dir", "d"])
        .about("Manage your workspaces directories")
        .subcommands([add_directory, remove_directory])
        .subcommand_required(true);

    // Search
    let search = Command::new("search")
        .aliases(["s"])
        .about("Search a workspace; Run without arguments to infer a workspace")
        .args([Arg::new("name")]);

    Command::new(crate_name!())
        .about(crate_description!())
        .author(crate_authors!())
        .subcommands([init_command, workspace_span, directory_span, search])
        .args([log_level, config_file])
        .get_matches()
}
