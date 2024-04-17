use std::{
    env, fs,
    io::{stdin, stdout},
    ops::Deref,
    path::{Path, PathBuf},
    sync::Mutex,
};

use clap::{crate_version, ArgMatches};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use termion::{event::Key, raw::IntoRawMode};
use toml::{to_string, toml};
use tracing::{error, Level};

mod cli;
mod error;
mod logger;
mod path_utils;
mod tui;

use error::Error;
use tui::Window;

lazy_static! {
    static ref CONFIG_FILE: Mutex<PathBuf> = Mutex::new(PathBuf::new());
}

/// TODO: Api version to be separate from crate version
/// TODO: Respect .gitignore when searching
/// TODO: Handle trying to add nested workspace directories and manually added nested workspace
/// directories
/// TODO: USE THE FUCKING PATH UTILS YOU HAVE PROGRAMMED!!!!
/// TODO: Add tracing you dumb fuck, you wasted time setting it up alrady
fn main() {
    let matches = cli::parse();

    logger::init(matches.get_one::<Level>("log_level").copied());
    if let Err(err) = _main(matches) {
        error!("{}", err);
    }
}
fn _main(matches: ArgMatches) -> Result<(), Error> {
    {
        let mut config = CONFIG_FILE.lock()?;
        *config = path_utils::resolve_path_variables(
            matches.get_one::<PathBuf>("config_file").cloned().ok_or(
                Error::UnhandledMissingArgument("config-file".to_string()),
            )?,
        )?;
    }

    match matches.subcommand() {
        Some(("init", _)) => init(),
        Some(("workspace", command)) => match command.subcommand() {
            Some(("create", command)) => {
                let name = command.get_one::<String>("name").cloned().ok_or(
                    Error::UnhandledMissingArgument("name".to_string()),
                )?;
                create_workspace(name)
            }
            Some(("remove", command)) => {
                let name = command.get_one::<String>("name").cloned().ok_or(
                    Error::UnhandledMissingArgument("name".to_string()),
                )?;
                remove_workspace(name)
            }
            Some((name, _)) => {
                return Err(Error::UnhandledAction(name.to_string()))
            }
            None => {
                return Err(Error::UnhandledAction("workspace".to_string()))
            }
        },
        Some(("directory", command)) => match command.subcommand() {
            Some(("add", command)) => {
                let name = command.get_one::<String>("name").cloned().ok_or(
                    Error::UnhandledMissingArgument("name".to_string()),
                )?;
                let project = command
                    .get_one::<PathBuf>("project_dir")
                    .cloned()
                    .ok_or(Error::UnhandledMissingArgument(
                        "project_dir".to_string(),
                    ))?;
                let project = fs::canonicalize(project)?;
                add_workspace_directory(name, project)
            }
            Some(("remove", command)) => {
                let name = command.get_one::<String>("name").cloned().ok_or(
                    Error::UnhandledMissingArgument("name".to_string()),
                )?;
                let project = command
                    .get_one::<PathBuf>("project_dir")
                    .cloned()
                    .ok_or(Error::UnhandledMissingArgument(
                        "project_dir".to_string(),
                    ))?;
                remove_workspace_directory(name, project)
            }

            Some((name, _)) => {
                return Err(Error::UnhandledAction(name.to_string()))
            }
            None => {
                return Err(Error::UnhandledAction("directory".to_string()))
            }
        },
        Some(("search", command)) => {
            let name = command.get_one::<String>("name");
            let name = match name {
                Some(name) => name.clone(),
                None => find_current_workspace()?,
            };

            let directories = search_workspace(name)?;
            fzf(directories)?;
            Ok(())
        }
        Some((name, _)) => {
            return Err(Error::UnhandledAction(name.to_string()))
        }
        None => {
            let name = find_current_workspace()?;
            let directories = search_workspace(name)?;
            fzf(directories)?;
            Ok(())
        }
    }
}

fn init() -> Result<(), Error> {
    let version = crate_version!();
    let content = toml! {
        [metadata]
        version = version
        [workspaces]
    };
    {
        fs::write(CONFIG_FILE.lock()?.deref(), content.to_string())?;
    }

    Ok(())
}

