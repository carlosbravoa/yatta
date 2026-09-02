# Packaging yatta as a snap

`snap/snapcraft.yaml` builds yatta as a **strictly confined** core24 snap.

## Prerequisites

```bash
sudo snap install snapcraft --classic
sudo snap install lxd
sudo lxd init --auto
sudo usermod -aG lxd "$USER"   # log out and back in afterwards
```

## Build

```bash
cd /path/to/yatta
export SNAPCRAFT_BUILD_INFO=1   # embeds build provenance for CVE reporting
snapcraft pack
```

The first build is slow — it provisions a Rust toolchain and compiles the whole
dependency tree inside a fresh LXD container. Later builds reuse it.

Never pass `--destructive-mode`. It builds on the host, pollutes it with build
dependencies, and produces artefacts that may not reproduce.

## Other architectures

`snapcraft.yaml` declares both `amd64` and `arm64` under `platforms:`. Without
that key snapcraft only ever builds for the host. Nothing else in the build is
architecture-specific -- rustup detects `aarch64` itself, and the `node` and
`gnome-46-2404` snaps both publish arm64.

Three ways to produce an arm64 build, in rough order of effort:

**CI (what this repo does).** `.github/workflows/ci.yml` builds both
architectures on every push. GitHub's arm64 runners are free for public
repositories, so it is a native build rather than emulation. The `.snap` files
land as workflow artefacts; publishing is deliberately manual.

**Launchpad.** `snapcraft remote-build` ships the source to Launchpad, builds
every platform in the manifest and downloads the results. Free for open source,
no hardware, but it needs a Launchpad account and publishes the source there
during the build.

**Locally, emulated.** `snapcraft --build-for=arm64` with qemu binfmt. No
accounts and no network dependency, but every instruction is emulated: the Rust
release build takes about three minutes natively and the better part of an hour
this way. Useful to prove a change compiles, painful as a habit.

> A successful arm64 build proves it compiles, not that it behaves. WebKitGTK
> rendering and the AppIndicator tray have not been exercised on arm64 hardware.
> Test on a real device before releasing an arm64 revision.

## Install and test

Test in two stages. `devmode` disables confinement, so it tells you whether the
app *works*; it tells you nothing about whether the interfaces are right.

```bash
# 1. Does it run at all?
sudo snap install --devmode ./yatta_0.1.0_amd64.snap
yatta

# 2. Does it run under real confinement?
sudo snap remove yatta
sudo snap install --dangerous ./yatta_0.1.0_amd64.snap
yatta
```

While testing confinement, watch for denials in another terminal:

```bash
sudo snap install snappy-debug
sudo snappy-debug
```

## Interface connections

Nine of the ten interfaces auto-connect. One does not:

```bash
# Only needed if you keep your task vault on an external drive or under /mnt.
sudo snap connect yatta:removable-media
```

Check what is connected:

```bash
snap connections yatta
```

| Interface | Auto | Why yatta needs it |
|---|---|---|
| `desktop`, `desktop-legacy` | yes | GTK3 app; session D-Bus and the XDG portal behind "open this file in my editor" |
| `wayland` | yes | Primary display backend on Ubuntu 24.04+ |
| `x11` | yes | Xwayland fallback, and the only backend where the global hotkey can register |
| `opengl` | yes | WebKitGTK composites the webview on the GPU |
| `gsettings` | yes | Follows the desktop light/dark theme preference |
| `unity7` | yes | AppIndicator tray icon |
| `home` | yes | The vault — the app's entire data store |
| `network` | yes | WebKitGTK's network process; future agent integration |
| `removable-media` | **no** | Only if the vault lives on an external drive |

## Design notes specific to this snap

**Why it is strict, not classic.** The optional auto-commit feature shells out
to `git`. That looks like a classic-confinement signal, but it is exactly one
known binary rather than arbitrary host tooling, so git is bundled via
`stage-packages` and pointed at its helpers with `GIT_EXEC_PATH` and
`GIT_TEMPLATE_DIR`. The bundled git still reads the user's `~/.gitconfig`
through the `home` interface, so their name and email carry over. Staying strict
means no Snap Store manual review and it works on Ubuntu Core.

