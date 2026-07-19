use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Regime de gatilho para consultas ao advisor.
///
/// `Max` é o comportamento original: o executor consulta proativamente em
/// qualquer uma de cinco categorias amplas (arquitetura, decisões complexas,
/// review crítico, performance, segurança). `Low` é modelado no "Advisor
/// Mode" da StepFun para o Step 3.7 Flash (dois gatilhos estreitos:
/// planejamento inicial de alto risco e recuperação de falha repetida) -
/// StepFun reporta 97% da qualidade do Claude Opus 4.6 a ~1/9 do custo por
/// tarefa com esse regime mais seletivo. `Max` continua sendo o default para
/// não mudar o comportamento de configs já salvas sem esse campo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AdvisorMode {
    #[default]
    Max,
    Low,
}

/// Configuração do Advisor Mode para uma sessão.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdvisorConfig {
    /// Se o advisor está ativado para esta sessão.
    pub enabled: bool,
    /// Modelo a ser usado pelo advisor (None = usar modelo do executor).
    pub model: Option<String>,
    /// Regime de gatilho (proativo/amplo vs restritivo). `#[serde(default)]`
    /// porque configs salvas antes deste campo existir não o têm no JSON.
    #[serde(default)]
    pub mode: AdvisorMode,
}

/// Caminho para o arquivo de configuração global do advisor.
fn advisor_config_path() -> Result<PathBuf, String> {
    let base =
        dirs::data_dir().ok_or_else(|| "Could not resolve app data directory".to_string())?;
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
///
/// Deixa explícito que o specialist JÁ EXISTE em disco (criado/atualizado por
/// `advisor::sync_specialist_model` antes desta mensagem ser enviada - ver
/// `ZeroBridge::set_advisor_config`) e que o modelo já está configurado no
/// próprio arquivo, sem precisar ser passado na chamada. Sem isso, o
/// executor rotineiramente checava `.zero/specialists/` via
/// `list_directory`/`glob` primeiro (que não mostram dotfiles), concluía
/// que o specialist não existia, e tentava criá-lo via `GenerateSpecialist`,
/// na melhor das hipóteses um turno desperdiçado ("specialist already
/// exists: advisor"), na pior um `overwrite: true` que apagava o `model:`
/// configurado pelo usuário.
///
/// Também deixa explícito que a consulta é PROATIVA - por iniciativa do
/// próprio executor, sem precisar que o usuário peça "consulte o advisor"
/// com essas palavras - já que "quando precisar de orientação" sozinho
/// deixava ambíguo se isso valia sem pedido explícito. Isso é balanceado
/// com regras de eficiência (uma consulta por decisão, com contexto
/// completo, não para tarefas triviais) para não virar uma consulta a cada
/// tool call - cada uma é uma chamada de API real, com custo e latência.
///
/// Delega pro corpo específico do modo (`Max`/`Low`) - ver
/// `max_mode_instructions`/`low_mode_instructions` - o texto de
/// enquadramento ao redor (tag `<advisor_mode>`, bloco JSON da tool `Task`,
/// aviso de que o specialist já existe) é idêntico nos dois modos.
pub fn executor_instruction_prompt(config: &AdvisorConfig) -> Option<String> {
    if !config.enabled {
        return None;
    }

    let trigger_rules = match config.mode {
        AdvisorMode::Max => max_mode_trigger_rules(),
        AdvisorMode::Low => low_mode_trigger_rules(),
    };

    Some(format!(
        r#"

<advisor_mode>
{trigger_rules}

Use a tool `Task`:

```json
{{
  "name": "advisor",
  "prompt": "<contexto relevante para análise>",
  "description": "Consultoria técnica"
}}
```

O specialist `advisor` JÁ EXISTE e já está configurado com o modelo certo -
não verifique se ele existe (list_directory/glob não mostram
`.zero/specialists/`, que é um diretório oculto, então "não encontrei" NÃO
significa que ele não existe) e não use `GenerateSpecialist` para criá-lo ou
recriá-lo. Chame `Task` diretamente.

Forneça contexto suficiente para que o advisor possa dar recomendações
precisas. Inclua: código relevante, restrições do projeto, e o que
especificamente precisa de orientação.

O advisor retornará análise e recomendações que você deve considerar antes de
implementar. Não ignore as recomendações do advisor sem justificativa clara.
</advisor_mode>
"#
    ))
}

/// Regras de gatilho do modo `Max`: proativo e amplo, cinco categorias.
/// Comportamento original do advisor mode, preservado como está.
fn max_mode_trigger_rules() -> &'static str {
    r#"O modo Advisor está ATIVADO (modo Max). Consulte o specialist `advisor`
PROATIVAMENTE, por sua própria iniciativa, sempre que a tarefa envolver:
- Arquitetura de software e design patterns
- Decisões de implementação complexas (múltiplas abordagens viáveis, trade-offs não óbvios)
- Review de código crítico antes de aplicar mudanças de alto impacto
- Otimização de performance
- Segurança e boas práticas

Não espere o usuário pedir uma consulta explicitamente ("consulte o
advisor", "peça uma segunda opinião" etc.) - se a tarefa se encaixa numa
dessas categorias, consulte por conta própria antes de implementar.

Seja eficiente - cada consulta é uma chamada de API real, com custo e
latência reais:
- Uma consulta por decisão, com contexto completo, em vez de várias
  consultas fragmentadas sobre a mesma coisa.
- Não consulte para tarefas triviais, mudanças óbvias, ou quando você já
  tem certeza da resposta correta.
- Reserve consultas para decisões de fato não triviais ou de alto impacto."#
}

/// Regras de gatilho do modo `Low`: restritivo, modelado no "Advisor Mode"
/// que a StepFun publicou para o Step 3.7 Flash - dois gatilhos estreitos e
/// reativos (planejamento de alto risco, recuperação de falha repetida) em
/// vez de categorias temáticas amplas. A régua não é "esse assunto importa",
/// é "estou prestes a tomar uma decisão cara de reverter" ou "já tentei e
/// falhei mais de uma vez". Deliberadamente sem a lista de cinco categorias
/// do modo Max: se ela aparecesse aqui também, o executor voltaria a tratar
/// qualquer tópico de arquitetura/segurança como gatilho, e o modo Low
/// deixaria de ser mais restritivo na prática.
fn low_mode_trigger_rules() -> &'static str {
    r#"O modo Advisor está ATIVADO (modo Low - restritivo). Consulte o specialist
`advisor` APENAS nestas duas situações:

1. **Planejamento inicial de alto risco** - antes de começar a implementar
   uma mudança arquitetural, de segurança ou de concorrência não-trivial,
   quando várias abordagens são viáveis e escolher errado seria caro de
   reverter depois.
2. **Recuperação de falha repetida** - se a mesma abordagem falhou 2 ou mais
   vezes seguidas (mesmo erro, mesmo teste quebrando, mesmo loop sem
   progresso), pare e consulte antes de tentar de novo do mesmo jeito.

Fora dessas duas situações, NÃO consulte - implemente direto. Isso vale
mesmo para tarefas de arquitetura, performance ou segurança que você já sabe
resolver de primeira: a régua aqui não é "esse assunto é importante", é
"estou prestes a tomar uma decisão cara de errar" ou "já tentei e falhei".

Quando consultar, seja eficiente - cada consulta é uma chamada de API real,
com custo e latência reais: uma consulta por decisão, com contexto
completo, não várias fragmentadas sobre a mesma coisa."#
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
    workspace_root
        .join(".zero")
        .join("specialists")
        .join("advisor.md")
}