fn find_current_workspace() -> Result<String, Error> {
    let config_file = CONFIG_FILE.lock()?;
    let content = fs::read_to_string(config_file.deref())?;
    let config: Config = toml::from_str(&content)?;

    let current_path = env::current_dir()?;
    let ancestors = current_path.ancestors();
    for (name, workspace) in config.workspaces {
        let workspace = workspace
            .as_array()
            .ok_or(Error::InvalidWorkspace(name.clone()))?;
        for project in workspace {
            let project = PathBuf::from(
                project
                    .as_str()
                    .ok_or(Error::InvalidWorkspace(name.clone()))?,
            );
            for ancestor in ancestors {
                if ancestor == project {
                    return Ok(name);
                }
            }
        }
    }
    Err(Error::NotInWorkspace(current_path))
}

fn search_workspace(name: String) -> Result<Vec<PathBuf>, Error> {
    let config_file = CONFIG_FILE.lock()?;
    let content = fs::read_to_string(config_file.deref())?;
    let config: Config = toml::from_str(&content)?;
    let workspace = config
        .workspaces
        .get(&name)
        .ok_or(Error::UndefinedWorkspace(name.clone()))?
        .as_array()
        .ok_or(Error::InvalidWorkspace(name.clone()))?;

    let mut directories: Vec<PathBuf> = Vec::new();
    println!("{:?}", workspace);
    for directory in workspace {
        let directory: PathBuf = directory
            .as_str()
            .ok_or(Error::InvalidWorkspace(name.clone()))?
            .into();
        if !directory.is_absolute() {
            return Err(Error::RelativeDirectoryError(
                name.clone(),
                directory.clone(),
            ));
        }
        directories = search_directory(directory, directories)?;
    }
    return Ok(directories);
}

fn search_directory(
    directory: PathBuf,
    mut directories: Vec<PathBuf>,
) -> Result<Vec<PathBuf>, Error> {
    let git_dir = directory.join(".git");
    if git_dir.try_exists()? && git_dir.is_dir() {
        directories.push(directory);
        return Ok(directories);
    }
    for entry in fs::read_dir(&directory)? {
        let entry = entry?.path();
        if entry.is_dir() {
            directories = search_directory(entry, directories)?;
        }
    }

    Ok(directories)
}

fn fzf(paths: Vec<PathBuf>) -> Result<(), Error> {
    let mut window =
        Window::init(stdin(), stdout().lock().into_raw_mode()?, paths)?;
    window.register_help(Key::Ctrl('c'), "Quit")?;
    window.register_help(Key::Char('\n'), "Choose")?;
    loop {
        let input = window.get_input().to_string();
        // TODO: Create a proper fzf filter function including sort etc
        window.filter_paths(|path| path.to_str().unwrap().contains(&input));
        window.draw_paths()?;
        let key = match window.next() {
            Some(key) => key?,
            None => break,
        };
        match key {
            Key::Ctrl('c') => break,
            Key::Char('\n') => {
                let selected = match window.get_selected() {
                    Some(selected) => selected,
                    None => continue,
                };
                std::env::set_current_dir(selected)?;
                break;
            }
            Key::Char(ch) => window.push(ch),
            Key::Backspace => {
                let _ = window.pop();
            }
            _ => continue,
        }
    }
    Ok(())
}

fn remove_workspace_directory(
    name: String,
    project: PathBuf,
) -> Result<(), Error> {
    let config_file = CONFIG_FILE.lock()?;
    let content = fs::read_to_string(config_file.deref())?;
    let mut config: Config = toml::from_str(&content)?;
    let workspace = config
        .workspaces
        .get_mut(&name)
        .ok_or(Error::UndefinedWorkspace(name.clone()))?
        .as_array_mut()
        .ok_or(Error::InvalidWorkspace(name.clone()))?;
    for i in 0..workspace.len() {
        if PathBuf::from(
            workspace
                .get(i)
                .unwrap()
                .as_str()
                .ok_or(Error::InvalidWorkspace(name.clone()))?,
        ) == project
        {
            workspace.remove(i);
        }
    }
    fs::write(config_file.deref(), toml::to_string(&config)?)?;

    Ok(())
}

