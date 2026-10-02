#!/bin/bash
# usage: scen.sh <name> <action: enter|esc|chat|ctrlc>   (claude must already be running in w1:p1)
cd "$(git rev-parse --show-toplevel)"
D=/private/tmp/claude-501/i137spike; N=i137s
E=(env -u HERDR_ENV -u HERDR_SOCKET_PATH -u HERDR_CLIENT_SOCKET_PATH -u HERDR_SESSION -u HERDR_WORKSPACE_ID -u HERDR_TAB_ID -u HERDR_PANE_ID -u ANTHROPIC_BASE_URL HERDR_CONFIG_PATH=$D/cfg/herdr-dev/config.toml)
H=("${E[@]}" target/debug/herdr --session $N)
st(){ "${H[@]}" pane get w1:p1 | python3 -c 'import json,sys; p=json.load(sys.stdin)["result"]["pane"]; t=p.get("tokens") or {}; print(p["agent_status"], t.get("question","-"))'; }
echo "=== $1 ($2)"
"${H[@]}" pane send-text w1:p1 "${3:-Use the AskUserQuestion tool once to ask me which colour I prefer, red or blue. Do nothing else.}" >/dev/null; sleep 0.4; "${H[@]}" pane send-keys w1:p1 Enter >/dev/null
for i in $(seq 1 30); do sleep 0.5; s=$(st); echo "$s" | grep -q "blocked" && break; done
echo "dialog open : $(st)"
case $2 in
 enter) "${H[@]}" pane send-keys w1:p1 Enter >/dev/null;;
 esc) "${H[@]}" pane send-keys w1:p1 Escape >/dev/null;;
 chat) "${H[@]}" pane send-keys w1:p1 4 >/dev/null; sleep 0.3; "${H[@]}" pane send-keys w1:p1 Enter >/dev/null;;
 ctrlc) "${H[@]}" pane send-keys w1:p1 ctrl+c >/dev/null; sleep 0.3; "${H[@]}" pane send-keys w1:p1 ctrl+c >/dev/null;;
esac
for i in 1 2 3 4 5 6; do sleep 1; echo "after +${i}s    : $(st)"; done
