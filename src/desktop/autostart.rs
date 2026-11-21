use crate::files::{write_atomic, xdg_directory};
use anyhow::{Result, bail};
use std::{fs, path::PathBuf};

pub fn set_login(enabled: bool) -> Result<()> {
    let snap = std::env::var_os("SNAP_USER_DATA");
    let config = match &snap {
        Some(p) => PathBuf::from(p).join(".config"),
        None => xdg_directory("XDG_CONFIG_HOME", ".config")?,
    };
    let path = config.join("autostart/ubrus-login.desktop");
    if !enabled {
        return match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        };
    }
    let command = if snap.is_some() {
        "ubrus".into()
    } else {
        let exe = match super::updates::installed_executable() {
            Some(path) => path.to_owned(),
            None => std::env::current_exe()?,
        };
        let exe = exe.to_string_lossy();
        if exe.contains(['\n', '\r']) {
            bail!("Invalid executable path");
        }
        format!(
            "\"{}\"",
            exe.replace('\\', "\\\\\\\\")
                .replace('"', "\\\\\"")
                .replace('$', "\\\\$")
                .replace('`', "\\\\`")
                .replace('%', "%%")
        )
    };
    write_atomic(
        &path,
        format!("[Desktop Entry]\nType=Application\nName=Ubrus\nExec={command} --background\nTerminal=false\n")
            .as_bytes(),
    )
}
