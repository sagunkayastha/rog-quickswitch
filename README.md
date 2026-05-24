# rog-quickswitch

Small GTK4/libadwaita GUI for ASUS ROG laptops on Linux. Talks to `asusd` and `supergfxd` over D-Bus; the heavyweight features (full per-key Aura, Anime Matrix, detailed tuning) stay in `rog-control-center`.

Built as a G-Helper-style daily driver for the actions you actually reach for: flip GPU mode, change performance profile, set charge limit, tweak the fan curve, toggle panel overdrive, dim the keyboard.

## Status

Scaffolded. One panel (Performance profile) is wired end-to-end via sysfs. The rest are layout stubs with TODO markers pointing at the D-Bus surface to bind.

## Build

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev libdbus-1-dev pkg-config
cargo build --release
```

First build pulls a lot of `gtk4-rs` — expect 5+ minutes.

## Run

```bash
cargo run --release
```

Or install the launcher:

```bash
cp rog-quickswitch.desktop ~/.local/share/applications/
update-desktop-database ~/.local/share/applications/
```

## Architecture

```
src/
├── main.rs         AdwApplication entry, tokio runtime
├── window.rs       Main AdwApplicationWindow + AdwPreferencesPage
├── panels/
│   ├── profile.rs            ✅ wired (sysfs)
│   ├── gpu_mode.rs           ⏳ supergfxd binding
│   ├── charge_limit.rs       ⏳ asus_armoury/charge_mode
│   ├── panel_overdrive.rs    ⏳ asus_armoury/panel_overdrive
│   ├── aura.rs               ⏳ xyz.ljones.aura/*
│   └── fan_curve.rs          ⏳ xyz.ljones.FanCurves (separate iface)
├── dbus/
│   ├── asusd.rs    proxy for xyz.ljones.AsusArmoury (one iface, many paths)
│   └── supergfx.rs proxy for org.supergfxctl.Daemon
└── sysfs.rs        /sys/firmware/acpi/platform_profile read/write
```

## Notes on the asusd D-Bus surface

`asusd` exposes a uniform `xyz.ljones.AsusArmoury` interface at one object path per attribute (`/xyz/ljones/asus_armoury/charge_mode`, `gpu_mux_mode`, `panel_overdrive`, `ppt_pl1_spl`, …). Each path advertises `Name`, `CurrentValue` (writable `i`), `PossibleValues` (`ai`), and `Min/Max/Default`. One proxy fits all of them — `dbus/asusd.rs::proxy_for(conn, PATH_*)`.

Re-introspect after asusctl updates — interface names and property layouts shift between major versions:

```bash
busctl --system tree xyz.ljones.Asusd
busctl --system introspect xyz.ljones.Asusd /xyz/ljones/asus_armoury/charge_mode
```

## Notes on supergfxctl

Mode enum ordinals are taken from `supergfxctl -s` output. Verify against the daemon's source before relying on them — they've shifted between versions. Switching out of Hybrid requires logout; switching to/from `AsusMuxDgpu` requires reboot. Surface `PendingUserAction` as an Adw toast.

## What this intentionally doesn't do

- Per-key Aura, Anime Matrix, NVIDIA OC/UV — already covered by `rog-control-center`.
- AMD CPU undervolt — use `ryzenadj` directly.
- BIOS update — `fwupd`.
- System tray — defer (GNOME's tray story is the AppIndicator extension, which not everyone has). Launch from the app menu / a custom keybinding.
