#!/usr/bin/env python3
"""Fake `zero acp` backend for testing zero-desktop's ACP bridge without a
real LLM call. Speaks just enough JSON-RPC 2.0 over stdio to drive a plan
update sequence: initialize -> session/new -> session/prompt, emitting
session/update notifications (agent_message_chunk, plan, tool_call,
tool_call_update, plan again) before answering session/prompt.
"""
import sys
import json


def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


def notify(session_id, update):
    send({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {"sessionId": session_id, "update": update},
    })


def main():
    session_id = "fake-session-1"
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        msg = json.loads(line)
        method = msg.get("method")
        msg_id = msg.get("id")

        if method == "initialize":
            send({"jsonrpc": "2.0", "id": msg_id, "result": {"protocolVersion": 1, "agentCapabilities": {}}})

        elif method == "session/new":
            send({"jsonrpc": "2.0", "id": msg_id, "result": {"sessionId": session_id}})

        elif method == "session/prompt":
            # 1. Plan starts, all pending.
            notify(session_id, {
                "sessionUpdate": "plan",
                "entries": [
                    {"content": "Read the failing test", "status": "pending", "priority": 0},
                    {"content": "Patch the bridge", "status": "pending", "priority": 0},
                ],
            })
            # 2. Agent narrates + starts step 1.
            notify(session_id, {"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "Looking into it.\n"}})
            notify(session_id, {
                "sessionUpdate": "plan",
                "entries": [
                    {"content": "Read the failing test", "status": "in_progress", "priority": 0},
                    {"content": "Patch the bridge", "status": "pending", "priority": 0},
                ],
            })
            # 3. A tool call happens mid-plan.
            notify(session_id, {
                "sessionUpdate": "tool_call",
                "toolCallId": "call_00_1",
                "title": "read_file test.rs",
                "rawInput": {"path": "test.rs"},
            })
            notify(session_id, {
                "sessionUpdate": "tool_call_update",
                "toolCallId": "call_00_1",
                "status": "completed",
                "content": [{"content": {"type": "text", "text": "ok"}}],
            })
            # 4. Plan finishes.
            notify(session_id, {
                "sessionUpdate": "plan",
                "entries": [
                    {"content": "Read the failing test", "status": "completed", "priority": 0},
                    {"content": "Patch the bridge", "status": "completed", "priority": 0},
                ],
            })
            notify(session_id, {"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "Done."}})
            send({"jsonrpc": "2.0", "id": msg_id, "result": {"stopReason": "end_turn"}})

        elif method == "_zero/set_model":
            model = msg.get("params", {}).get("model")
            send({"jsonrpc": "2.0", "id": msg_id, "result": {"model": model}})

        elif method == "session/cancel":
            pass
        else:
            if msg_id is not None:
                send({"jsonrpc": "2.0", "id": msg_id, "error": {"code": -32601, "message": "method not found"}})


if __name__ == "__main__":
    main()
