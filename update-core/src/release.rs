//! Os arquivos de uma release do GitHub e a escolha do pacote certo para esta máquina.
//!
//! Serve ao daemon e ao app desktop: os dois leem o JSON de `releases/latest` e precisam do
//! `.deb` do **seu** produto, para **esta** arquitetura, mais a `.minisig` dele — e de nenhum
//! arquivo de outro lugar. Quem baixa não precisa confiar no resultado (o helper root confere a
//! assinatura), mas escolher errado só atrasaria a recusa.

use serde::Deserialize;

/// Um arquivo anexado à release (`rustployd_0.1.10-1_amd64.deb`, o `.minisig`…).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
}

/// O que interessa do JSON de uma release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    /// A tag como publicada (`v0.1.7`).
    pub tag: String,
    /// Página da release (notas e arquivos).
    pub html_url: String,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Deserialize)]
struct ReleasePayload {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<AssetPayload>,
}

#[derive(Deserialize)]
struct AssetPayload {
    name: String,
    browser_download_url: String,
}

/// Lê o JSON de `GET /repos/{repo}/releases/latest`.
pub fn parse_release(json: &str) -> Result<ReleaseInfo, String> {
    let payload: ReleasePayload = serde_json::from_str(json)
        .map_err(|error| format!("resposta do GitHub ilegível: {error}"))?;
    Ok(ReleaseInfo {
        tag: payload.tag_name,
        html_url: payload.html_url,
        assets: payload
            .assets
            .into_iter()
            .map(|asset| ReleaseAsset {
                name: asset.name,
                url: asset.browser_download_url,
            })
            .collect(),
    })
}

/// Arquitetura no vocabulário do Debian, a que aparece no nome do `.deb`.
pub fn debian_architecture() -> Result<&'static str, String> {
    debian_architecture_of(std::env::consts::ARCH)
}

pub fn debian_architecture_of(rust_architecture: &str) -> Result<&'static str, String> {
    match rust_architecture {
        "x86_64" => Ok("amd64"),
        "aarch64" => Ok("arm64"),
        other => Err(format!("não há pacote para a arquitetura {other}")),
    }
}