fn add_workspace_directory(
    name: String,
    project: PathBuf,
) -> Result<(), Error> {
    let config_file = CONFIG_FILE.lock()?;
    let content = fs::read_to_string(config_file.deref())?;
    let mut config: Config = toml::from_str(&content)?;
    let workspace = config
        .workspaces
        .get_mut(&name)
        .ok_or(Error::UndefinedWorkspace(name.clone()))?
        .as_array_mut()
        .ok_or(Error::InvalidWorkspace(name.clone()))?;
    for i in 0..workspace.len() {
        if PathBuf::from(
            workspace
                .get(i)
                .unwrap()
                .as_str()
                .ok_or(Error::InvalidWorkspace(name.clone()))?,
        ) == project
        {
            return Err(Error::DuplicateDirectory(name, project));
        }
    }
    workspace.push(toml::Value::String(project.to_string_lossy().to_string()));
    fs::write(config_file.deref(), toml::to_string(&config)?)?;

    Ok(())
}

fn remove_workspace(name: String) -> Result<(), Error> {
    let config_file = CONFIG_FILE.lock()?;
    let content = fs::read_to_string(config_file.deref())?;
    let mut config: Config = toml::from_str(&content)?;
    config.workspaces.remove(&name);
    fs::write(config_file.deref(), toml::to_string(&config)?)?;

    Ok(())
}

