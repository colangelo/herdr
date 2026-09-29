#!/bin/sh
# installed by herdr
# managed by herdr; reinstalling or updating the integration overwrites this file.
# add custom hooks beside this file instead of editing it.
# HERDR_INTEGRATION_ID=claude
# HERDR_INTEGRATION_VERSION=9

set -eu

action="${1:-}"
hook_input_file="$(mktemp "${TMPDIR:-/tmp}/herdr-claude-hook.XXXXXX")" || exit 0
trap 'rm -f "$hook_input_file"' EXIT HUP INT TERM
cat >"$hook_input_file" 2>/dev/null || true

case "$action" in
  session) ;;
  *) exit 0 ;;
esac

[ "${HERDR_ENV:-}" = "1" ] || exit 0
[ -n "${HERDR_SOCKET_PATH:-}" ] || exit 0
[ -n "${HERDR_PANE_ID:-}" ] || exit 0
command -v python3 >/dev/null 2>&1 || exit 0

HERDR_ACTION="$action" HERDR_HOOK_INPUT_FILE="$hook_input_file" python3 - <<'PY'
import json
import os
import random
import re
import socket
import subprocess
import time

source = "herdr:claude"
action = os.environ.get("HERDR_ACTION", "")
pane_id = os.environ.get("HERDR_PANE_ID")
socket_path = os.environ.get("HERDR_SOCKET_PATH")
hook_input_file = os.environ.get("HERDR_HOOK_INPUT_FILE")

if not pane_id or not socket_path:
    raise SystemExit(0)

hook_input = {}
if hook_input_file:
    try:
        with open(hook_input_file, encoding="utf-8") as handle:
            content = handle.read()
        if content.strip():
            hook_input = json.loads(content)
    except Exception:
        hook_input = {}

hook_event_name = str(hook_input.get("hook_event_name") or "")
is_subagent = bool(hook_input.get("agent_id"))
if is_subagent:
    raise SystemExit(0)
if hook_event_name == "SubagentStop":
    # SubagentStop is a completion event. Older Herdr integrations mapped it
    # to durable working, but Claude recap/away-summary can emit it after the
    # main turn has already stopped. Never let it revive an idle pane.
    raise SystemExit(0)
request_id = f"{source}:{int(time.time() * 1000)}:{random.randrange(1_000_000):06d}"
report_seq = time.time_ns()
session_id = hook_input.get("session_id")
agent_session_id = session_id if isinstance(session_id, str) and session_id else None
transcript_path = hook_input.get("transcript_path")
agent_session_path = transcript_path if isinstance(transcript_path, str) and transcript_path else None
session_start_source = hook_input.get("source") if hook_event_name == "SessionStart" else None
if not isinstance(session_start_source, str) or not session_start_source:
    session_start_source = None
# The command that resumes this session as it is now: its permission mode,
# model and effort (fork issue 123). Values come from the hook input when it
# has them, then from the end of the transcript, then from the flags Claude
# itself was launched with. A value that cannot be found is left out.
RESUME_MODES = {"acceptEdits", "auto", "bypassPermissions", "default", "manual", "dontAsk", "plan"}
RESUME_EFFORTS = {"low", "medium", "high", "xhigh", "max"}
TRANSCRIPT_TAIL_BYTES = 512 * 1024
PLAIN_VALUE = re.compile(r"^[A-Za-z0-9._:/\[\]-]{1,200}$")


def plain(value):
    return value if isinstance(value, str) and PLAIN_VALUE.match(value) else None


def transcript_facts(path):
    facts = {"mode": None, "model": None, "effort": None, "saw_bypass": False}
    if not path:
        return facts
    try:
        with open(path, "rb") as handle:
            handle.seek(0, os.SEEK_END)
            start = max(0, handle.tell() - TRANSCRIPT_TAIL_BYTES)
            handle.seek(start)
            lines = handle.read().split(b"\n")
    except Exception:
        return facts
    if start > 0:
        lines = lines[1:]
    for raw in lines:
        try:
            entry = json.loads(raw)
        except Exception:
            continue
        if not isinstance(entry, dict) or entry.get("isSidechain"):
            continue
        if entry.get("type") == "user":
            mode = entry.get("permissionMode")
            if mode in RESUME_MODES:
                facts["mode"] = mode
                facts["saw_bypass"] |= mode == "bypassPermissions"
        elif entry.get("type") == "assistant":
            message = entry.get("message")
            model = plain(message.get("model")) if isinstance(message, dict) else None
            if model:
                facts["model"] = model
            if entry.get("effort") in RESUME_EFFORTS:
                facts["effort"] = entry["effort"]
    return facts


def claude_launch_flags():
    # Claude runs this hook as its child, sometimes through a shell: walk up to
    # the Claude process and read the flags it was started with.
    pid = os.getppid()
    for _ in range(4):
        if pid <= 1:
            return []
        try:
            line = subprocess.run(
                ["ps", "-o", "ppid=,args=", "-p", str(pid)],
                capture_output=True,
                text=True,
                timeout=1,
            ).stdout.strip()
            parent, _, args = line.partition(" ")
            parent = int(parent)
        except Exception:
            return []
        words = args.split()
        if words and (os.path.basename(words[0]) == "claude" or "/claude/versions/" in words[0]):
            return words[1:]
        pid = parent
    return []


def launch_mode(flags):
    if "--dangerously-skip-permissions" in flags:
        return "bypassPermissions"
    for index, flag in enumerate(flags):
        if flag == "--permission-mode" and index + 1 < len(flags):
            return flags[index + 1]
        if flag.startswith("--permission-mode="):
            return flag.split("=", 1)[1]
    return None


def claude_resume_argv(session_id):
    if not plain(session_id):
        return None
    facts = transcript_facts(agent_session_path)
    flags = claude_launch_flags()
    started_mode = launch_mode(flags)
    mode = next(
        (m for m in (hook_input.get("permission_mode"), facts["mode"], started_mode) if m in RESUME_MODES),
        None,
    )
    model = plain(hook_input.get("model")) or facts["model"]
    bypass = (
        mode == "bypassPermissions"
        or facts["saw_bypass"]
        or started_mode == "bypassPermissions"
        or "--allow-dangerously-skip-permissions" in flags
    )
    argv = ["claude", "--resume", session_id]
    if model:
        argv += ["--model", model]
    if facts["effort"]:
        argv += ["--effort", facts["effort"]]
    if bypass:
        argv.append("--allow-dangerously-skip-permissions")
    if mode:
        argv += ["--permission-mode", mode]
    return argv


if agent_session_id:
    params = {
        "pane_id": pane_id,
        "source": source,
        "agent": "claude",
        "seq": report_seq,
        "agent_session_id": agent_session_id,
    }
    if agent_session_path:
        params["agent_session_path"] = agent_session_path
    if session_start_source:
        params["session_start_source"] = session_start_source
    resume_argv = claude_resume_argv(agent_session_id)
    if resume_argv:
        params["resume_argv"] = resume_argv
    request = {
        "id": request_id,
        "method": "pane.report_agent_session",
        "params": params,
    }
else:
    raise SystemExit(0)

try:
    client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    client.settimeout(0.5)
    client.connect(socket_path)
    client.sendall((json.dumps(request) + "\n").encode())
    try:
        client.recv(4096)
    except Exception:
        pass
    client.close()
except Exception:
    pass
PY
