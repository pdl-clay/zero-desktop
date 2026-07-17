/**
 * Advisor Mode utilities for the frontend.
 *
 * These functions mirror the backend advisor.rs logic for
 * generating system prompt injections and detecting consultations.
 */

/**
 * Trigger rules for advisor-mode Max: proactive and broad, five categories.
 * Original advisor mode behavior, preserved as-is.
 */
const MAX_MODE_TRIGGER_RULES = `O modo Advisor está ATIVADO (modo Max). Consulte o specialist \`advisor\`
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
- Reserve consultas para decisões de fato não triviais ou de alto impacto.`;

/**
 * Trigger rules for advisor-mode Low: restrictive, modeled on StepFun's
 * published "Advisor Mode" for Step 3.7 Flash - two narrow, reactive
 * triggers (high-risk planning, repeated-failure recovery) instead of broad
 * thematic categories. Deliberately does NOT repeat the Max mode's five
 * categories - if they leaked in here too, the executor would treat any
 * architecture/security-adjacent topic as a trigger, same as Max, and Low
 * would stop being more restrictive in practice.
 */
const LOW_MODE_TRIGGER_RULES = `O modo Advisor está ATIVADO (modo Low - restritivo). Consulte o specialist
\`advisor\` APENAS nestas duas situações:

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
completo, não várias fragmentadas sobre a mesma coisa.`;

/**
 * Generates the system prompt injection for the executor when advisor mode is enabled.
 *
 * Mirrors src-tauri/src/advisor.rs::executor_instruction_prompt - kept in
 * sync for the test suite (see tests/advisor-prompt.test.js), though the
 * Rust version is what actually gets injected at runtime (bridge.rs::send).
 *
 * Deliberately doesn't mention the advisor's model or ask the executor to
 * check whether the specialist exists: the model lives in the specialist
 * file's own `model:` frontmatter (already synced before this prompt is
 * built - see advisor.rs::sync_specialist_model), and `.zero/specialists/`
 * is a dotdir that list_directory/glob don't show, so an executor told to
 * "verify" it existed would conclude it's missing and try to (re)create it
 * via GenerateSpecialist - in the best case a wasted turn ("specialist
 * already exists: advisor"), in the worst case an `overwrite: true` that
 * wiped out the user's configured model.
 *
 * `config.mode` picks between two trigger regimes (see MAX_MODE_TRIGGER_RULES
 * / LOW_MODE_TRIGGER_RULES above); everything else (the Task call shape, the
 * "specialist already exists" warning) is identical between modes. Missing
 * or unrecognized mode defaults to "max", matching the Rust side's
 * `#[serde(default)]` on AdvisorMode - a config saved before this field
 * existed must keep behaving like it did before.
 * @param {Object} config - AdvisorConfig with enabled, model, and mode fields
 * @returns {string|null} - The prompt injection or null if disabled
 */
export function executorInstructionPrompt(config) {
  if (!config || !config.enabled) {
    return null;
  }

  const triggerRules = config.mode === "low" ? LOW_MODE_TRIGGER_RULES : MAX_MODE_TRIGGER_RULES;

  return `
<advisor_mode>
${triggerRules}

Use a tool \`Task\`:

\`\`\`json
{
  "name": "advisor",
  "prompt": "<contexto relevante para análise>",
  "description": "Consultoria técnica"
}
\`\`\`

O specialist \`advisor\` JÁ EXISTE e já está configurado com o modelo certo -
não verifique se ele existe (list_directory/glob não mostram
\`.zero/specialists/\`, que é um diretório oculto, então "não encontrei" NÃO
significa que ele não existe) e não use \`GenerateSpecialist\` para criá-lo ou
recriá-lo. Chame \`Task\` diretamente.

Forneça contexto suficiente para que o advisor possa dar recomendações
precisas. Inclua: código relevante, restrições do projeto, e o que
especificamente precisa de orientação.

O advisor retornará análise e recomendações que você deve considerar antes de
implementar. Não ignore as recomendações do advisor sem justificativa clara.
</advisor_mode>
`;
}

/**
 * Checks if a tool call is an advisor consultation.
 * @param {string} toolName - The name of the tool being called
 * @param {Object} args - The arguments passed to the tool
 * @returns {boolean} - True if this is an advisor consultation
 */
export function isAdvisorConsultation(toolName, args) {
  if (toolName !== "Task") {
    return false;
  }
  return !!(args && args.name === "advisor");
}

/**
 * Extracts the consultation prompt from advisor tool call arguments.
 * @param {Object} args - The arguments passed to the Task tool
 * @returns {string|null} - The consultation prompt or null
 */
export function extractAdvisorPrompt(args) {
  return args?.prompt || null;
}
