use flate2::read::GzDecoder;
use sha2::Digest;
use std::{fs, io::Read};

const REPO: &str = "https://github.com/rtk-ai/rtk";
const MAX_DOWNLOAD: usize = 64 * 1024 * 1024;

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
    let dir = home.join(".local/bin");
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Não foi possível criar {}: {e}", dir.display()))?;
    let path = dir.join("rtk");
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
    Ok(format!("RTK {tag} instalado em {}", path.display()))
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