/// O `.deb` de `package` para `architecture` e a `.minisig` dele, **dentro do repositório dado**
/// (uma URL fora de `https://github.com/<repo>/releases/download/` nunca é escolhida).
///
/// O nome do arquivo tem de começar por `<package>_`: o underscore impede que `rustploy` case com
/// `rustploy-gui_…`.
pub fn select_package_assets(
    assets: &[ReleaseAsset],
    repo: &str,
    package: &str,
    architecture: &str,
    tag: &str,
) -> Result<(ReleaseAsset, ReleaseAsset), String> {
    let allowed_prefix = format!("https://github.com/{repo}/releases/download/");
    let from_repo = |asset: &&ReleaseAsset| asset.url.starts_with(&allowed_prefix);
    let name_start = format!("{package}_");
    let name_end = format!("_{architecture}.deb");
    let deb = assets
        .iter()
        .filter(from_repo)
        .find(|asset| asset.name.starts_with(&name_start) && asset.name.ends_with(&name_end))
        .ok_or_else(|| {
            format!("a release {tag} não tem pacote de {package} para {architecture}")
        })?;
    let signature_name = format!("{}.minisig", deb.name);
    let signature = assets
        .iter()
        .filter(from_repo)
        .find(|asset| asset.name == signature_name)
        .ok_or_else(|| {
            format!("a release {tag} não tem a assinatura ({signature_name}); só instalo pacote assinado")
        })?;
    Ok((deb.clone(), signature.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str, repo: &str) -> ReleaseAsset {
        ReleaseAsset {
            name: name.into(),
            url: format!("https://github.com/{repo}/releases/download/v0.1.16/{name}"),
        }
    }

    fn full_release(repo: &str) -> Vec<ReleaseAsset> {
        [
            "checksums.sha256",
            "rustploy-gui-windows-v0.1.16.zip",
            "rustploy-gui_0.1.20-1_amd64.deb",
            "rustploy-gui_0.1.20-1_amd64.deb.minisig",
            "rustployd_0.1.10-1_amd64.deb",
            "rustployd_0.1.10-1_amd64.deb.minisig",
            "rustployd_0.1.10-1_arm64.deb",
            "rustployd_0.1.10-1_arm64.deb.minisig",
        ]
        .map(|name| asset(name, repo))
        .to_vec()
    }

    #[test]
    fn picks_the_package_for_this_product_and_architecture_with_its_signature() {
        let assets = full_release("o/r");
        let (deb, signature) =
            select_package_assets(&assets, "o/r", "rustployd", "arm64", "v1").unwrap();
        assert_eq!(deb.name, "rustployd_0.1.10-1_arm64.deb");
        assert_eq!(signature.name, "rustployd_0.1.10-1_arm64.deb.minisig");

        let (gui, gui_signature) =
            select_package_assets(&assets, "o/r", "rustploy-gui", "amd64", "v1").unwrap();
        assert_eq!(gui.name, "rustploy-gui_0.1.20-1_amd64.deb");
        assert_eq!(
            gui_signature.name,
            "rustploy-gui_0.1.20-1_amd64.deb.minisig"
        );
    }

    #[test]
    fn a_shorter_package_name_does_not_match_a_longer_one() {
        // `rustploy` não pode casar com `rustploy-gui_…` nem com `rustployd_…`.
        let error = select_package_assets(&full_release("o/r"), "o/r", "rustploy", "amd64", "v1")
            .unwrap_err();
        assert!(error.contains("não tem pacote"), "{error}");
    }

    #[test]
    fn a_release_without_a_signature_is_not_installable() {
        let mut assets = full_release("o/r");
        assets.retain(|asset| !asset.name.ends_with(".minisig"));
        let error = select_package_assets(&assets, "o/r", "rustployd", "amd64", "v1").unwrap_err();
        assert!(error.contains("só instalo pacote assinado"), "{error}");
    }

    #[test]
    fn a_release_without_a_package_for_the_architecture_is_refused() {
        let error =
            select_package_assets(&full_release("o/r"), "o/r", "rustployd", "riscv64", "v1")
                .unwrap_err();
        assert!(error.contains("riscv64"), "{error}");
    }

    #[test]
    fn urls_outside_the_configured_repo_are_ignored() {
        let error = select_package_assets(
            &full_release("atacante/repo"),
            "o/r",
            "rustployd",
            "amd64",
            "v1",
        )
        .unwrap_err();
        assert!(error.contains("não tem pacote"), "{error}");

        let mut hostile = full_release("o/r");
        for asset in &mut hostile {
            asset.url = format!("https://evil.example/{}", asset.name);
        }
        assert!(select_package_assets(&hostile, "o/r", "rustployd", "amd64", "v1").is_err());
    }

    #[test]
    fn rust_architectures_map_to_debian_ones() {
        assert_eq!(debian_architecture_of("x86_64"), Ok("amd64"));
        assert_eq!(debian_architecture_of("aarch64"), Ok("arm64"));
        assert!(debian_architecture_of("riscv64").is_err());
    }

    #[test]
    fn parses_the_github_release_json() {
        let json = r#"{
            "tag_name": "v0.1.8", "html_url": "https://github.com/o/r/releases/tag/v0.1.8",
            "draft": false,
            "assets": [
              {"name": "rustployd_0.1.10-1_amd64.deb", "size": 1, "browser_download_url": "https://github.com/o/r/releases/download/v0.1.8/rustployd_0.1.10-1_amd64.deb"}
            ]
        }"#;
        let release = parse_release(json).unwrap();
        assert_eq!(release.tag, "v0.1.8");
        assert_eq!(release.assets.len(), 1);
        assert_eq!(release.assets[0].name, "rustployd_0.1.10-1_amd64.deb");
        assert!(parse_release("{}").is_err());
        assert!(parse_release("<html>").is_err());
        // Sem assets é uma release válida, só sem arquivos.
        assert!(
            parse_release(r#"{"tag_name":"v1","html_url":"https://x"}"#)
                .unwrap()
                .assets
                .is_empty()
        );
    }
}
