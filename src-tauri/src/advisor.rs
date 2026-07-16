use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Configuração do Advisor Mode para uma sessão.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdvisorConfig {
    /// Se o advisor está ativado para esta sessão.
    pub enabled: bool,
    /// Modelo a ser usado pelo advisor (None = usar modelo do executor).
    pub model: Option<String>,
}

/// Caminho para o arquivo de configuração global do advisor.
fn advisor_config_path() -> Result<PathBuf, String> {
    let base = dirs::data_dir().ok_or_else(|| "Could not resolve app data directory".to_string())?;
    Ok(base.join("zero-desktop").join("advisor-config.json"))
}

/// Carrega a configuração global do advisor.
pub fn load_global_config() -> AdvisorConfig {
    let Ok(path) = advisor_config_path() else {
        return AdvisorConfig::default();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return AdvisorConfig::default();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// Salva a configuração global do advisor.
pub fn save_global_config(config: &AdvisorConfig) -> Result<(), String> {
    let path = advisor_config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

/// Gera o system prompt de instrução para o executor quando o advisor está ativado.
/// Este prompt é injetado nas mensagens do sistema para instruir o executor
/// a usar a tool Task com o specialist advisor quando precisar de orientação.
pub fn executor_instruction_prompt(config: &AdvisorConfig) -> Option<String> {
    if !config.enabled {
        return None;
    }

    let model_hint = config
        .model
        .as_ref()
        .map(|m| format!(" (modelo recomendado: {})", m))
        .unwrap_or_default();

    Some(format!(
        r#"

<advisor_mode>
O modoAdvisor está ATIVADO. Quando precisar de orientação sobre:
- Arquitetura de software e design patterns
- Decisões de implementação complexas
- Review de código crítico
- Otimização de performance
- Segurança e boas práticas

Use a tool `Task` para consultar o specialist `advisor`:

```json
{{
  "name": "advisor",
  "prompt": "<contexto relevante para análise>",
  "description": "Consultoria técnica"
}}
```

Forneça contexto suficiente para que o advisor possa dar recomendações precisas.
Inclua: código relevante, restrições do projeto, e o que especificamente precisa de orientação{model_hint}.

O advisor retornará análise e recomendações que você deve considerar antes de implementar.
Não ignore as recomendações do advisor sem justificativa clara.
</advisor_mode>
"#
    ))
}

/// Verifica se uma mensagem contém uma chamada à tool Task com o specialist advisor.
/// Usado para detectar consultas ao advisor no fluxo de eventos.
pub fn is_advisor_consultation(tool_name: &str, args: &serde_json::Value) -> bool {
    if tool_name != "Task" {
        return false;
    }
    args.get("name")
        .and_then(|v| v.as_str())
        .map(|name| name == "advisor")
        .unwrap_or(false)
}

/// Extrai o prompt de uma consulta ao advisor.
pub fn extract_advisor_prompt(args: &serde_json::Value) -> Option<String> {
    args.get("prompt")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Caminho do specialist `advisor` dentro de um workspace.
fn specialist_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".zero").join("specialists").join("advisor.md")
}

/// Reescreve o campo `model:` no frontmatter YAML do specialist `advisor`
/// para casar com o modelo configurado na UI. O mecanismo nativo de
/// specialists do zero CLI resolve `model:` por specialist (não é algo que
/// a própria chamada da tool `Task` possa sobrescrever por invocação), então
/// é assim que o advisor passa a rodar num modelo diferente do executor.
/// `model: None` significa "usar o modelo do executor", por isso o campo é
/// removido em vez de deixado vazio. Não faz nada silenciosamente se o
/// arquivo do specialist ainda não existir - isso é só uma sincronização de
/// conveniência, não o que faz o advisor mode funcionar (isso é
/// `executor_instruction_prompt` + a tool `Task` nativa).
pub fn sync_specialist_model(workspace_root: &Path, model: Option<&str>) -> Result<(), String> {
    let path = specialist_path(workspace_root);
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Ok(());
    };

    let updated = set_frontmatter_model(&content, model);
    if updated == content {
        return Ok(());
    }
    std::fs::write(&path, updated).map_err(|e| e.to_string())
}

/// Transformação pura de string (sem tocar disco) para ser testável
/// isoladamente. Assume frontmatter YAML delimitado por uma linha `---` no
/// início do arquivo e a próxima linha `---` depois dela.
fn set_frontmatter_model(content: &str, model: Option<&str>) -> String {
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    if lines.first().map(|s| s.as_str()) != Some("---") {
        return content.to_string();
    }
    let Some(rel_end) = lines[1..].iter().position(|l| l == "---") else {
        return content.to_string();
    };
    let end = rel_end + 1;

    let model_idx = lines[1..end]
        .iter()
        .position(|l| l.trim_start().starts_with("model:"))
        .map(|i| i + 1);

    match (model, model_idx) {
        (Some(m), Some(idx)) => lines[idx] = format!("model: \"{m}\""),
        (Some(m), None) => lines.insert(end, format!("model: \"{m}\"")),
        (None, Some(idx)) => {
            lines.remove(idx);
        }
        (None, None) => {}
    }

    let mut result = lines.join("\n");
    if content.ends_with('\n') {
        result.push('\n');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_instruction_prompt_disabled() {
        let config = AdvisorConfig {
            enabled: false,
            model: None,
        };
        assert!(executor_instruction_prompt(&config).is_none());
    }

    #[test]
    fn test_executor_instruction_prompt_enabled_no_model() {
        let config = AdvisorConfig {
            enabled: true,
            model: None,
        };
        let prompt = executor_instruction_prompt(&config).unwrap();
        assert!(prompt.contains("advisor_mode"));
        assert!(prompt.contains("Task"));
        assert!(prompt.contains("advisor"));
    }

    #[test]
    fn test_executor_instruction_prompt_enabled_with_model() {
        let config = AdvisorConfig {
            enabled: true,
            model: Some("claude-opus-4-1".to_string()),
        };
        let prompt = executor_instruction_prompt(&config).unwrap();
        assert!(prompt.contains("claude-opus-4-1"));
    }

    #[test]
    fn test_is_advisor_consultation() {
        let args = serde_json::json!({
            "name": "advisor",
            "prompt": "Analise este código"
        });
        assert!(is_advisor_consultation("Task", &args));
        assert!(!is_advisor_consultation("Task", &serde_json::json!({"name": "explorer"})));
        assert!(!is_advisor_consultation("read_file", &args));
    }

    #[test]
    fn test_extract_advisor_prompt() {
        let args = serde_json::json!({
            "name": "advisor",
            "prompt": "Analise este código"
        });
        assert_eq!(extract_advisor_prompt(&args), Some("Analise este código".to_string()));
    }

    const SPECIALIST_NO_MODEL: &str = "---\nname: \"advisor\"\ndescription: \"desc\"\ntools:\n  - \"read-only\"\n---\n\nBody text.\n";

    #[test]
    fn test_set_frontmatter_model_inserts_when_absent() {
        let result = set_frontmatter_model(SPECIALIST_NO_MODEL, Some("claude-opus-4-1"));
        assert!(result.contains("model: \"claude-opus-4-1\""));
        assert!(result.contains("name: \"advisor\""), "keeps other frontmatter fields");
        assert!(result.contains("Body text."), "keeps the body untouched");
        // The inserted field must land inside the frontmatter block, before
        // the closing `---`, not after it (which would silently do nothing).
        let closing_marker_pos = result.match_indices("---").nth(1).unwrap().0;
        let model_pos = result.find("model:").unwrap();
        assert!(model_pos < closing_marker_pos);
    }

    #[test]
    fn test_set_frontmatter_model_replaces_existing() {
        let with_model = set_frontmatter_model(SPECIALIST_NO_MODEL, Some("model-a"));
        let replaced = set_frontmatter_model(&with_model, Some("model-b"));
        assert!(replaced.contains("model: \"model-b\""));
        assert!(!replaced.contains("model-a"));
        // No duplicate `model:` lines left behind.
        assert_eq!(replaced.matches("model:").count(), 1);
    }

    #[test]
    fn test_set_frontmatter_model_none_removes_existing() {
        let with_model = set_frontmatter_model(SPECIALIST_NO_MODEL, Some("claude-opus-4-1"));
        let removed = set_frontmatter_model(&with_model, None);
        assert!(!removed.contains("model:"));
        assert_eq!(removed, SPECIALIST_NO_MODEL);
    }

    #[test]
    fn test_set_frontmatter_model_none_when_absent_is_noop() {
        let result = set_frontmatter_model(SPECIALIST_NO_MODEL, None);
        assert_eq!(result, SPECIALIST_NO_MODEL);
    }

    #[test]
    fn test_set_frontmatter_model_no_frontmatter_returns_unchanged() {
        let content = "Just a plain file, no frontmatter.\n";
        assert_eq!(set_frontmatter_model(content, Some("x")), content);
    }

    #[test]
    fn test_sync_specialist_model_missing_file_is_noop() {
        let dir = std::env::temp_dir().join(format!("advisor-sync-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(sync_specialist_model(&dir, Some("x")).is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_sync_specialist_model_writes_through_to_disk() {
        let dir = std::env::temp_dir().join(format!("advisor-sync-write-test-{}", std::process::id()));
        let specialist_dir = dir.join(".zero").join("specialists");
        std::fs::create_dir_all(&specialist_dir).unwrap();
        let path = specialist_dir.join("advisor.md");
        std::fs::write(&path, SPECIALIST_NO_MODEL).unwrap();

        sync_specialist_model(&dir, Some("gpt-5")).unwrap();
        let updated = std::fs::read_to_string(&path).unwrap();
        assert!(updated.contains("model: \"gpt-5\""));

        sync_specialist_model(&dir, None).unwrap();
        let cleared = std::fs::read_to_string(&path).unwrap();
        assert!(!cleared.contains("model:"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
