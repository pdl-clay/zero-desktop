# Modo Advisor

O Modo Advisor permite um padrão dual-model onde um **executor** (o modelo configurado) realiza o trabalho principal, e um **advisor** (modelo mais inteligente) é consultado para decisões críticas. O advisor opera em modo **read-only** - apenas analisa e recomenda, nunca modifica código.

## Visão Geral

Quando o Modo Advisor está ativado para uma sessão:

1. O executor recebe instruções para consultar o advisor em decisões arquiteturais, design patterns, e reviews críticos de código
2. O executor usa a tool `Task` para delegar consultoria ao specialist `advisor`
3. O advisor analisa o contexto e retorna recomendações
4. O executor considera o conselho antes de implementar mudanças

## Arquitetura

```
┌─────────────────────────────────────────────────────────────┐
│                     Painel de Chat                          │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Toggle: [ON/OFF]  │ Modelo: [Opus ▼]                 │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  Usuário: "Refatore esta função"                           │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ 🔮 Consulta ao Advisor                              │    │
│  │ "Analisei o código. Recomendações: 1) Extrair...    │    │
│  │  2) Renomear... 3) Adicionar testes..."             │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
│  Assistente: "Implementando baseado no conselho..."         │
└─────────────────────────────────────────────────────────────┘
```

## Detalhes de Implementação

### Backend (Rust)

**Arquivo: `src-tauri/src/advisor.rs`**

- Struct `AdvisorConfig`: gerencia estado ativado e preferência de modelo
- `executor_instruction_prompt()`: gera injeção de system prompt para o executor
- `is_advisor_consultation()`: detecta chamadas de tool do advisor no fluxo de eventos
- `extract_advisor_prompt()`: extrai o prompt de consultoria dos argumentos da tool

**Arquivo: `src-tauri/src/lib.rs`**

Comandos Tauri:

- `get_advisor_config`: retorna configuração global do advisor
- `set_advisor_config`: salva configuração global do advisor
- `toggle_advisor`: ativa/desativa advisor globalmente
- `set_advisor_model`: define o modelo do advisor globalmente
- `get_session_advisor_config`: retorna config do advisor para uma sessão específica
- `set_session_advisor_config`: atualiza config do advisor para uma sessão específica

**Arquivo: `src-tauri/src/bridge.rs`**

- `AcpSession.advisor_config`: configuração do advisor por sessão
- `get_advisor_config()`: recupera config do advisor da sessão
- `set_advisor_config()`: atualiza config do advisor da sessão

### Frontend (Vue/Pinia)

**Arquivo: `src/stores/zero-session-store.js`**

State:

- `advisorEnabled`: se o modo advisor está ativo para esta sessão
- `advisorModel`: modelo usado pelo advisor (null = padrão)

Actions:

- `toggleAdvisor(enabled)`: ativa/desativa advisor para esta sessão
- `setAdvisorModel(model)`: define o modelo do advisor
- `_loadAdvisorConfig()`: carrega config do backend ao iniciar sessão

**Arquivo: `src/components/chat/ChatInput.vue`**

- Botão toggle "🔮 Advisor" na barra de entrada
- Indicador visual: verde quando ativo, cinza quando inativo
- Tooltip: "Ativar/desativar modo advisor para consultas técnicas"

**Arquivo: `src/services/zero.js`**

Funções de API:

- `getAdvisorConfig()`: busca config global do advisor
- `setAdvisorConfig(config)`: salva config global do advisor
- `toggleAdvisor(enabled)`: alterna advisor globalmente
- `setAdvisorModel(model)`: define modelo do advisor globalmente
- `getSessionAdvisorConfig(key)`: busca config do advisor da sessão
- `setSessionAdvisorConfig(key, config)`: atualiza config do advisor da sessão

### Sistema de Specialists

**Arquivo: `.zero/specialists/advisor.md`**

O advisor é implementado como um specialist do Zero:

```markdown
---
name: "advisor"
description: "Consultor técnico sênior para decisões arquiteturais e de design."
tools:
  - "read-only"
---

Você é um consultor técnico sênior com profunda experiência em arquitetura de software,
design patterns, e boas práticas de desenvolvimento.
```

O specialist:

