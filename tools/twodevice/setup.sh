#!/usr/bin/env bash
# Build a two-device yatta test bench: one bare "server" repo and two vaults
# that clone it, plus a separate config dir per device so the two app
# instances do not share settings.json.
#
#   ./setup.sh            rebuild the bench from scratch (destroys the old one)
set -euo pipefail

BENCH="${BENCH:-$HOME/yatta-twodevice}"
ID=com.yatta.app

rm -rf "$BENCH"
mkdir -p "$BENCH"
cd "$BENCH"

git init -q --bare -b main remote.git

# --- device one: seed a couple of tasks and push -----------------------------
git -c init.defaultBranch=main clone -q remote.git laptop
cd laptop
cat > report.md <<'TASK'
---
id: shared01
title: Send the quarterly report
status: todo
priority: medium
due: 2026-10-01
tags: [work]
created: 2026-09-18
---

Draft is in the shared folder.
TASK
cat > milk.md <<'TASK'
---
id: shared02
title: Buy milk
status: todo
priority: low
tags: [home]
created: 2026-09-18
---

Semi-skimmed.
TASK
git add -A
git -c user.name=laptop -c user.email=laptop@localhost commit -qm "first tasks"
git push -q -u origin HEAD
cd ..

# --- device two: the way a real second machine starts ------------------------
git clone -q remote.git desktop

# --- one settings.json per device -------------------------------------------
for device in laptop desktop; do
  dir="$BENCH/config-$device/$ID"
  mkdir -p "$dir"
  cat > "$dir/settings.json" <<JSON
{
  "vault_path": "$BENCH/$device",
  "theme": "system",
  "first_run_done": true,
  "git_autocommit": true,
  "git_sync": true,
  "git_sync_interval_mins": 0,
  "git_sync_on_change": true,
  "reminders_enabled": false,
  "tray_enabled": false,
  "autostart": false
}
JSON
done

echo "bench ready at $BENCH"
echo "  remote.git   the shared 'server'"
echo "  laptop/      device one's vault (2 tasks, pushed)"
echo "  desktop/     device two's vault (cloned)"
echo
echo "run each device in its own terminal:"
echo "  $(dirname "$0")/run.sh laptop"
echo "  $(dirname "$0")/run.sh desktop"
