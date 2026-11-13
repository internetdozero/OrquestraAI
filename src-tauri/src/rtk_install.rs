use flate2::read::GzDecoder;
use sha2::Digest;
use std::{fs, io::Read, process::Command};

const REPO: &str = "https://github.com/rtk-ai/rtk";
const MAX_DOWNLOAD: usize = 64 * 1024 * 1024;

pub fn status() -> Option<String> {
    // `exec_env` searches the login shell's PATH too, which is where a GUI
    // launch finds nvm/cargo/mise binaries.
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Some(found) = crate::exec_env::resolve(binary_name()) {
        candidates.push(found);
    }
    if let Ok(home) = crate::paths::require_home("locate RTK") {
        let installed = install_dir(&home).join(binary_name());
        if !candidates.contains(&installed) {
            candidates.push(installed);
        }
    }

    for path in candidates {
        if !path.is_file() {
            continue;
        }
        if let Ok(output) = Command::new(&path).arg("--version").output() {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return Some(if version.is_empty() {
                    path.display().to_string()
                } else {
                    format!("{version} — {}", path.display())
                });
            }
        }
    }
    None
}

fn binary_name() -> &'static str {
    if cfg!(windows) {
        "rtk.exe"
    } else {
        "rtk"
    }
}

pub fn install() -> Result<String, String> {
    let target = target_triple()?;
    let asset = format!("rtk-{target}.tar.gz");
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .timeout(std::time::Duration::from_secs(180))
        .user_agent("OrquestraAI")
        .build()
        .map_err(|e| format!("Não foi possível iniciar o download do RTK: {e}"))?;
    let release: serde_json::Value = client
        .get("https://api.github.com/repos/rtk-ai/rtk/releases/latest")
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .and_then(|r| r.json())
        .map_err(|e| format!("Não foi possível consultar a versão mais recente do RTK: {e}"))?;
    let tag = release["tag_name"]
        .as_str()
        .ok_or("O GitHub não retornou a versão do RTK")?;
    if tag.is_empty() || tag.contains('/') || tag.contains("..") {
        return Err("O GitHub retornou uma versão inválida do RTK".into());
    }
    let base = format!("{REPO}/releases/download/{tag}");
    let archive = download(&client, &format!("{base}/{asset}"))?;
    let sums = download(&client, &format!("{base}/checksums.txt"))?;
    let expected = String::from_utf8_lossy(&sums)
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            let hash = fields.next()?;
            let name = fields.next()?.trim_start_matches('*');
            (name == asset).then(|| hash.to_ascii_lowercase())
        })
        .ok_or_else(|| {
            format!("O checksum do binário {asset} não está disponível; instalação cancelada")
        })?;
    let actual = format!("{:x}", sha2::Sha256::digest(&archive));
    if expected != actual {
        return Err("A verificação de integridade do RTK falhou; instalação cancelada".into());
    }

    let mut tar = tar::Archive::new(GzDecoder::new(archive.as_slice()));
    let mut binary = None;
    for item in tar
        .entries()
        .map_err(|e| format!("Pacote do RTK inválido: {e}"))?
    {
        let entry = item.map_err(|e| format!("Pacote do RTK inválido: {e}"))?;
        if entry
            .path()
            .map_err(|e| format!("Caminho inválido no pacote: {e}"))?
            .as_os_str()
            == "rtk"
        {
            if !entry.header().entry_type().is_file() {
                return Err("O pacote do RTK contém um binário inválido".into());
            }
            let mut bytes = Vec::new();
            entry
                .take(MAX_DOWNLOAD as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| format!("Falha ao extrair RTK: {e}"))?;
            if bytes.len() > MAX_DOWNLOAD {
                return Err("O binário do RTK excede o limite permitido".into());
            }
            binary = Some(bytes);
            break;
        }
    }
    let binary = binary.ok_or("O pacote baixado não contém o binário RTK")?;
    let home = crate::paths::require_home("instalar RTK")?;
    let dir = install_dir(&home);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Não foi possível criar {}: {e}", dir.display()))?;
    let path = dir.join(binary_name());
    let temp = dir.join(format!(
        ".rtk-install-{}",
        crate::paths::unique_tmp_suffix()
    ));
    fs::write(&temp, binary).map_err(|e| format!("Não foi possível gravar o RTK: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temp, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Não foi possível preparar o RTK: {e}"))?;
    }
    fs::rename(&temp, &path).map_err(|e| {
        let _ = fs::remove_file(&temp);
        format!("Não foi possível instalar o RTK em {}: {e}", path.display())
    })?;

    // A tool the app shells out to is useless if the operator's shell cannot
    // find it: `~/.local/bin` is not on PATH by default in zsh (macOS) and is
    // never on it in Windows, so the message says what to add instead of
    // claiming an install that cannot be used.
    let dirs = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .unwrap_or_default();
    let mut message = format!("RTK {tag} instalado em {}", path.display());
    // A login shell may already export the directory even when this process's
    // PATH does not; do not tell the operator to edit an rc file that is fine.
    let reachable = on_path(&dir, &dirs)
        || crate::exec_env::extra_dirs()
            .iter()
            .any(|entry| entry == &dir);
    if !reachable {
        message.push_str(&format!(
            " — este diretório não está no PATH; para usar o RTK adicione:\n  {}",
            path_hint(&dir)
        ));
    }
    Ok(message)
}

