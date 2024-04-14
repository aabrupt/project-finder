use std::{env::VarError, path::PathBuf, sync::PoisonError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    SystemError(#[from] std::io::Error),
    #[error(transparent)]
    VariableLookupError(#[from] shellexpand::LookupError<VarError>),
    #[error("Following path contain invalid characters: {0:?}")]
    PathUnicodeError(PathBuf),
    #[error("Got unhandled action: {0}")]
    UnhandledAction(String),
    #[error(transparent)]
    GlobalPathError(
        #[from] PoisonError<std::sync::MutexGuard<'static, PathBuf>>,
    ),
    #[error("Expected value or default value for argument: {0}")]
    UnhandledMissingArgument(String),
    #[error(transparent)]
    TomlDeserializeError(#[from] toml::de::Error),
    #[error(transparent)]
    TomlSerializeError(#[from] toml::ser::Error),
    #[error("The following workspace is undefined: {0}")]
    UndefinedWorkspace(String),
    #[error("The following workspace is not a toml array: {0}")]
    InvalidWorkspace(String),
    #[error("The workspace '{0}' already contain '{1}'")]
    DuplicateDirectory(String, PathBuf),
    #[error("Current path is not within a project directory")]
    NotInWorkspace(PathBuf),
    #[error("All directories must be absolute within a workspace; Found {1:?} in {0}")]
    RelativeDirectoryError(String, PathBuf),
}