fn create_workspace(name: String) -> Result<(), Error> {
    let config_file = CONFIG_FILE.lock()?;
    let content = fs::read_to_string(config_file.deref())?;
    let mut config: Config = toml::from_str(&content)?;
    config.workspaces.insert(name, toml::Value::Array(vec![]));
    fs::write(config_file.deref(), toml::to_string(&config)?)?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Config {
    metadata: Metadata,
    workspaces: toml::Table,
}

#[derive(Serialize, Deserialize)]
struct Metadata {
    version: String,
}

#[cfg(test)]
mod tests {
    use std::{path::Path, str::FromStr};

    use super::*;
    use assert_fs::TempDir;
    use serial_test::serial;
    use toml::toml;

    struct TestEnvironment {
        temp_dir: TempDir,
        project_dir: PathBuf,
        config_file: PathBuf,
        default_workspace: String,
    }

    impl TestEnvironment {
        fn new() -> Self {
            let temp_dir = TempDir::new().unwrap();

            let project_dir = temp_dir.join("projects");

            let config_file = temp_dir.join("projectfinder.toml");

            {
                let mut config = CONFIG_FILE.lock().unwrap();
                *config = config_file.clone();
            }

            Self {
                temp_dir,
                project_dir,
                config_file,
                default_workspace: "default".to_string(),
            }
        }
        fn init(self) -> Self {
            let project_dir_str = self.project_dir.to_string_lossy();
            let version = crate_version!();
            let table = toml! {
                [metadata]
                version = version

                [workspaces]
                default = [project_dir_str]
            }
            .to_string();

            fs::write(&self.config_file, table).unwrap();
            fs::create_dir(&self.project_dir).unwrap();

            self
        }
    }

    impl Deref for TestEnvironment {
        type Target = Path;

        fn deref(&self) -> &Self::Target {
            self.temp_dir.path()
        }
    }

    #[test]
    #[serial]
    fn test_init() {
        let test_env = TestEnvironment::new();

        init().unwrap();

        let string_config = fs::read_to_string(test_env.config_file).unwrap();
        let config = toml::Table::from_str(&string_config).unwrap();

        let version = crate_version!();
        let maybe_config = toml! {
            [metadata]
            version = version

            [workspaces]
        };

        assert_eq!(config, maybe_config);
    }

    #[test]
    #[serial]
    fn test_create_workspace() {
        let test_env = TestEnvironment::new().init();
        let workspace_name = "new_workspace";
        let str = fs::read_to_string(&test_env.config_file).unwrap();
        let config: Config = toml::from_str(&str).unwrap();
        assert!(!config.workspaces.contains_key(workspace_name));
        create_workspace(workspace_name.to_string()).unwrap();
        let str = fs::read_to_string(&test_env.config_file).unwrap();
        let config: Config = toml::from_str(&str).unwrap();
        assert!(config.workspaces.contains_key(workspace_name));
        assert_eq!(
            config.workspaces.get(workspace_name).unwrap(),
            &toml::Value::Array(vec![])
        );
    }

    #[test]
    #[serial]
    fn test_remove_workspace() {
        let test_env = TestEnvironment::new().init();
        let workspace_name = test_env.default_workspace;
        let str = fs::read_to_string(&test_env.config_file).unwrap();
        let config: Config = toml::from_str(&str).unwrap();
        assert!(config.workspaces.contains_key(&workspace_name));
        remove_workspace(workspace_name.to_string()).unwrap();
        let str = fs::read_to_string(test_env.config_file).unwrap();
        let config: Config = toml::from_str(&str).unwrap();
        assert!(!config.workspaces.contains_key(&workspace_name));
    }

    #[test]
    #[serial]
    fn test_add_directory() {
        let test_env = TestEnvironment::new().init();
        let project_dir = test_env.join("/other_directory");
        add_workspace_directory(
            test_env.default_workspace.clone(),
            project_dir.clone(),
        )
        .unwrap();
        let content = fs::read_to_string(test_env.config_file).unwrap();
        let config: Config = toml::from_str(&content).unwrap();
        let workspace = config
            .workspaces
            .get(&test_env.default_workspace)
            .unwrap()
            .as_array()
            .unwrap();
        let mut counter = 0;
        for directory in workspace {
            if PathBuf::from(directory.as_str().unwrap())
                == test_env.project_dir
            {
                counter += 1;
            }
        }

        if counter > 1 {
            panic!("Duplicate directory")
        } else if counter == 0 {
            panic!("Directory not added")
        }
    }

    #[test]
    #[serial]
    fn test_remove_directory() {
        let test_env = TestEnvironment::new().init();
        remove_workspace_directory(
            test_env.default_workspace.clone(),
            test_env.project_dir.clone(),
        )
        .unwrap();
        let content = fs::read_to_string(test_env.config_file).unwrap();
        let config: Config = toml::from_str(&content).unwrap();
        let workspace = config
            .workspaces
            .get(&test_env.default_workspace)
            .unwrap()
            .as_array()
            .unwrap();
        for directory in workspace {
            if PathBuf::from(directory.as_str().unwrap())
                == test_env.project_dir
            {
                panic!("Did not delete all instances of directory within workspace");
            }
        }
    }

    #[test]
    #[serial]
    fn test_find_current_workspace() {
        let test_env = TestEnvironment::new().init();
        env::set_current_dir(&test_env.project_dir).unwrap();
        assert_eq!(
            find_current_workspace().unwrap(),
            test_env.default_workspace
        );
        let sub_dir = test_env.project_dir.join("project");
        fs::create_dir(&sub_dir).unwrap();
        env::set_current_dir(sub_dir).unwrap();
        assert_eq!(
            find_current_workspace().unwrap(),
            test_env.default_workspace
        );
    }

    #[test]
    #[serial]
    fn test_search_workspace() {
        let test_env = TestEnvironment::new().init();
        let file_trap = test_env.project_dir.join("trap");
        fs::write(&file_trap, "").unwrap();

        let dir_trap = test_env.project_dir.join("dir_trap");
        fs::create_dir(&dir_trap).unwrap();

        let a_project = test_env.project_dir.join("a_project");
        fs::create_dir(&a_project).unwrap();
        fs::create_dir(a_project.join(".git")).unwrap();

        let another_project = test_env.project_dir.join("another_project");
        fs::create_dir(&another_project).unwrap();
        fs::create_dir(another_project.join(".git")).unwrap();

        let subdir_project =
            test_env.project_dir.join("subdir").join("project");
        fs::create_dir_all(&subdir_project).unwrap();
        fs::create_dir(subdir_project.join(".git")).unwrap();

        // INFO: The current temp directory structure should be the following
        // projectfinder.toml
        // projects/    trap
        //              dir_trap/
        //              a_project/          .git/
        //              another_project/    .git/
        //              subdir/             project/    .git/

        let directories = search_workspace("default".to_string()).unwrap();

        assert!(directories.contains(&a_project));
        assert!(directories.contains(&another_project));
        assert!(directories.contains(&subdir_project));
        assert!(!directories.contains(&file_trap));
        assert!(!directories.contains(&dir_trap));
    }
}