**Why almost nothing is staged.** The `gnome-46-2404` platform snap supplied by
the `gnome` extension already provides libwebkit2gtk-4.1, libjavascriptcoregtk-4.1,
libgtk-3, libsoup-3.0, librsvg-2 *and* libayatana-appindicator3. Staging our own
copies would add roughly 80 MB and risk version skew against the platform's
libsoup. Only `git` is staged. The dev headers are still in `build-packages`
because the compiler needs something to link against.

**Why the frontend build runs before `craftctl default`.** This is a Tauri app:
`tauri-build` embeds the compiled frontend into the Rust binary at compile time
(`frontendDist` points at `../dist`), so `dist/` must already exist when cargo
runs. The `override-build` order is load-bearing — reversing it produces a binary
that builds fine and then serves nothing.

**Why Node comes from a build-snap.** `stage-packages`/`build-packages` are
normally preferred, but noble ships Node 18.19 and Vite 6 requires 20.19+/22.12+.
`node/22/stable` is the only route.

**Where your tasks are stored.** On first run yatta asks, and creates nothing
until you choose. Under confinement `HOME` points at `$SNAP_USER_DATA`, so the
default it suggests is derived from `$SNAP_REAL_HOME` instead — you get
`~/Documents/yatta`, not `~/snap/yatta/current/Documents/yatta`. If you pick a
folder somewhere the `home` interface does not reach (a hidden `~/.dotfile`
directory, or `/media` without `removable-media` connected) the picker reports
the error and stays put rather than failing silently.

## Measuring startup

Both the Rust and webview sides emit epoch-millisecond marks to stderr when
`YATTA_TIMING` is set, so the two timelines read as one sequence:

```bash
YATTA_TIMING=1 ./src-tauri/target/release/yatta 2>&1 | grep TIMING
```

Measure a **release** build; a debug binary starts far slower and will mislead
you. Marks run from process entry through `setup()`, the webview's module start
and mount, each phase of the first data load, and first paint.

This is how the 0.4.1 startup fix was found, after three plausible theories --
WebKit init, GPU setup, the IPC layer -- all turned out to be wrong. The tell
was timers scheduled 50ms, 200ms and 400ms out firing at the same instant, which
means the thread is blocked rather than any one call being slow.

## Troubleshooting

**A blank or black window.** WebKitGTK inside confinement sometimes needs
software compositing. Add to the app's `environment:` and rebuild:

```yaml
      WEBKIT_DISABLE_DMABUF_RENDERER: '1'
      # or, more aggressively:
      # WEBKIT_DISABLE_COMPOSITING_MODE: '1'
```

Both disable acceleration, so only add them if you actually see the problem.

**The very first launch immediately after `snap install` may show the tray icon
but no window.** Observed once, on the launch chained directly onto the install
command. A second launch a few seconds later worked, and every launch since has
been fine.

What the evidence does *not* support: a slow start. `WebKitWebProcess` spawned
0.7 s after launch in both the failed and the working run, and the two runs'
stderr is identical. The process then sat for 36 s wall against 5 s CPU - up
and idle, not working - with no window.

The most likely cause is snapd still settling its mount namespace. The install
logs show it reloading AppArmor profiles, running the configure hook, and
notably `renaming mount entry for directory ".../gpu-2404" to "gpu-2404-2" to
avoid a clash` roughly two seconds before that first launch. If the gpu-2404
content mount was not yet in place, WebKit's GL context creation could fail
without the window ever mapping, while the tray - which needs no graphics -
registers over D-Bus regardless. That is a hypothesis, not a confirmed
diagnosis; reproducing it needs another purge/install cycle.

**A second purge/install cycle did not reproduce it** - that run's first launch
came up normally. So this is a transient race during install settling, not a
deterministic fault, and the gpu-2404 explanation above remains unconfirmed.

Practical guidance: if the window does not appear on the launch immediately
following an install, quit and start it again. Seen once in two install cycles,
never on a subsequent launch. Not worth engineering around unless it starts
recurring - if it does, the thing to capture is whether `WebKitWebProcess` is
running while no window exists, which is what distinguishes this from an
ordinary slow start.

**`GDBus.Error:org.freedesktop.portal.Error.NotAllowed: This call is not
available inside the sandbox` on startup.** PARTLY fixed; a variant remains.
The gnome extension sets `GTK_USE_PORTAL=1`; GLib reads that as "I am in a
Flatpak sandbox" and selects its portal-backed `GProxyResolver`. snapd's portal
does not serve `org.freedesktop.portal.ProxyResolver` to non-Flatpak apps, so
the lookup is refused and GIO warns. It is only a warning - the app runs - but
it is noise on every launch.