/// Template do specialist `advisor`, usado para CRIAR
/// `.zero/specialists/advisor.md` num workspace que ainda não tem um.
/// Embutido no binário (não em `.zero/`, que é ignorado pelo git em todo
/// workspace por convenção do zero CLI) porque advisor mode inteiro depende
/// do loader de specialists do zero CLI achar esse arquivo - sem isso, todo
/// workspace que não teve esse arquivo criado manualmente falha com
/// `specialist "advisor" not found` assim que o executor tenta delegar via
/// `Task`.
const ADVISOR_SPECIALIST_TEMPLATE: &str = include_str!("../resources/specialists/advisor.md");

/// Garante que `.zero/specialists/advisor.md` existe no workspace (criando
/// a partir do template embutido se faltar) e reescreve o campo `model:` no
/// seu frontmatter YAML para casar com o modelo configurado na UI. O
/// mecanismo nativo de specialists do zero CLI resolve `model:` por
/// specialist (não é algo que a própria chamada da tool `Task` possa
/// sobrescrever por invocação), então é assim que o advisor passa a rodar
/// num modelo diferente do executor. `model: None` significa "usar o
/// modelo do executor", por isso o campo é removido em vez de deixado
/// vazio.
pub fn sync_specialist_model(workspace_root: &Path, model: Option<&str>) -> Result<(), String> {
    let path = specialist_path(workspace_root);
    let (content, existed) = match std::fs::read_to_string(&path) {
        Ok(content) => (content, true),
        Err(_) => (ADVISOR_SPECIALIST_TEMPLATE.to_string(), false),
    };

    let updated = set_frontmatter_model(&content, model);
    if existed && updated == content {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
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
            mode: AdvisorMode::Max,
        };
        assert!(executor_instruction_prompt(&config).is_none());
    }

    #[test]
    fn test_executor_instruction_prompt_enabled_no_model() {
        let config = AdvisorConfig {
            enabled: true,
            model: None,
            mode: AdvisorMode::Max,
        };
        let prompt = executor_instruction_prompt(&config).unwrap();
        assert!(prompt.contains("advisor_mode"));
        assert!(prompt.contains("Task"));
        assert!(prompt.contains("advisor"));
    }

    #[test]
    fn test_executor_instruction_prompt_enabled_with_model_does_not_repeat_it() {
        // The model lives in the specialist file's own `model:` frontmatter
        // (see advisor::sync_specialist_model), not in the prompt text - the
        // executor never needs to know or pass it. This also guards against
        // reintroducing a model hint that could go stale relative to
        // whatever the specialist file was last synced to.
        let config = AdvisorConfig {
            enabled: true,
            model: Some("claude-opus-4-1".to_string()),
            mode: AdvisorMode::Max,
        };
        let prompt = executor_instruction_prompt(&config).unwrap();
        assert!(!prompt.contains("claude-opus-4-1"));
    }

    #[test]
    fn test_executor_instruction_prompt_tells_executor_specialist_already_exists() {
        let config = AdvisorConfig {
            enabled: true,
            model: None,
            mode: AdvisorMode::Max,
        };
        let prompt = executor_instruction_prompt(&config).unwrap();
        assert!(prompt.contains("JÁ EXISTE"));
        assert!(prompt.contains("GenerateSpecialist"));
    }

    #[test]
    fn test_executor_instruction_prompt_tells_executor_to_consult_proactively() {
        // Regression test: "quando precisar de orientação" alone left it
        // ambiguous whether consulting required an explicit user request
        // ("consulte o advisor"). The executor must know it can and should
        // consult on its own initiative.
        let config = AdvisorConfig {
            enabled: true,
            model: None,
            mode: AdvisorMode::Max,
        };
        let prompt = executor_instruction_prompt(&config).unwrap();
        assert!(prompt.contains("PROATIVAMENTE"));
        assert!(prompt.contains("Não espere o usuário pedir"));
    }

    #[test]
    fn test_executor_instruction_prompt_includes_efficiency_guidance() {
        // Proactive consultation without any counterweight would turn into
        // a Task call on every non-trivial tool use - each one is a real,
        // possibly-more-expensive API call. The prompt must tell the
        // executor to consult sparingly, not liberally.
        let config = AdvisorConfig {
            enabled: true,
            model: None,
            mode: AdvisorMode::Max,
        };
        let prompt = executor_instruction_prompt(&config).unwrap();
        assert!(prompt.contains("Seja eficiente"));
        assert!(prompt.contains("Não consulte para tarefas triviais"));
    }

    #[test]
    fn test_low_mode_default_is_max_for_backward_compatibility() {
        // A saved AdvisorConfig from before `mode` existed deserializes with
        // no "mode" key in its JSON - #[serde(default)] must resolve that to
        // Max, not Low, so a pre-existing user's advisor keeps behaving the
        // same way it did before this field was introduced.
        let deserialized: AdvisorConfig =
            serde_json::from_str(r#"{"enabled":true,"model":null}"#).unwrap();
        assert_eq!(deserialized.mode, AdvisorMode::Max);
    }

    #[test]
    fn test_low_mode_prompt_has_only_two_narrow_triggers() {
        let config = AdvisorConfig {
            enabled: true,
            model: None,
            mode: AdvisorMode::Low,
        };
        let prompt = executor_instruction_prompt(&config).unwrap();
        assert!(prompt.contains("modo Low"));
        assert!(prompt.contains("Planejamento inicial de alto risco"));
        assert!(prompt.contains("Recuperação de falha repetida"));
        // Deliberately does NOT list the Max mode's five broad categories -
        // if "Segurança e boas práticas" (etc.) leaked in here too, the
        // executor would treat any security-adjacent topic as a trigger,
        // same as Max, defeating the point of a more restrictive mode.
        assert!(!prompt.contains("Segurança e boas práticas"));
        assert!(!prompt.contains("Arquitetura de software e design patterns"));
    }

    #[test]
    fn test_low_mode_prompt_still_explains_task_tool_and_specialist_exists() {
        // The framing shared with Max mode (how to call Task, that the
        // specialist file already exists) must survive the split - Low mode
        // isn't a different mechanism, just a different trigger rule.
        let config = AdvisorConfig {
            enabled: true,
            model: None,
            mode: AdvisorMode::Low,
        };
        let prompt = executor_instruction_prompt(&config).unwrap();
        assert!(prompt.contains("\"name\": \"advisor\""));
        assert!(prompt.contains("JÁ EXISTE"));
        assert!(prompt.contains("GenerateSpecialist"));
    }

    #[test]
    fn test_max_and_low_mode_prompts_differ() {
        let max = executor_instruction_prompt(&AdvisorConfig {
            enabled: true,
            model: None,
            mode: AdvisorMode::Max,
        })
        .unwrap();
        let low = executor_instruction_prompt(&AdvisorConfig {
            enabled: true,
            model: None,
            mode: AdvisorMode::Low,
        })
        .unwrap();
        assert_ne!(max, low);
    }

    #[test]
    fn test_is_advisor_consultation() {
        let args = serde_json::json!({
            "name": "advisor",
            "prompt": "Analise este código"
        });
        assert!(is_advisor_consultation("Task", &args));
        assert!(!is_advisor_consultation(
            "Task",
            &serde_json::json!({"name": "explorer"})
        ));
        assert!(!is_advisor_consultation("read_file", &args));
    }

    #[test]
    fn test_extract_advisor_prompt() {
        let args = serde_json::json!({
            "name": "advisor",
            "prompt": "Analise este código"
        });
        assert_eq!(
            extract_advisor_prompt(&args),
            Some("Analise este código".to_string())
        );
    }

    const SPECIALIST_NO_MODEL: &str = "---\nname: \"advisor\"\ndescription: \"desc\"\ntools:\n  - \"read-only\"\n---\n\nBody text.\n";

    #[test]
    fn test_set_frontmatter_model_inserts_when_absent() {
        let result = set_frontmatter_model(SPECIALIST_NO_MODEL, Some("claude-opus-4-1"));
        assert!(result.contains("model: \"claude-opus-4-1\""));
        assert!(
            result.contains("name: \"advisor\""),
            "keeps other frontmatter fields"
        );
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
    fn test_sync_specialist_model_creates_file_from_template_when_missing() {
        // Regression test: this used to be a silent no-op when the file
        // didn't exist, which only "worked" in a workspace where someone
        // had manually created .zero/specialists/advisor.md by hand - every
        // other workspace hit `specialist "advisor" not found` the moment
        // the executor tried to delegate, since .zero/ is gitignored and
        // nothing ever created the file.
        let dir =
            std::env::temp_dir().join(format!("advisor-sync-create-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".zero").join("specialists").join("advisor.md");
        assert!(
            !path.is_file(),
            "test assumes the specialist file doesn't exist yet"
        );

        sync_specialist_model(&dir, None).unwrap();

        let created = std::fs::read_to_string(&path).expect("advisor.md should have been created");
        assert!(
            created.contains("name: \"advisor\""),
            "created file is a valid specialist manifest"
        );
        assert!(
            !created.contains("model:"),
            "no model field when model is None"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_sync_specialist_model_creates_file_with_model_when_missing() {
        let dir = std::env::temp_dir().join(format!(
            "advisor-sync-create-model-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".zero").join("specialists").join("advisor.md");

        sync_specialist_model(&dir, Some("gpt-5")).unwrap();

        let created = std::fs::read_to_string(&path).expect("advisor.md should have been created");
        assert!(created.contains("name: \"advisor\""));
        assert!(created.contains("model: \"gpt-5\""));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_sync_specialist_model_writes_through_to_disk() {
        let dir =
            std::env::temp_dir().join(format!("advisor-sync-write-test-{}", std::process::id()));
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
