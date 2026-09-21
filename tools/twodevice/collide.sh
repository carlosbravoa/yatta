#!/usr/bin/env bash
# Force the interesting conflict: both devices edit the same task's notes in
# the same place, offline. Close (or just leave) both apps, run this, then hit
# Sync on one device and then the other.
set -euo pipefail
BENCH="${BENCH:-$HOME/yatta-twodevice}"

python3 - "$BENCH" <<'PY'
import pathlib, sys
bench = pathlib.Path(sys.argv[1])

edits = {
    "laptop": ("done", "urgent", "Draft is in the shared folder.\nAsk Ana to review the numbers first."),
    "desktop": ("doing", "high", "Draft is in the shared folder.\nNumbers confirmed by finance on Tuesday."),
}
for device, (status, priority, body) in edits.items():
    path = bench / device / "report.md"
    path.write_text(
        f"---\nid: shared01\ntitle: Send the quarterly report\n"
        f"status: {status}\npriority: {priority}\ndue: 2026-10-01\n"
        f"tags: [work, {device}]\ncreated: 2026-09-18\n---\n\n{body}\n"
    )
    print(f"{device}: status={status} priority={priority}, rival note")
PY

echo
echo "now: Sync on laptop, then Sync on desktop."
echo "expect on desktop: status done, priority urgent, tags work+laptop+desktop,"
echo "       and the two notes side by side under conflict markers."