- Tem acesso read-only ao workspace
- Analisa contexto e retorna recomendações estruturadas
- Nunca modifica código diretamente
- Usa formato estruturado: Análise, Recomendações, Riscos, Alternativas

## Uso

### Ativando o Modo Advisor

1. Clique no toggle "🔮 Advisor" na barra de entrada do chat
2. O toggle fica verde quando ativo
3. O executor agora consultará o advisor para decisões críticas

### Como Funciona

Quando o executor encontra:

- Decisões arquiteturais
- Seleção de design patterns
- Escolhas de implementação complexas
- Reviews críticos de código

Ele automaticamente usará a tool `Task` para consultar o advisor:

```json
{
  "name": "advisor",
  "prompt": "<contexto relevante para análise>",
  "description": "Consultoria técnica"
}
```

### Visualizando Consultas

Consultas ao advisor aparecem como blocos especiais no chat:

- Borda dourada/roxa
- Cabeçalho "🔮 Consulta ao Advisor"
- Análise estruturada com recomendações

## Configuração

### Configuração Global

Armazenada em `~/.local/share/zero-desktop/advisor-config.json`:

```json
{
  "enabled": false,
  "model": null
}
```

### Configuração Por Sessão

Cada sessão mantém seu próprio estado de advisor, permitindo que diferentes sessões tenham diferentes configurações.

## Seleção de Modelo

O advisor pode usar qualquer modelo disponível através do provider configurado:

- **Mesmo provider do executor**: usa a mesma chave API e provider
- **Provider diferente**: pode ser configurado para usar um provider diferente para o advisor
- **Override de modelo**: o modelo do advisor pode ser especificado independentemente

## Considerações de Custo

- Cada consulta ao advisor cria uma nova chamada de API
- O advisor usa o modelo configurado, que pode ter preços diferentes
- Sem limites embutidos de consultas (o custo é responsabilidade do usuário)
- Considere usar um modelo econômico para o advisor se as consultas forem frequentes

## Melhorias Futuras

### Fase 2: Integração Nativa com Tool

Um aprimoramento futuro poderia adicionar suporte nativo à tool `consult_advisor` no Zero CLI:

- Mais eficiente que delegação via specialist (sem spawning de processo)
- Menor latência
- Melhor integração com o loop do agente

### Fase 3: Recursos Avançados

- Histórico e análises de consultas
- Rastreamento de custo por sessão
- Comparação de performance de modelos
- Ativação automática do advisor baseada na complexidade da tarefa

## Testes

### Testes Unitários

Execute testes Rust:

```bash
cd src-tauri && cargo test
```

### Testes de Integração

1. Ative o modo advisor em uma sessão
2. Envie uma mensagem que requer decisões arquiteturais
3. Verifique se o executor consulta o advisor
4. Verifique se a consulta aparece no chat
5. Verifique se o executor considera o conselho

### Checklist de Testes

- [ ] Toggle ativa/desativa modo advisor
- [ ] Consulta ao advisor aparece no chat
- [ ] Executor usa tool Task com specialist advisor
- [ ] Advisor retorna recomendações estruturadas
- [ ] Configuração persiste entre sessões
- [ ] Seleção de modelo funciona corretamente
- [ ] Sem impacto em sessões com advisor desativado

## Solução de Problemas

### Advisor Não Consultado

1. Verifique se o modo advisor está ativado (toggle verde)
2. Verifique se a mensagem requer decisões arquiteturais
3. Verifique se o specialist advisor existe: `zero specialist list`
4. Verifique logs do backend para erros

### Consulta Não Aparecendo

1. Verifique console do frontend para erros
2. Verifique se a chamada da tool Task está sendo feita
3. Verifique se o specialist está retornando resultados

### Erros de Modelo

1. Verifique se o modelo do advisor está disponível
2. Verifique configuração da chave API
3. Verifique conectividade do provider

## Documentação Relacionada

- [Zero Bridge](./zero-bridge.md) - Arquitetura do backend
- [Sistema de Sessões](./session-system.md) - Gerenciamento de sessões
- [Troca de Modelo](./model-switching.md) - Configuração de modelo
- [Interface de Chat](./chat-interface.md) - Componentes de UI
