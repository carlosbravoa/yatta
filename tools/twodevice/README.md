# Two-device sync bench

A throwaway rig for testing git sync without owning two computers: one bare
repository standing in for the server, two vaults that clone it, and a separate
config directory per device so two instances of yatta do not share one
`settings.json`.

```bash
tools/twodevice/setup.sh        # build the bench (destroys any previous one)
tools/twodevice/run.sh laptop   # one terminal
tools/twodevice/run.sh desktop  # another
tools/twodevice/collide.sh      # force a notes conflict in both vaults
```

`setup.sh` puts everything under `~/yatta-twodevice` (override with `BENCH=`)
and needs the release binary:

```bash
npm run build
cargo build --release --manifest-path src-tauri/Cargo.toml --features custom-protocol
```

`XDG_CONFIG_HOME` is what makes the two instances separate devices rather than
two windows onto the same settings; `YATTA_DEVICE` is the name that ends up in
commit messages and in conflict markers, which is otherwise the hostname and
therefore the same for both.

The bench syncs to a local bare repo, so it exercises the merge and the
scheduling but never authentication. The interval is set to *only when I ask*,
so the sending device pushes on its own and the receiving one pulls when you
press Sync.

Watching what actually reaches the "server" is often clearer than watching the
windows:

```bash
watch -n2 'git -C ~/yatta-twodevice/remote.git log --oneline -10'
```
