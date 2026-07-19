use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::locator::locate_zero;

/// Uma entrada do catálogo de provedores do `zero` CLI (`zero providers
/// catalog --json`). Cobre tanto provedores prontos (OpenAI, Anthropic,
/// Google, etc.) quanto as duas entradas especiais `custom-openai-compatible`
/// / `custom-anthropic-compatible`, usadas para cadastrar um endpoint 100%
/// personalizado - não existe distinção de comando entre os dois casos, só
/// o `id` muda.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCatalogEntry {
    pub id: String,
    pub name: String,
    pub transport: String,
    #[serde(rename = "defaultBaseUrl", default)]
    pub default_base_url: String,
    #[serde(rename = "defaultModel", default)]
    pub default_model: String,
    #[serde(rename = "authEnvVars", default)]
    pub auth_env_vars: Vec<String>,
    #[serde(rename = "requiresAuth", default)]
    pub requires_auth: bool,
    #[serde(default)]
    pub local: bool,
    #[serde(rename = "runtimeSupported", default)]
    pub runtime_supported: bool,
    #[serde(default)]
    pub recommended: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogOutput {
    #[serde(default)]
    providers: Vec<ProviderCatalogEntry>,
}

/// Um perfil de provedor já configurado em `~/.config/zero/config.json`
/// (`zero providers list --json`). Nunca carrega a API key em si - a CLI já
/// redige segredos na saída, só expõe `api_key_set`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfiguredProvider {
    pub name: String,
    #[serde(rename = "providerKind", default)]
    pub provider_kind: String,
    #[serde(rename = "baseUrl", default)]
    pub base_url: String,
    #[serde(default)]
    pub model: String,
    #[serde(rename = "apiModel", default)]
    pub api_model: String,
    #[serde(default)]
    pub active: bool,
    #[serde(rename = "apiKeySet", default)]
    pub api_key_set: bool,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProvidersListOutput {
    #[serde(default)]
    providers: Vec<ConfiguredProvider>,
}

