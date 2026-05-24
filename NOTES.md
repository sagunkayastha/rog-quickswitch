# Implementation notes

Working log of non-obvious decisions and reference values. The code is the source
of truth; this file captures the *why* and the bits that would be tedious to
re-derive.

Target hardware: ROG Zephyrus G15 **GA503RM** (Ryzen 9 6900HS + RTX 3060).

## D-Bus surface (asusd)

Bus name: `xyz.ljones.Asusd`. All interfaces live under `/xyz/ljones`.

Fan curves — `xyz.ljones.FanCurves`:

| method                          | signature                       | notes |
|--------------------------------|---------------------------------|-------|
| `FanCurveData`                  | `u → a(s(yyyyyyyy)(yyyyyyyy)b)` | returns `Vec<(name, 8 temps, 8 duty%, enabled)>` |
| `SetFanCurve`                   | `u(s(yyyyyyyy)(yyyyyyyy)b) → ·` | write a single fan's curve |
| `SetCurvesToDefaults`           | `u → ·`                         | reset profile to firmware defaults |
| `SetFanCurvesEnabled`           | `ub → ·`                        | enable/disable all fans in a profile |
| `SetProfileFanCurveEnabled`     | `usb → ·`                       | enable/disable a single fan |

**Profile integer mapping (non-obvious).** Verified live via
`xyz.ljones.Platform.PlatformProfileChoices = [2, 0, 1]` matched to the kernel's
`platform_profile_choices = [quiet, balanced, performance]`:

| `u` value | Profile      |
|-----------|--------------|
| 0         | Balanced     |
| 1         | Performance  |
| 2         | Quiet        |

Do **not** assume sequential 0/1/2 = quiet/balanced/perf — that's wrong.
`src/dbus/fancurves.rs` encodes this.

## Power profile semantics (Mode → asus_armoury)

| Mode        | `platform_profile` | `ppt_pl1_spl` |
|-------------|--------------------|---------------|
| Ultra Eco   | quiet              | 15 W (firmware floor for 6900HS) |
| Quiet       | quiet              | 25 W (firmware default) |
| Balanced    | balanced           | 25 W |
| Performance | performance        | 25 W |

`pl2`/`pl3` not touched — they reject values below 35 W anyway.

Ultra Eco also offers a logout + supergfx switch to Integrated.

## G-Helper visual rework

Window is a fixed-size compact GTK4 + libadwaita app styled to look like
G-Helper rather than a stock GNOME prefs page:

- Force-dark via `adw::StyleManager`
- Custom CSS in `src/style.css`: #1f1f1f bg, #ffc107 yellow accent for active
  toggles & buttons, compact paddings, monospaced spinbutton text
- Replaced `AdwPreferencesPage`/`PreferencesGroup` with a vertical `GtkBox` of
  small uppercase section labels + dense content rows
- Telemetry rendered as a horizontal grid (small dim label + bold yellow value)
  rather than `ActionRow`s
- Toggles use `.linked` button bars; CSS gives them G-Helper's pill look
  (yellow when checked, dark when unchecked)

## ProfileBus (cross-panel signalling)

`src/panels/mod.rs` exposes a tiny single-threaded pub/sub (`Rc<RefCell<Vec<Box<dyn Fn>>>>`)
so the top **Performance Profile** pill row notifies the **Fan Curves** panel
when the user changes profile. Mapping in `panels::profile::Mode::fan_profile`:

- Ultra Eco / Quiet → `fancurves::Profile::Quiet`
- Balanced → `Balanced`
- Performance → `Performance`

The fan-curve panel still has its own internal selector — change either, the
top→bottom direction is linked. Fan-curve initial profile is read from
`/sys/firmware/acpi/platform_profile` so the panel opens on the currently
active profile (not always Balanced).

## Fan-curve editor (G-Helper-style)

Single combined chart with both fans overlaid:

- **CPU** = yellow `#ffc107`, **GPU** = cyan `#4ec5ff`
- Axis labels: 0/50/100% on Y, 30/50/70/90° on X
- 8 (temp, duty%) points per fan, drawn as filled dots on the line
- Hover near a point → cursor switches to **grab**
- `gtk::GestureDrag` hit-tests within an 18-pixel radius, then on drag-update
  converts mouse coords back to (temp, duty) and writes to the matching
  spinbutton. The spinbutton's `value_changed` handler is the single owner of
  state updates and `queue_draw`, so dragging, typing, and reset all go through
  one wiring.
- One **Apply** button writes both curves via `SetFanCurve`, then calls
  `SetProfileFanCurveEnabled(profile, fan, true)` per fan to make the custom
  curve take effect immediately.
- **Reset firmware defaults** calls `SetCurvesToDefaults` then refills the
  panel so the spinbuttons reflect the restored values.

The editor enforces *no* monotonicity — firmware accepts whatever you send.

## Reference fan curves

### asusd firmware defaults on GA503RM (PWM 0–255)

| Profile     | Fan | Temps (°C)                        | PWM                              |
|-------------|-----|-----------------------------------|----------------------------------|
| Quiet       | CPU | 58 62 66 70 74 78 82 82           | 2 10 17 25 33 53 58 58           |
| Quiet       | GPU | 52 56 60 64 68 72 76 76           | 2 17 25 33 43 43 66 66           |
| Balanced    | CPU | 0 55 59 63 67 71 75 79            | 10 25 33 53 58 76 96 119         |
| Balanced    | GPU | 0 52 56 60 64 68 72 76            | 17 25 43 43 66 84 104 130        |
| Performance | CPU | 52 56 60 64 68 72 76 76           | 53 58 76 96 119 140 255 255      |
| Performance | GPU | 44 49 54 59 64 69 74 74           | 43 66 84 104 130 150 255 **135** |

Two firmware oddities visible above:

- Balanced curves have `temp = 0` for the first two points (floor clamps, not
  real breakpoints).
- Performance GPU curve drops from 255 → 135 at its top point (non-monotonic).
  This is in the firmware data itself.

### G-Helper "Silent" curve (applied as our custom Quiet)

From `seerge/g-helper`, `app/AppConfig.cs:GetDefaultCurve` (encoded as 0–100%).
No GA503RM-specific override exists; G-Helper uses the same defaults for every
model.

| Fan | Temps (°C)                  | Duty %                       |
|-----|------------------------------|------------------------------|
| CPU | 30 49 59 66 71 80 90 100    | 0 0 3 12 20 28 34 41        |
| GPU | 30 49 59 66 71 80 90 100    | 0 0 4 17 27 35 40 45        |

Applied via:

```
asusctl fan-curve --mod-profile Quiet --fan cpu --data "30c:0%,49c:0%,59c:3%,66c:12%,71c:20%,80c:28%,90c:34%,100c:41%"
asusctl fan-curve --mod-profile Quiet --fan gpu --data "30c:0%,49c:0%,59c:4%,66c:17%,71c:27%,80c:35%,90c:40%,100c:45%"
asusctl fan-curve --mod-profile Quiet --enable-fan-curves true
```

To revert: `asusctl fan-curve --mod-profile Quiet --default`.

## Telemetry sensors

`src/panels/telemetry.rs` polls hwmon every 1 s:

- `k10temp` → CPU (Tctl)
- `amdgpu` → iGPU (edge)
- `nvidia` → dGPU (only when awake)
- Fans: ASUS EC under one of `asus_custom_fan_curve`, `asus`, `asus-nb-wmi`,
  `asus_wmi` (first match wins) — `fan1_input` = CPU, `fan2_input` = GPU.
