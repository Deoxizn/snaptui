# snaptui

A small TUI for managing btrfs snapshots on [Omarchy](https://omarchy.org/) Linux.

Talks to `snapperd` (snapper's D-Bus service) directly, so it shows and edits
the same data the `snapper` CLI manages — including the automatic cleanup
limits that `snapper-cleanup.timer` enforces.

## Features

- **List snapshots** per config — number, type, date, description, cleanup
  algorithm, owner and used space.
- **Delete snapshots** (single or multi-select with `space`).
- **Create** single snapshots with a description and cleanup algorithm.
- **Edit** a snapshot's description / cleanup algorithm.
- **Config browser** — every snapper config and its subvolume.
- **Limits editor** — `NUMBER_LIMIT`, `TIMELINE_LIMIT_*`, `SPACE_LIMIT`, …
  saves via `sudo snapper set-config`, so old snapshots get pruned
  automatically by `snapper-cleanup.timer`. You set limits once and forget it.
- **Live omarchy theming** — picks up the active theme's
  `colors.toml` and re-themes itself as soon as the file changes
  (e.g. after `omarchy theme set …`).

## Why not polkit?

This snapper build does **not** use polkit for authorization. Permission is
granted per-config with `ALLOW_USERS` / `ALLOW_GROUPS` in
`/etc/snapper/configs/<name>`, checked inside `snapperd`. The installer sets
`ALLOW_USERS` for you, so snapshot operations work without passwords. Editing
config limits (`SetConfig`) is root-only in snapperd, so the limits screen
drops to a `sudo` prompt to save.

## Install

```bash
./install.sh
```

This builds the release binary, copies it to `~/.local/bin/snaptui`, and sets
`ALLOW_USERS` on every snapper config (one sudo prompt).

To make the **size** column show real numbers, btrfs quotas need to be
enabled on the filesystem (one-time, persists across reboots). Without it the
size column shows `–`:

```bash
sudo btrfs quota enable /
```

### Uninstall

```bash
./uninstall.sh
```

Removes the binary and clears the `ALLOW_USERS` value install.sh set — only if
it still equals your username, so any `ALLOW_USERS` you set yourself or that
another tool (e.g. btrfs-assistant) uses is left untouched. Delete the source
tree with `rm -rf ~/git/snaptui`.

### Manual

```bash
cargo build --release
cp target/release/snaptui ~/.local/bin/
sudo snapper -c root set-config "ALLOW_USERS=${USER}"
```

## Usage

```
snaptui
```

| Key | Action |
|-----|--------|
| `Tab` / `t` | next tab (Snapshots / Configs / Limits) |
| `↑`/`↓` / `j`/`k` | move selection |
| `space` | (snapshots) toggle multi-select |
| `a` | select all snapshots |
| `d` | delete selected / highlighted snapshot |
| `c` | create a single snapshot |
| `e` | edit snapshot description / cleanup |
| `i` | initialize snapshot storage (`sudo btrfs subvolume create /.snapshots`) |
| `s` | (limits) save changed limits via sudo |
| `x` | (limits) discard unsaved limit changes |
| `r` | refresh |
| `q` | quit |

Snapshot "size" uses btrfs quota data (`GetUsedSpace`). It shows `0 B` until
qgroups are enabled:

```bash
sudo btrfs quota enable /
```

## Permissions recap

- Read + snapshot create/delete/edit: granted via `ALLOW_USERS` in the config.
- Config limit changes: root-only (`sudo snapper set-config …`).