`GIO_USE_PROXY_RESOLVER: gnome` in the app's `environment:` pins the
GSettings-backed resolver and removes the `ProxyResolver` refusals. Setting
`dummy` would also work but disables proxy support, which matters on a
corporate network.

That does **not** silence the warning entirely. A second GLib subsystem trips
the same error: with `GTK_USE_PORTAL=1`, GIO also reaches for
`org.freedesktop.portal.Documents`, which snapd does not implement - the
document portal is a Flatpak mechanism. The refusal is logged and GIO carries
on.

This one is being left alone deliberately. Silencing it means `GTK_USE_PORTAL=0`,
which changes how every file dialog behaves in order to remove a cosmetic
stderr line. It also buys nothing on snap: the document portal exists to grant
access to files outside a sandbox, but a snap's access is governed by its
interfaces regardless of what the portal returns, so routing the vault picker
through it gains no reach it does not already have via `home`.

To identify a portal refusal like this rather than guessing, capture the bus:

```bash
dbus-monitor --session > /tmp/bus.txt &
snap run yatta
grep -B30 "not available inside the sandbox" /tmp/bus.txt | grep -E "member=|interface="
```

The `reply_serial` of the error matches the `serial=` of the offending call.

**Startup noise: what was fixed and what was not.**

Fixed - `Not loading module "atk-bridge"` (printed once per process). The
session exports `GTK_MODULES=gail:atk-bridge`; GTK3 provides that natively and
says so. `GTK_MODULES: ''` in the app environment clears it, losing nothing:
accessibility comes from GTK's built-in bridge, and host modules are outside
the sandbox and unloadable regardless.

Not worth fixing, with reasons:

- `GDBus.Error ... NotAllowed` - traced to **WebKitNetworkProcess** calling the
  Documents portal, which snapd does not implement. Note this is *not* GLib's
  file-chooser portal path: setting `GTK_USE_PORTAL=0` was tested and does not
  silence it. The only lever is disabling WebKit's own sandbox, which is a
  security regression traded for a log line. Leave it.
- `Could not open /sys/class/dmi/id/chassis_type` and `/sys/firmware/acpi/pm_profile`
  - WebKit probing whether it is on a laptop. Needs `hardware-observe`: a broad
  permission that does not auto-connect, so every user would have to run
  `snap connect` to remove a warning that affects nothing.
- `libayatana-appindicator is deprecated` - upstream's message, not ours.
- `Error creating IO channel for /proc/self/mountinfo: Permission denied` -
  appears when the folder picker opens. GTK's file chooser enumerates mounted
  volumes via `/proc/self/mountinfo` and `/etc/fstab`, neither of which a
  confined snap may read. The chooser still works; its sidebar just will not
  list other volumes. Fixing it would need `mount-observe`, which is not worth
  a sidebar entry.

**Inspect the confined environment:**

```bash
snap run --shell yatta
echo "$SNAP_REAL_HOME"          # should be your real home
ls "$SNAP/usr/lib/git-core"     # bundled git helpers
```

**Denials and logs:**

```bash
sudo snappy-debug              # AppArmor denials, with suggested interfaces
journalctl -xe | grep -i yatta
sudo dmesg | grep -i apparmor | tail -20
```

**The tray icon does not appear.** GNOME needs an AppIndicator extension
installed to show tray icons at all — check that before assuming a snap problem.
The tray can also be compiled out entirely with
`cargo build --no-default-features`, and the settings panel hides those toggles
automatically in that build.

**The global hotkey does nothing.** Expected in a Wayland session: the
compositor owns global shortcuts, and no amount of packaging changes that. Bind a
shortcut to `yatta` in GNOME's keyboard settings instead.

## Publishing

Before any public release:

- **Add a LICENSE file.** The repo has none, so `snapcraft.yaml` declares no
  `license:` field. This must be resolved before publishing.
- Optionally add `contact:`, `issues:`, `source-code:` and `website:` to
  `snapcraft.yaml` to clear the remaining lint hints.

```bash
snapcraft login
snapcraft register yatta          # the name is currently unregistered
snapcraft upload ./yatta_0.1.0_amd64.snap --release=edge
```

Promote once it has had some real use:

```bash
snapcraft release yatta <revision> beta
```
