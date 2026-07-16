# Advisor Mode

Advisor Mode enables a dual-model pattern where an **executor** (the configured model) performs the main work, and an **advisor** (a more capable model) is consulted for critical decisions. The advisor operates in **read-only** mode — it only analyzes and recommends, never modifies code.

## Overview

When Advisor Mode is enabled for a session:

1. The executor receives instructions to consult the advisor for architectural decisions, design patterns, and critical code reviews
2. The executor uses the `Task` tool to delegate consultation to the `advisor` specialist
3. The advisor analyzes the context and returns recommendations
4. The executor considers the advice before implementing changes

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Chat Panel                              │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Toggle: [ON/OFF]  │ Model: [Opus ▼]                 │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  User: "Refactor this function"                             │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ 🔮 Advisor Consultation                             │    │
│  │ "Analyzed the code. Recommendations: 1) Extract...  │    │
│  │  2) Rename... 3) Add tests..."                      │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
│  Assistant: "Implementing based on advisor advice..."       │
└─────────────────────────────────────────────────────────────┘
```

## Implementation Details

### Backend (Rust)

**File: `src-tauri/src/advisor.rs`**

- `AdvisorConfig` struct: manages enabled state and model preference
- `executor_instruction_prompt()`: generates system prompt injection for the executor
- `is_advisor_consultation()`: detects advisor tool calls in the event flow
- `extract_advisor_prompt()`: extracts the consultation prompt from tool arguments

**File: `src-tauri/src/lib.rs`**

Tauri commands:

- `get_advisor_config`: returns global advisor configuration
- `set_advisor_config`: saves global advisor configuration
- `toggle_advisor`: enables/disables advisor globally
- `set_advisor_model`: sets the advisor model globally
- `get_session_advisor_config`: returns advisor config for a specific session
- `set_session_advisor_config`: updates advisor config for a specific session

**File: `src-tauri/src/bridge.rs`**

- `AcpSession.advisor_config`: per-session advisor configuration
- `get_advisor_config()`: retrieves session advisor config
- `set_advisor_config()`: updates session advisor config

### Frontend (Vue/Pinia)

**File: `src/stores/zero-session-store.js`**

State:

- `advisorEnabled`: whether advisor mode is active for this session
- `advisorModel`: the model used by the advisor (null = default)

Actions:

- `toggleAdvisor(enabled)`: enables/disables advisor for this session
- `setAdvisorModel(model)`: sets the advisor model
- `_loadAdvisorConfig()`: loads config from backend on session start

**File: `src/components/chat/ChatInput.vue`**

- Toggle button "🔮 Advisor" in the input bar
- Visual indicator: green when active, gray when inactive
- Tooltip: "Toggle advisor mode for technical consultations"

**File: `src/services/zero.js`**

API functions:

- `getAdvisorConfig()`: fetches global advisor config
- `setAdvisorConfig(config)`: saves global advisor config
- `toggleAdvisor(enabled)`: toggles advisor globally
- `setAdvisorModel(model)`: sets advisor model globally
- `getSessionAdvisorConfig(key)`: fetches session advisor config
- `setSessionAdvisorConfig(key, config)`: updates session advisor config

### Specialist System

**File: `.zero/specialists/advisor.md`**

The advisor is implemented as a Zero specialist:

```markdown
---
name: "advisor"
description: "Technical senior advisor for architectural and design decisions."
tools:
  - "read-only"
---

You are a senior technical advisor with deep experience in software architecture,
design patterns, and development best practices.
```

The specialist:

- Has read-only access to the workspace
- Analyzes context and returns structured recommendations
- Never modifies code directly
- Uses a structured format: Analysis, Recommendations, Risks, Alternatives

## Usage

### Enabling Advisor Mode

1. Click the "🔮 Advisor" toggle in the chat input bar
2. The toggle turns green when active
3. The executor will now consult the advisor for critical decisions

### How It Works

When the executor encounters:

- Architectural decisions
- Design pattern selection
- Complex implementation choices
- Critical code reviews

It will automatically use the `Task` tool to consult the advisor:

```json
{
  "name": "advisor",
  "prompt": "<relevant context for analysis>",
  "description": "Technical consultation"
}
```

### Viewing Consultations

Advisor consultations appear as special blocks in the chat:

- Purple/gold border
- "🔮 Advisor Consultation" header
- Structured analysis with recommendations

## Configuration

### Global Configuration

Stored in `~/.local/share/zero-desktop/advisor-config.json`:

```json
{
  "enabled": false,
  "model": null
}
```

### Per-Session Configuration

Each session maintains its own advisor state, allowing different sessions to have different advisor settings.

## Model Selection

The advisor can use any model available through the configured provider:

- **Same provider as executor**: uses the same API key and provider
- **Different provider**: can be configured to use a different provider for the advisor
- **Model override**: the advisor model can be specified independently

## Cost Considerations

- Each advisor consultation creates a new API call
- The advisor uses the configured model, which may have different pricing
- No built-in limits on consultations (cost is the user's responsibility)
- Consider using a cost-effective model for the advisor if consultations are frequent

## Future Enhancements

### Phase 2: Native Tool Integration

A future enhancement could add native `consult_advisor` tool support in Zero CLI:

- More efficient than specialist delegation (no process spawning)
- Lower latency
- Better integration with the agent loop

### Phase 3: Advanced Features

- Consultation history and analytics
- Cost tracking per session
- Model performance comparison
- Automatic advisor activation based on task complexity

## Testing

### Unit Tests

Run Rust tests:

```bash
cd src-tauri && cargo test
```

### Integration Tests

1. Enable advisor mode in a session
2. Send a message requiring architectural decisions
3. Verify the executor consults the advisor
4. Verify the consultation appears in the chat
5. Verify the executor considers the advice

### Test Checklist

- [ ] Toggle enables/disables advisor mode
- [ ] Advisor consultation appears in chat
- [ ] Executor uses Task tool with advisor specialist
- [ ] Advisor returns structured recommendations
- [ ] Configuration persists across sessions
- [ ] Model selection works correctly
- [ ] No impact on sessions with advisor disabled

## Troubleshooting

### Advisor Not Consulted

1. Verify advisor mode is enabled (toggle is green)
2. Check that the message requires architectural decisions
3. Verify the advisor specialist exists: `zero specialist list`
4. Check backend logs for errors

### Consultation Not Appearing

1. Check frontend console for errors
2. Verify the Task tool call is being made
3. Check if the specialist is returning results

### Model Errors

1. Verify the advisor model is available
2. Check API key configuration
3. Verify provider connectivity

## Related Documentation

- [Zero Bridge](./zero-bridge.md) - Backend architecture
- [Session System](./session-system.md) - Session management
- [Model Switching](./model-switching.md) - Model configuration
- [Chat Interface](./chat-interface.md) - UI components
