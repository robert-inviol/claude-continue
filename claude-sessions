#!/usr/bin/env bash
set -euo pipefail

PROJECTS_DIR="${HOME}/.claude/projects"

# Colors
HEADER_FG="#f5c2e7"
DIM="#6c7086"
ACCENT="#89b4fa"
GREEN="#a6e3a1"
YELLOW="#f9e2af"
RED="#f38ba8"

die() { echo "$1" >&2; exit 1; }
command -v gum >/dev/null || die "gum is not installed. Install it: https://github.com/charmbracelet/gum"
command -v fzf >/dev/null || die "fzf is not installed. Install it: https://github.com/junegunn/fzf"
command -v python3 >/dev/null || die "python3 is required"

# ── CWD to project directory mapping ─────────────────────────────────────────

# Convert cwd to its project dir name under ~/.claude/projects/
# Echoes the dir name if found (with sessions), empty otherwise
cwd_to_project_dir() {
    local dir_name
    dir_name=$(pwd | sed 's|/|-|g')
    if [[ -d "${PROJECTS_DIR}/${dir_name}" ]] && ls "${PROJECTS_DIR}/${dir_name}"/*.jsonl &>/dev/null; then
        echo "$dir_name"
    fi
}

# ── Data extraction (single python3 call per operation) ──────────────────────

# Outputs tab-separated: dir_name \t display_path \t session_count \t time_ago
list_projects_data() {
    python3 -c "
import os, json, glob
from datetime import datetime, timezone

projects_dir = os.path.expanduser('~/.claude/projects')
home = os.path.expanduser('~')
now = datetime.now(timezone.utc)
results = []

def time_ago(ts):
    secs = int((now - ts).total_seconds())
    if secs < 0: secs = 0
    if secs < 3600: return f'{secs//60}m ago'
    if secs < 86400: return f'{secs//3600}h ago'
    if secs < 604800: return f'{secs//86400}d ago'
    return ts.strftime('%Y-%m-%d')

for entry in os.listdir(projects_dir):
    proj_path = os.path.join(projects_dir, entry)
    if not os.path.isdir(proj_path):
        continue
    sessions = glob.glob(os.path.join(proj_path, '*.jsonl'))
    if not sessions:
        continue

    newest = max(sessions, key=os.path.getmtime)
    mtime = os.path.getmtime(newest)

    # Extract cwd from newest session
    decoded = None
    try:
        with open(newest) as f:
            for line in f:
                try:
                    obj = json.loads(line)
                    if obj.get('type') == 'user' and obj.get('cwd'):
                        decoded = obj['cwd']
                        break
                except: pass
    except: pass

    if not decoded:
        decoded = '/' + entry.lstrip('-').replace('-', '/')

    short = decoded.replace(home, '~')
    if short == '~':
        short = '~/'
    dt = datetime.fromtimestamp(mtime, tz=timezone.utc)
    results.append((entry, short, len(sessions), mtime, time_ago(dt)))

results.sort(key=lambda x: x[3], reverse=True)
for dir_name, path, count, _, ago in results:
    print(f'{dir_name}\t{path}\t{count}\t{ago}')
"
}

# Shared python helper for session scanning
SESSION_SCANNER_PY='
import json, sys, os, glob
from datetime import datetime, timezone

home = os.path.expanduser("~")
now = datetime.now(timezone.utc)

def time_ago(ts_str):
    if not ts_str: return "unknown"
    try:
        dt = datetime.fromisoformat(ts_str.replace("Z", "+00:00"))
        secs = int((now - dt).total_seconds())
        if secs < 0: secs = 0
        if secs < 60: return f"{secs}s ago"
        if secs < 3600: return f"{secs//60}m ago"
        if secs < 86400: return f"{secs//3600}h ago"
        if secs < 604800: return f"{secs//86400}d ago"
        return dt.strftime("%Y-%m-%d")
    except: return ts_str[:10] if len(ts_str) >= 10 else "unknown"

def fmt_size(s):
    if s > 1048576: return f"{s/1048576:.1f} MB"
    if s > 1024: return f"{s/1024:.1f} KB"
    return f"{s} B"

def scan_session(path):
    first_msg = first_ts = last_ts = cwd = model = ""
    user_count = asst_count = 0
    user_messages = []
    try:
        with open(path) as f:
            for line in f:
                try: obj = json.loads(line)
                except: continue
                t = obj.get("type", "")
                if t == "user":
                    user_count += 1
                    ts = obj.get("timestamp", "")
                    if not first_ts: first_ts = ts
                    last_ts = ts
                    if not cwd: cwd = obj.get("cwd", "")
                    msg = obj["message"]["content"]
                    if isinstance(msg, str):
                        clean = msg.replace("\n", " ").replace("\t", " ")
                        user_messages.append(clean[:200])
                        if not first_msg:
                            first_msg = clean[:120]
                elif t == "assistant":
                    asst_count += 1
                    ts = obj.get("timestamp", "")
                    if ts: last_ts = ts
                    m = obj.get("message", {})
                    if not model and isinstance(m, dict):
                        model = m.get("model", "")
    except: pass
    sid = os.path.basename(path).replace(".jsonl", "")
    total = user_count + asst_count
    fsize = fmt_size(os.path.getsize(path))
    return {
        "first_msg": first_msg or "(no message)",
        "first_ts": first_ts, "last_ts": last_ts,
        "cwd": cwd.replace(home, "~"),
        "model": model or "unknown",
        "user_msgs": user_count, "assistant_msgs": asst_count,
        "total_msgs": total,
        "file_size": fsize,
        "session_id": sid,
        "first_ts_ago": time_ago(first_ts),
        "last_ts_ago": time_ago(last_ts),
        "user_messages": user_messages,
    }

def output_session(info, project_label=None):
    sid = info["session_id"]
    ago = time_ago(info["last_ts"])
    total = info["total_msgs"]
    fsize = info["file_size"]
    display_msg = info["first_msg"][:70] if info["first_msg"] else "(no message)"
    if project_label:
        display = f"{ago:<10s}  {total:>3} msgs  {fsize:>7s}  {sid[:8]:<8s}  {project_label:<20s}  {display_msg}"
    else:
        display = f"{ago:<10s}  {total:>3} msgs  {fsize:>7s}  {sid[:8]:<8s}  {display_msg}"
    out = dict(info)
    out.pop("user_messages", None)
    sys.stdout.write(f"{sid}\x1e{display}\x1e{json.dumps(out)}\n")
'

# Outputs \x1e-separated lines: session_id RS display_line RS info_json
list_sessions_data() {
    local proj_path="$1"
    python3 -c "
${SESSION_SCANNER_PY}

import sys
proj_path = sys.argv[1]
files = glob.glob(os.path.join(proj_path, '*.jsonl'))
files.sort(key=os.path.getmtime, reverse=True)
for path in files:
    info = scan_session(path)
    output_session(info)
" "$proj_path"
}

# Scan ALL sessions across all projects. Outputs same \x1e-separated format
# with project path in the display line. Also outputs proj_path after info_json.
search_all_sessions_data() {
    python3 -c "
${SESSION_SCANNER_PY}

projects_dir = os.path.expanduser('~/.claude/projects')
all_sessions = []

for entry in os.listdir(projects_dir):
    proj_path = os.path.join(projects_dir, entry)
    if not os.path.isdir(proj_path): continue
    files = glob.glob(os.path.join(proj_path, '*.jsonl'))
    for path in files:
        mtime = os.path.getmtime(path)
        all_sessions.append((mtime, path, proj_path))

# Sort all sessions by mtime descending
all_sessions.sort(key=lambda x: x[0], reverse=True)

for mtime, path, proj_path in all_sessions:
    info = scan_session(path)
    if info['total_msgs'] == 0: continue
    # Use cwd as the project label, or derive from proj_path
    project_label = info['cwd'] if info['cwd'] else proj_path.replace(home, '~')
    # Truncate project label for display
    if len(project_label) > 20:
        project_label = '..' + project_label[-18:]
    sid = info['session_id']
    ago = time_ago(info['last_ts'])
    total = info['total_msgs']
    fsize = info['file_size']
    display_msg = info['first_msg'][:55] if info['first_msg'] else '(no message)'
    display = f'{ago:<10s}  {total:>3} msgs  {fsize:>7s}  {sid[:8]:<8s}  {project_label:<20s}  {display_msg}'
    # Include full session ID and user messages in search text for matching
    search_blob = (sid + ' ' + info['first_msg'] + ' ' + ' '.join(info.get('user_messages', []))).lower()
    out = dict(info)
    out.pop('user_messages', None)
    out['proj_path'] = proj_path
    out['search_blob'] = search_blob
    sys.stdout.write(f'{sid}\x1e{display}\x1e{json.dumps(out)}\n')
"
}

# ── Search all sessions screen ───────────────────────────────────────────────

show_search() {
    local initial_query="${1:-}"

    # Loading indicator
    gum spin --spinner dot --title "Indexing all sessions..." -- bash -c '
        # Pre-cache: just trigger the scan so the next call is fast
        true
    '

    local -a session_ids=()
    local -a display_lines=()
    local -a info_jsons=()

    while IFS= read -r line; do
        [[ -z "$line" ]] && continue
        local sid display info
        sid="${line%%$'\x1e'*}"
        local rest="${line#*$'\x1e'}"
        display="${rest%%$'\x1e'*}"
        info="${rest#*$'\x1e'}"
        session_ids+=("$sid")
        display_lines+=("$display")
        info_jsons+=("$info")
    done < <(search_all_sessions_data)

    if [[ ${#session_ids[@]} -eq 0 ]]; then
        gum style --foreground "$RED" "No sessions found."
        sleep 1
        return
    fi

    while true; do
        clear
        gum style --bold --foreground "$HEADER_FG" --border double --border-foreground "$ACCENT" --padding "0 2" \
            "  Search All Sessions  "
        echo ""
        gum style --foreground "$DIM" "${#session_ids[@]} sessions  enter: resume · del: delete · →: details · esc: back"
        echo ""

        local keyfile
        keyfile=$(mktemp)
        local selected
        if [[ -n "$initial_query" ]]; then
            selected=$(printf '%s\n' ".." "${display_lines[@]}" | fzf \
                --layout reverse --height 100% --prompt "Search> " \
                --query "$initial_query" \
                --bind "del:execute-silent(echo del > $keyfile)+accept" \
                --bind "right:execute-silent(echo right > $keyfile)+accept" \
                --ansi --no-info) || { rm -f "$keyfile"; return; }
            initial_query=""
        else
            selected=$(printf '%s\n' ".." "${display_lines[@]}" | fzf \
                --layout reverse --height 100% --prompt "Search> " \
                --bind "del:execute-silent(echo del > $keyfile)+accept" \
                --bind "right:execute-silent(echo right > $keyfile)+accept" \
                --ansi --no-info) || { rm -f "$keyfile"; return; }
        fi

        local key=""
        [[ -s "$keyfile" ]] && key=$(<"$keyfile")
        rm -f "$keyfile"

        [[ -z "$selected" ]] && continue
        [[ "$selected" == ".." ]] && return

        local idx=-1
        for i in "${!display_lines[@]}"; do
            if [[ "${display_lines[$i]}" == "$selected" ]]; then
                idx=$i
                break
            fi
        done
        [[ $idx -lt 0 ]] && continue

        case "$key" in
            del)
                local del_sid="${session_ids[$idx]}" proj_path
                proj_path=$(python3 -c "import sys,json; print(json.loads(sys.argv[1])['proj_path'])" "${info_jsons[$idx]}" 2>/dev/null)
                if gum confirm --default=No "Delete session ${del_sid:0:8}...?"; then
                    rm -f "${proj_path}/${del_sid}.jsonl"
                    rm -rf "${proj_path}/${del_sid}" 2>/dev/null
                    unset 'session_ids[$idx]' 'display_lines[$idx]' 'info_jsons[$idx]'
                    session_ids=("${session_ids[@]}")
                    display_lines=("${display_lines[@]}")
                    info_jsons=("${info_jsons[@]}")
                fi
                ;;
            right)
                local proj_path cwd
                proj_path=$(python3 -c "import sys,json; print(json.loads(sys.argv[1])['proj_path'])" "${info_jsons[$idx]}" 2>/dev/null)
                cwd=$(python3 -c "import sys,json; print(json.loads(sys.argv[1])['cwd'])" "${info_jsons[$idx]}" 2>/dev/null)
                show_session_detail "${session_ids[$idx]}" "${info_jsons[$idx]}" "$cwd" "$proj_path"
                ;;
            *)
                local cwd
                cwd=$(python3 -c "import sys,json; print(json.loads(sys.argv[1])['cwd'])" "${info_jsons[$idx]}" 2>/dev/null)
                launch_claude "$cwd" --resume "${session_ids[$idx]}"
                ;;
        esac
    done
}

# ── Direct session lookup by ID (partial match) ─────────────────────────────

lookup_session() {
    local query="$1"

    python3 -c "
import os, glob, sys

query = sys.argv[1].lower()
projects_dir = os.path.expanduser('~/.claude/projects')
matches = []

for entry in os.listdir(projects_dir):
    proj_path = os.path.join(projects_dir, entry)
    if not os.path.isdir(proj_path): continue
    for f in glob.glob(os.path.join(proj_path, '*.jsonl')):
        basename = os.path.basename(f).replace('.jsonl', '')
        if query in basename.lower():
            matches.append((f, proj_path, basename))

if not matches:
    print('NONE')
elif len(matches) == 1:
    f, pp, sid = matches[0]
    print(f'{sid}\t{pp}')
else:
    # Multiple matches - print all
    for f, pp, sid in matches:
        print(f'{sid}\t{pp}')
" "$query"
}

# ── View mode state ──────────────────────────────────────────────────────────

VIEW_MODE="folders"  # "folders" or "recent"

# ── Project list screen (folders mode) ──────────────────────────────────────

show_project_list() {
    local -a items=()
    local -a dir_names=()

    while IFS=$'\t' read -r dir_name path count ago; do
        items+=("$(printf '%-45s  %3s sessions  %s' "$path" "$count" "$ago")")
        dir_names+=("$dir_name")
    done < <(list_projects_data)

    if [[ ${#items[@]} -eq 0 ]]; then
        gum style --foreground "$RED" "No sessions found."
        exit 0
    fi

    local header
    header="  Folders  |  Recent                          (tab to switch, esc to quit)"

    local choice
    choice=$(printf '%s\n' "${items[@]}" | fzf \
        --header "$header" \
        --header-first \
        --layout reverse \
        --height 100% \
        --prompt "Folders> " \
        --bind 'tab:abort' \
        --expect tab \
        --ansi \
        --no-info) || exit 0

    # fzf --expect outputs the key on line 1, selection on line 2
    local key selected
    key=$(head -1 <<< "$choice")
    selected=$(tail -n +2 <<< "$choice")

    if [[ "$key" == "tab" ]]; then
        VIEW_MODE="recent"
        return
    fi

    [[ -z "$selected" ]] && return

    for i in "${!items[@]}"; do
        if [[ "${items[$i]}" == "$selected" ]]; then
            show_sessions "${dir_names[$i]}"
            return
        fi
    done
}

# ── Flat recent sessions screen ─────────────────────────────────────────────

show_recent() {
    local -a session_ids=()
    local -a display_lines=()
    local -a info_jsons=()

    while IFS= read -r line; do
        [[ -z "$line" ]] && continue
        local sid display info
        sid="${line%%$'\x1e'*}"
        local rest="${line#*$'\x1e'}"
        display="${rest%%$'\x1e'*}"
        info="${rest#*$'\x1e'}"
        session_ids+=("$sid")
        display_lines+=("$display")
        info_jsons+=("$info")
    done < <(search_all_sessions_data)

    if [[ ${#session_ids[@]} -eq 0 ]]; then
        gum style --foreground "$RED" "No sessions found."
        exit 0
    fi

    local header
    header="  Folders  |  Recent          tab: switch · enter: resume · del: delete · →: details · esc: quit"

    local keyfile
    keyfile=$(mktemp)
    local selected
    selected=$(printf '%s\n' "${display_lines[@]}" | fzf \
        --header "$header" \
        --header-first \
        --layout reverse \
        --height 100% \
        --prompt "Recent> " \
        --bind "tab:execute-silent(echo tab > $keyfile)+accept" \
        --bind "del:execute-silent(echo del > $keyfile)+accept" \
        --bind "right:execute-silent(echo right > $keyfile)+accept" \
        --ansi \
        --no-info) || { rm -f "$keyfile"; exit 0; }

    local key=""
    [[ -s "$keyfile" ]] && key=$(<"$keyfile")
    rm -f "$keyfile"

    if [[ "$key" == "tab" ]]; then
        VIEW_MODE="folders"
        return
    fi

    [[ -z "$selected" ]] && return

    local idx=-1
    for i in "${!display_lines[@]}"; do
        if [[ "${display_lines[$i]}" == "$selected" ]]; then
            idx=$i
            break
        fi
    done
    [[ $idx -lt 0 ]] && return

    case "$key" in
        del)
            local del_sid="${session_ids[$idx]}" proj_path
            proj_path=$(python3 -c "import sys,json; print(json.loads(sys.argv[1])['proj_path'])" "${info_jsons[$idx]}" 2>/dev/null)
            if gum confirm --default=No "Delete session ${del_sid:0:8}...?"; then
                rm -f "${proj_path}/${del_sid}.jsonl"
                rm -rf "${proj_path}/${del_sid}" 2>/dev/null
            fi
            ;;
        right)
            local proj_path cwd
            proj_path=$(python3 -c "import sys,json; print(json.loads(sys.argv[1])['proj_path'])" "${info_jsons[$idx]}" 2>/dev/null)
            cwd=$(python3 -c "import sys,json; print(json.loads(sys.argv[1])['cwd'])" "${info_jsons[$idx]}" 2>/dev/null)
            show_session_detail "${session_ids[$idx]}" "${info_jsons[$idx]}" "$cwd" "$proj_path"
            ;;
        *)
            local cwd
            cwd=$(python3 -c "import sys,json; print(json.loads(sys.argv[1])['cwd'])" "${info_jsons[$idx]}" 2>/dev/null)
            launch_claude "$cwd" --resume "${session_ids[$idx]}"
            ;;
    esac
}

# ── Session list screen ──────────────────────────────────────────────────────

show_sessions() {
    local project_dir="$1"
    local proj_path="${PROJECTS_DIR}/${project_dir}"

    while true; do
        # Re-scan sessions each iteration so deletes are reflected
        local -a session_ids=()
        local -a display_lines=()
        local -a info_jsons=()

        while IFS= read -r line; do
            local sid display info
            sid="${line%%$'\x1e'*}"
            local rest="${line#*$'\x1e'}"
            display="${rest%%$'\x1e'*}"
            info="${rest#*$'\x1e'}"
            session_ids+=("$sid")
            display_lines+=("$display")
            info_jsons+=("$info")
        done < <(list_sessions_data "$proj_path")

        if [[ ${#session_ids[@]} -eq 0 ]]; then
            gum style --foreground "$YELLOW" "No sessions in this project."
            sleep 1
            return
        fi

        local decoded_path
        decoded_path=$(python3 -c "import sys,json; print(json.loads(sys.argv[1])['cwd'])" "${info_jsons[0]}" 2>/dev/null || echo "$project_dir")

        clear
        gum style --bold --foreground "$HEADER_FG" --border double --border-foreground "$ACCENT" --padding "0 2" \
            "  $decoded_path  "
        echo ""
        gum style --foreground "$DIM" "${#session_ids[@]} sessions  enter: resume · del: delete · →: details · esc: back"
        echo ""

        local keyfile
        keyfile=$(mktemp)
        local selected
        selected=$(printf '%s\n' ".." "${display_lines[@]}" | fzf \
            --layout reverse --height 100% --prompt "Sessions> " \
            --bind "del:execute-silent(echo del > $keyfile)+accept" \
            --bind "right:execute-silent(echo right > $keyfile)+accept" \
            --ansi --no-info) || { rm -f "$keyfile"; return; }

        local key=""
        [[ -s "$keyfile" ]] && key=$(<"$keyfile")
        rm -f "$keyfile"

        [[ -z "$selected" ]] && continue
        [[ "$selected" == ".." ]] && return

        local idx=-1
        for i in "${!display_lines[@]}"; do
            if [[ "${display_lines[$i]}" == "$selected" ]]; then
                idx=$i
                break
            fi
        done
        [[ $idx -lt 0 ]] && continue

        case "$key" in
            del)
                local del_sid="${session_ids[$idx]}"
                if gum confirm --default=No "Delete session ${del_sid:0:8}...?"; then
                    rm -f "${proj_path}/${del_sid}.jsonl"
                    rm -rf "${proj_path}/${del_sid}" 2>/dev/null
                fi
                ;;
            right)
                show_session_detail "${session_ids[$idx]}" "${info_jsons[$idx]}" "$decoded_path" "$proj_path"
                ;;
            *)
                local cwd
                cwd=$(python3 -c "import sys,json; print(json.loads(sys.argv[1])['cwd'])" "${info_jsons[$idx]}" 2>/dev/null)
                launch_claude "$cwd" --resume "${session_ids[$idx]}"
                ;;
        esac
    done
}

# ── Launch claude ────────────────────────────────────────────────────────────

launch_claude() {
    local target_cwd="${1/#\~/$HOME}"
    shift
    clear
    cd "$target_cwd"
    exec claude "$@"
}

# ── Session detail screen ────────────────────────────────────────────────────

show_session_detail() {
    local session_id="$1"
    local info_json="$2"
    local project_name="$3"
    local proj_path="$4"
    local jsonl_file="${proj_path}/${session_id}.jsonl"

    # Parse all fields in one python3 call
    local detail
    detail=$(python3 -c "
import sys, json
info = json.loads(sys.argv[1])
fields = [
    info['first_msg'],
    info['session_id'],
    info['cwd'],
    info['model'],
    info['first_ts_ago'] + ' (' + info['first_ts'] + ')' if info['first_ts'] else 'unknown',
    info['last_ts_ago'] + ' (' + info['last_ts'] + ')' if info['last_ts'] else 'unknown',
    str(info['user_msgs']),
    str(info['assistant_msgs']),
    str(info['total_msgs']),
    info['file_size'],
]
print('\x1e'.join(fields))
" "$info_json")

    local first_msg sid cwd model started last_activity user_msgs asst_msgs total_msgs file_size
    IFS=$'\x1e' read -r first_msg sid cwd model started last_activity user_msgs asst_msgs total_msgs file_size <<< "$detail"

    while true; do
        clear
        gum style --bold --foreground "$HEADER_FG" --border double --border-foreground "$ACCENT" --padding "0 2" \
            "  Session Detail  "
        echo ""

        gum style --foreground "$ACCENT" --bold "First message:"
        gum style --foreground "$GREEN" --italic "  \"$first_msg\""
        echo ""

        gum style --foreground "$DIM" "$(printf '  %-16s %s' 'Session ID:' "$sid")"
        gum style --foreground "$DIM" "$(printf '  %-16s %s' 'Project:' "$project_name")"
        gum style --foreground "$DIM" "$(printf '  %-16s %s' 'Working Dir:' "$cwd")"
        gum style --foreground "$DIM" "$(printf '  %-16s %s' 'Model:' "$model")"
        gum style --foreground "$DIM" "$(printf '  %-16s %s' 'Started:' "$started")"
        gum style --foreground "$DIM" "$(printf '  %-16s %s' 'Last Activity:' "$last_activity")"
        gum style --foreground "$DIM" "$(printf '  %-16s %s user / %s assistant (%s total)' 'Messages:' "$user_msgs" "$asst_msgs" "$total_msgs")"
        gum style --foreground "$DIM" "$(printf '  %-16s %s' 'Log Size:' "$file_size")"
        echo ""

        local action
        action=$(gum choose \
            "Resume session" \
            "New session in this directory" \
            "View conversation" \
            "Copy session ID" \
            "Delete session" \
            "Back") || return

        case "$action" in
            "Resume session")
                launch_claude "$cwd" --resume "$sid"
                ;;
            "New session in this directory")
                launch_claude "$cwd"
                ;;
            "View conversation")
                show_conversation "$jsonl_file"
                ;;
            "Copy session ID")
                echo -n "$sid" | xclip -selection clipboard 2>/dev/null \
                    || echo -n "$sid" | xsel --clipboard 2>/dev/null \
                    || echo -n "$sid" | wl-copy 2>/dev/null \
                    || true
                gum style --foreground "$GREEN" "Session ID copied: $sid"
                sleep 1
                ;;
            "Delete session")
                if gum confirm --default=No "Delete session ${sid:0:8}...?"; then
                    rm -f "$jsonl_file"
                    # Also remove companion directory if it exists
                    rm -rf "${jsonl_file%.jsonl}" 2>/dev/null
                    gum style --foreground "$GREEN" "Session deleted."
                    sleep 1
                    return
                fi
                ;;
            "Back")
                return
                ;;
        esac
    done
}

# ── Conversation viewer ──────────────────────────────────────────────────────

show_conversation() {
    local jsonl_file="$1"

    python3 -c "
import json, sys

path = sys.argv[1]
messages = []

with open(path) as f:
    for line in f:
        try:
            obj = json.loads(line)
        except json.JSONDecodeError:
            continue

        t = obj.get('type', '')
        if t == 'user':
            msg = obj['message']['content']
            if isinstance(msg, str) and msg.strip():
                messages.append(('user', msg.strip(), obj.get('timestamp', '')))
        elif t == 'assistant':
            m = obj.get('message', {})
            if isinstance(m, dict):
                content_parts = m.get('content', [])
                text_parts = []
                tool_parts = []
                for part in (content_parts if isinstance(content_parts, list) else []):
                    if isinstance(part, dict):
                        if part.get('type') == 'text' and part.get('text', '').strip():
                            text_parts.append(part['text'].strip())
                        elif part.get('type') == 'tool_use':
                            tool_parts.append(f\"[tool: {part.get('name', '?')}]\")
                combined = ' '.join(text_parts)
                if tool_parts and not combined:
                    combined = ' '.join(tool_parts)
                elif tool_parts:
                    combined += '  ' + ' '.join(tool_parts)
                if combined:
                    messages.append(('assistant', combined, obj.get('timestamp', '')))

if not messages:
    print('(no readable messages)')
    sys.exit()

for role, msg, ts in messages:
    ts_short = ts[11:16] if len(ts) > 16 else ''
    if role == 'user':
        prefix = f'\033[1;34m[{ts_short}] You:\033[0m'
    else:
        prefix = f'\033[1;32m[{ts_short}] Claude:\033[0m'

    # Truncate very long assistant messages
    if role == 'assistant' and len(msg) > 800:
        msg = msg[:797] + '...'

    print(prefix)
    for line in msg.split('\n'):
        print(f'  {line}')
    print()
" "$jsonl_file" | gum pager
}

# ── Usage ─────────────────────────────────────────────────────────────────────

usage() {
    echo "Usage: claude-sessions [options]"
    echo ""
    echo "Interactive explorer for Claude Code sessions."
    echo ""
    echo "Options:"
    echo "  -s, --search [QUERY]   Search all sessions (opens search view)"
    echo "  -i, --id ID            Look up a session by full or partial ID"
    echo "  -h, --help             Show this help"
    echo ""
    echo "With no arguments, opens the interactive project browser."
}

# ── Main ─────────────────────────────────────────────────────────────────────

main() {
    [[ -d "$PROJECTS_DIR" ]] || die "No Claude projects directory found at $PROJECTS_DIR"

    case "${1:-}" in
        -h|--help)
            usage
            exit 0
            ;;
        -s|--search)
            shift
            show_search "${*:-}"
            exit 0
            ;;
        -i|--id)
            shift
            [[ -z "${1:-}" ]] && die "Usage: claude-sessions --id <session-id>"
            local query="$1"
            local result
            result=$(lookup_session "$query")

            if [[ "$result" == "NONE" ]]; then
                gum style --foreground "$RED" "No session found matching: $query"
                exit 1
            fi

            local match_count
            match_count=$(echo "$result" | wc -l)

            if [[ "$match_count" -eq 1 ]]; then
                local sid proj_path
                IFS=$'\t' read -r sid proj_path <<< "$result"
                # Build info and show detail
                local info
                info=$(python3 -c "
${SESSION_SCANNER_PY}
import sys
path = sys.argv[1]
info = scan_session(path)
info.pop('user_messages', None)
info['proj_path'] = sys.argv[2]
import json; print(json.dumps(info))
" "${proj_path}/${sid}.jsonl" "$proj_path")
                local cwd
                cwd=$(python3 -c "import sys,json; print(json.loads(sys.argv[1])['cwd'])" "$info")
                show_session_detail "$sid" "$info" "$cwd" "$proj_path"
            else
                gum style --foreground "$YELLOW" "Multiple sessions match '$query':"
                echo ""
                # Feed into search with the query pre-filled
                show_search "$query"
            fi
            exit 0
            ;;
        "")
            # Try to scope to current directory's project
            local cwd_project
            cwd_project=$(cwd_to_project_dir)
            if [[ -n "$cwd_project" ]]; then
                show_sessions "$cwd_project"
            fi
            # Fall through to full browser when .. or esc is pressed
            while true; do
                clear
                if [[ "$VIEW_MODE" == "recent" ]]; then
                    show_recent
                else
                    show_project_list
                fi
            done
            ;;
        *)
            # Treat bare argument as a search query if it looks like a UUID fragment,
            # otherwise as a general search
            if [[ "$1" =~ ^[0-9a-f-]+$ ]]; then
                # Looks like a session ID fragment
                local result
                result=$(lookup_session "$1")
                if [[ "$result" != "NONE" ]]; then
                    local match_count
                    match_count=$(echo "$result" | wc -l)
                    if [[ "$match_count" -eq 1 ]]; then
                        local sid proj_path
                        IFS=$'\t' read -r sid proj_path <<< "$result"
                        local info
                        info=$(python3 -c "
${SESSION_SCANNER_PY}
import sys
path = sys.argv[1]
info = scan_session(path)
info.pop('user_messages', None)
info['proj_path'] = sys.argv[2]
import json; print(json.dumps(info))
" "${proj_path}/${sid}.jsonl" "$proj_path")
                        local cwd
                        cwd=$(python3 -c "import sys,json; print(json.loads(sys.argv[1])['cwd'])" "$info")
                        show_session_detail "$sid" "$info" "$cwd" "$proj_path"
                        exit 0
                    fi
                fi
            fi
            # Fall through to general search
            show_search "$*"
            exit 0
            ;;
    esac
}

main "$@"