/// Payload do form "Adicionar provedor". `auth_header_value` é a API key em
/// texto puro - vive só na memória deste processo pelo tempo de montar o
/// argv do `zero providers add`; nunca é logada nem persistida por este app.
/// O único lugar que a guarda é o `~/.config/zero/config.json` do próprio
/// `zero`.
#[derive(Debug, Clone, Deserialize)]
pub struct AddProviderRequest {
    pub catalog_id: String,
    pub name: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub auth_header: Option<String>,
    #[serde(default)]
    pub auth_scheme: Option<String>,
    #[serde(default)]
    pub auth_header_value: Option<String>,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub set_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderHealthCheck {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderHealth {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub checks: Vec<ProviderHealthCheck>,
}

/// Resultado de `zero providers check <name> [--connectivity] --json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCheckResult {
    pub provider: ConfiguredProvider,
    pub status: String,
    #[serde(default)]
    pub health: Option<ProviderHealth>,
    #[serde(rename = "nextActions", default)]
    pub next_actions: Vec<String>,
}

fn zero_path() -> Result<std::path::PathBuf, String> {
    Ok(locate_zero()
        .map_err(|e| format!("Failed to locate zero CLI: {e}"))?
        .path)
}

/// Lista o catálogo de provedores conhecidos pelo `zero` CLI.
pub async fn catalog() -> Result<Vec<ProviderCatalogEntry>, String> {
    let path = zero_path()?;
    let output = Command::new(&path)
        .arg("providers")
        .arg("catalog")
        .arg("--json")
        .output()
        .await
        .map_err(|e| format!("Failed to run zero providers catalog: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("zero providers catalog failed: {stderr}"));
    }

    let parsed: CatalogOutput = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse providers catalog JSON: {e}"))?;
    Ok(parsed.providers)
}

/// Lista os perfis de provedor já configurados.
pub async fn list_configured() -> Result<Vec<ConfiguredProvider>, String> {
    let path = zero_path()?;
    let output = Command::new(&path)
        .arg("providers")
        .arg("list")
        .arg("--json")
        .output()
        .await
        .map_err(|e| format!("Failed to run zero providers list: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("zero providers list failed: {stderr}"));
    }

    let parsed: ProvidersListOutput = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse providers list JSON: {e}"))?;
    Ok(parsed.providers)
}

/// Adiciona (ou atualiza in-place, se `name` já existir) um perfil de
/// provedor - cobre tanto um `catalog_id` já existente no catálogo (OpenAI,
/// Anthropic, etc.) quanto os dois catalog-ids especiais
/// `custom-openai-compatible`/`custom-anthropic-compatible` usados para um
/// endpoint 100% personalizado. `set_active`, quando `true`, muda o
/// provedor ativo GLOBALMENTE para todo processo `zero` na máquina - a
/// chamada deve vir de uma ação explícita do usuário na UI, nunca como
/// default.
pub async fn add(req: &AddProviderRequest) -> Result<(), String> {
    let path = zero_path()?;
    let mut cmd = Command::new(&path);
    cmd.arg("providers")
        .arg("add")
        .arg(&req.catalog_id)
        .arg("--name")
        .arg(&req.name);

    if let Some(model) = &req.model {
        if !model.is_empty() {
            cmd.arg("--model").arg(model);
        }
    }
    if let Some(base_url) = &req.base_url {
        if !base_url.is_empty() {
            cmd.arg("--base-url").arg(base_url);
        }
    }
    if let Some(env) = &req.api_key_env {
        if !env.is_empty() {
            cmd.arg("--api-key-env").arg(env);
        }
    }
    if let Some(header) = &req.auth_header {
        if !header.is_empty() {
            cmd.arg("--auth-header").arg(header);
        }
    }
    if let Some(scheme) = &req.auth_scheme {
        if !scheme.is_empty() {
            cmd.arg("--auth-scheme").arg(scheme);
        }
    }
    if let Some(value) = &req.auth_header_value {
        if !value.is_empty() {
            cmd.arg("--auth-header-value").arg(value);
        }
    }
    for (key, value) in &req.headers {
        cmd.arg("--header").arg(format!("{key}={value}"));
    }
    if req.set_active {
        cmd.arg("--set-active");
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to run zero providers add: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("zero providers add failed: {stderr}"));
    }
    Ok(())
}

/// Remove um perfil de provedor configurado.
pub async fn remove(name: &str) -> Result<(), String> {
    let path = zero_path()?;
    let output = Command::new(&path)
        .arg("providers")
        .arg("remove")
        .arg(name)
        .output()
        .await
        .map_err(|e| format!("Failed to run zero providers remove: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("zero providers remove failed: {stderr}"));
    }
    Ok(())
}

/// Ativa um perfil de provedor já configurado - mesmo efeito colateral
/// global de `add(..., set_active: true)`, mas sem re-adicionar o perfil.
pub async fn use_provider(name: &str) -> Result<(), String> {
    let path = zero_path()?;
    let output = Command::new(&path)
        .arg("providers")
        .arg("use")
        .arg(name)
        .output()
        .await
        .map_err(|e| format!("Failed to run zero providers use: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("zero providers use failed: {stderr}"));
    }
    Ok(())
}

/// Testa um perfil de provedor configurado (`--connectivity` também sonda o
/// endpoint ao vivo, não é instantâneo).
pub async fn check(name: &str, connectivity: bool) -> Result<ProviderCheckResult, String> {
    let path = zero_path()?;
    let mut cmd = Command::new(&path);
    cmd.arg("providers").arg("check").arg(name);
    if connectivity {
        cmd.arg("--connectivity");
    }
    cmd.arg("--json");

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to run zero providers check: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("zero providers check failed: {stderr}"));
    }

    serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse providers check JSON: {e}"))
}
