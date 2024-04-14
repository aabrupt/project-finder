use std::{env, path::PathBuf};

use tracing::{debug, info, trace};

use crate::error::Error;

pub fn resolve_path_variables(path: PathBuf) -> Result<PathBuf, Error> {
    debug!("resolving variables in path: {:?}", path);
    Ok(shellexpand::full(
        path.to_str()
            .ok_or(Error::PathUnicodeError(path.to_path_buf()))?,
    )?
    .to_string()
    .into())
}
pub fn substitute_path_with_variables(path: PathBuf) -> Result<PathBuf, Error> {
    debug!("substituting following path with variables: {:?}", path);
    trace!("cleaning out non-path variables");
    let mut variables = env::vars().filter_map(|(name, value)| {
        let path = PathBuf::from(&value);
        trace!("processing {} with value {}", name, value);
        if !path.is_absolute() {
            return None;
        }
        Some((name, path))
    });
    trace!("variables to substitute with: {:?}", variables);
    if let Some((name, value)) = path.ancestors().find_map(|ancestor| {
        trace!("current ancestor: {:?}", ancestor);
        variables.find_map(|(name, value)| {
            if ancestor != value {
                return None;
            }
            debug!("variable matching ancestor found: {:?}", name);
            Some((name, value))
        })
    }) {
        let new_path: PathBuf = path
            .to_str()
            .ok_or(Error::PathUnicodeError(path.to_path_buf()))?
            .replace(
                value
                    .to_str()
                    .ok_or(Error::PathUnicodeError(value.to_path_buf()))?,
                &format!("${}", name),
            )
            .into();

        info!("{:?} became {:?}", path, new_path);

        return Ok(new_path);
    }
    debug!("did not find any variables that matches path ancestry");
    return Ok(path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_path_with_home() {
        assert_eq!(
            resolve_path_variables("~".into()).unwrap(),
            PathBuf::from(std::env::var_os("HOME").unwrap())
        );
        assert_eq!(
            resolve_path_variables("$HOME".into()).unwrap(),
            PathBuf::from(std::env::var_os("HOME").unwrap())
        );
        assert_eq!(
            resolve_path_variables("$XDG_CONFIG_HOME".into()).unwrap(),
            PathBuf::from(std::env::var_os("XDG_CONFIG_HOME").unwrap())
        );
    }

    #[test]
    fn resolve_variable_home_from_path() {
        assert_eq!(
            substitute_path_with_variables(
                std::env::var_os("HOME").unwrap().into()
            )
            .unwrap(),
            PathBuf::from("$HOME")
        );
        assert_eq!(
            substitute_path_with_variables(
                std::env::var_os("XDG_CONFIG_HOME").unwrap().into()
            )
            .unwrap(),
            PathBuf::from("$XDG_CONFIG_HOME")
        );
    }
}