/// Where RTK is installed.
///
/// `~/.local/bin` is the XDG convention on Linux and works on macOS too;
/// Windows has no such folder, so the app keeps its tools under
/// `%LOCALAPPDATA%` instead of littering the profile root.
fn install_dir(home: &std::path::Path) -> std::path::PathBuf {
    if cfg!(windows) {
        home.join("AppData/Local/OrquestraAI/bin")
    } else {
        home.join(".local/bin")
    }
}

/// Is `dir` one of the entries a shell would search?
fn on_path(dir: &std::path::Path, entries: &[std::path::PathBuf]) -> bool {
    entries.iter().any(|entry| entry == dir)
}

/// The exact line the operator has to add, in their shell's syntax.
fn path_hint(dir: &std::path::Path) -> String {
    let shown = dir.display();
    if cfg!(windows) {
        format!("setx PATH \"%PATH%;{shown}\"")
    } else {
        // One line that works in bash, zsh, sh and fish's POSIX mode; the
        // persistent form belongs in the shell's rc file, which only the
        // operator knows.
        format!("export PATH=\"{shown}:$PATH\"  # add to ~/.profile or ~/.zshrc")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_on_path_is_recognized_byte_for_byte() {
        let dir = std::path::PathBuf::from("/home/dev/.local/bin");
        assert!(on_path(
            &dir,
            &[std::path::PathBuf::from("/usr/bin"), dir.clone()]
        ));
        assert!(!on_path(&dir, &[std::path::PathBuf::from("/usr/bin")]));
        // A trailing separator is the same directory: `Path` comparison
        // ignores it, and so should this.
        assert!(on_path(
            &dir,
            &[std::path::PathBuf::from("/home/dev/.local/bin/")]
        ));
        // A different directory that merely shares the suffix is not a match.
        assert!(!on_path(&dir, &[std::path::PathBuf::from("/opt/bin")]));
    }

    #[test]
    fn the_hint_names_the_install_directory() {
        let dir = install_dir(std::path::Path::new("/home/dev"));
        let hint = path_hint(&dir);
        assert!(hint.contains(&dir.display().to_string()), "{hint}");
        if cfg!(windows) {
            assert!(hint.starts_with("setx PATH"), "{hint}");
        } else {
            assert!(hint.starts_with("export PATH="), "{hint}");
        }
    }

    #[test]
    fn the_install_directory_follows_the_platform_convention() {
        let dir = install_dir(std::path::Path::new("/home/dev"));
        if cfg!(windows) {
            assert!(dir.ends_with("OrquestraAI/bin"), "{}", dir.display());
        } else {
            assert_eq!(dir, std::path::PathBuf::from("/home/dev/.local/bin"));
        }
    }
}

fn download(client: &reqwest::blocking::Client, url: &str) -> Result<Vec<u8>, String> {
    let response = client
        .get(url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|e| format!("Falha ao baixar RTK ({url}): {e}"))?;
    if response
        .content_length()
        .is_some_and(|n| n > MAX_DOWNLOAD as u64)
    {
        return Err("Download do RTK excede o limite permitido".into());
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_DOWNLOAD as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Falha ao receber RTK: {e}"))?;
    if bytes.len() > MAX_DOWNLOAD {
        return Err("Download do RTK excede o limite permitido".into());
    }
    Ok(bytes)
}

fn target_triple() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-musl"),
        ("linux", "aarch64") => Ok("aarch64-unknown-linux-gnu"),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin"),
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        _ => Err(
            "A instalação automática do RTK oferece suporte a Linux e macOS (x86_64/ARM64)".into(),
        ),
    }
}
