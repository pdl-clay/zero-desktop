/**
 * Advisor Mode utilities for the frontend.
 *
 * These functions mirror the backend advisor.rs logic for
 * generating system prompt injections and detecting consultations.
 */

/**
 * Generates the system prompt injection for the executor when advisor mode is enabled.
 * @param {Object} config - AdvisorConfig with enabled and model fields
 * @returns {string|null} - The prompt injection or null if disabled
 */
export function executorInstructionPrompt(config) {
  if (!config || !config.enabled) {
    return null;
  }

  const modelHint = config.model ? ` (modelo recomendado: ${config.model})` : "";

  return `
<advisor_mode>
O modoAdvisor está ATIVADO. Quando precisar de orientação sobre:
- Arquitetura de software e design patterns
- Decisões de implementação complexas
- Review de código crítico
- Otimização de performance
- Segurança e boas práticas

Use a tool \`Task\` para consultar o specialist \`advisor\`:

\`\`\`json
{
  "name": "advisor",
  "prompt": "<contexto relevante para análise>",
  "description": "Consultoria técnica"
}
\`\`\`

Forneça contexto suficiente para que o advisor possa dar recomendações precisas.
Inclua: código relevante, restrições do projeto, e o que especificamente precisa de orientação${modelHint}.

O advisor retornará análise e recomendações que você deve considerar antes de implementar.
Não ignore as recomendações do advisor sem justificativa clara.
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
