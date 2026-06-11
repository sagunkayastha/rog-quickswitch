// Fan curves: G-Helper-style chart editor.
//   - Profile selector (Quiet / Balanced / Performance) at top
//   - Single chart showing CPU (yellow) and GPU (cyan) curves overlaid
//   - Drag any point on the chart to edit; the node shows its % value
//   - Apply writes both curves; Reset restores firmware defaults
//
// No spinbox grid / per-fan switches — the chart is the only control, the way
// G-Helper does it. Apply enables the curves for the profile.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4 as gtk;
use gtk::prelude::*;

use crate::dbus::fancurves::{self, FanCurve, Octet, Profile};
use crate::panels::{self, ProfileBus};
use crate::sysfs::platform_profile;

const CHART_W: i32 = 400;
const CHART_H: i32 = 190;
const PAD_LEFT: f64 = 26.0;
const PAD_BOTTOM: f64 = 16.0;
const PAD_TOP: f64 = 14.0;
const PAD_RIGHT: f64 = 6.0;
const TEMP_MIN: f64 = 30.0;
const TEMP_MAX: f64 = 100.0;
const DUTY_MAX: f64 = 100.0;
const DRAG_HIT_RADIUS: f64 = 18.0;
const POINT_RADIUS: f64 = 4.5;

const CPU_RGB: (f64, f64, f64) = (1.0, 0.757, 0.027); // #ffc107
const GPU_RGB: (f64, f64, f64) = (0.306, 0.773, 1.0); // #4ec5ff

fn fan_color(name: &str) -> (f64, f64, f64) {
    if name.eq_ignore_ascii_case("CPU") {
        CPU_RGB
    } else {
        GPU_RGB
    }
}

fn current_profile() -> Profile {
    match platform_profile::current().unwrap_or_default().as_str() {
        "performance" => Profile::Performance,
        "quiet" => Profile::Quiet,
        _ => Profile::Balanced,
    }
}

pub fn build(bus: ProfileBus) -> gtk::Box {
    let initial = current_profile();
    let active: Rc<Cell<Profile>> = Rc::new(Cell::new(initial));

    let outer = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .build();

    let selector = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::Fill)
        .hexpand(true)
        .build();
    selector.add_css_class("linked");

    let fan_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .build();

    let mut toggles: Vec<(Profile, gtk::ToggleButton)> = Vec::with_capacity(3);
    let mut first: Option<gtk::ToggleButton> = None;
    for profile in Profile::ALL {
        let btn = gtk::ToggleButton::with_label(profile.label());
        btn.set_hexpand(true);
        if let Some(g) = &first {
            btn.set_group(Some(g));
        }
        if profile == initial {
            btn.set_active(true);
        }

        let active_clone = active.clone();
        let fan_box_clone = fan_box.clone();
        btn.connect_toggled(move |b| {
            if !b.is_active() {
                return;
            }
            active_clone.set(profile);
            refill(&fan_box_clone, profile);
        });

        selector.append(&btn);
        toggles.push((profile, btn.clone()));
        if first.is_none() {
            first = Some(btn);
        }
    }

    outer.append(&selector);
    outer.append(&fan_box);

    refill(&fan_box, initial);

    let toggles_rc: Rc<Vec<(Profile, gtk::ToggleButton)>> = Rc::new(toggles);
    let toggles_clone = toggles_rc.clone();
    bus.subscribe(move |new_profile| {
        for (p, btn) in toggles_clone.iter() {
            if *p == new_profile {
                btn.set_active(true);
                break;
            }
        }
    });

    panels::section("FAN CURVES", &outer)
}

fn refill(container: &gtk::Box, profile: Profile) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    match fancurves::fan_curve_data_blocking(profile as u32) {
        Ok(curves) if !curves.is_empty() => {
            container.append(&build_editor(profile, curves, container.clone()));
        }
        Ok(_) => {
            container.append(&gtk::Label::new(Some("No fan curve data for this profile.")));
        }
        Err(e) => {
            eprintln!("fan_curve_data({profile:?}): {e}");
            container.append(&gtk::Label::new(Some("Failed to read fan curves.")));
        }
    }
}

fn tuple_from(arr: &[u8; 8]) -> Octet {
    (arr[0], arr[1], arr[2], arr[3], arr[4], arr[5], arr[6], arr[7])
}

#[derive(Clone)]
struct FanState {
    name: String,
    temps: Rc<RefCell<[u8; 8]>>,
    duty: Rc<RefCell<[u8; 8]>>,
}

fn build_editor(profile: Profile, curves: Vec<FanCurve>, parent: gtk::Box) -> gtk::Box {
    let outer = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .build();

    // Build fan states (state cells only — the chart is the sole editor).
    let mut states: Vec<FanState> = Vec::with_capacity(curves.len());
    for curve in &curves {
        states.push(FanState {
            name: curve.name.clone(),
            temps: Rc::new(RefCell::new(curve.temp_points())),
            duty: Rc::new(RefCell::new(curve.duty_points())),
        });
    }

    // Chart drawing area.
    let chart = gtk::DrawingArea::builder()
        .content_width(CHART_W)
        .content_height(CHART_H)
        .build();
    {
        let states_for_chart = states.clone();
        chart.set_draw_func(move |_, cr, w, h| {
            draw_chart(cr, w as f64, h as f64, &states_for_chart);
        });
    }

    // GestureDrag on the chart for point-dragging — writes state directly.
    let states_rc: Rc<Vec<FanState>> = Rc::new(states.clone());
    let drag_target: Rc<Cell<Option<(usize, usize)>>> = Rc::new(Cell::new(None));
    let drag_start: Rc<Cell<(f64, f64)>> = Rc::new(Cell::new((0.0, 0.0)));

    let gesture = gtk::GestureDrag::new();
    {
        let states = states_rc.clone();
        let drag_target = drag_target.clone();
        let drag_start = drag_start.clone();
        let chart = chart.clone();
        gesture.connect_drag_begin(move |_, x, y| {
            drag_start.set((x, y));
            let w = chart.width() as f64;
            let h = chart.height() as f64;
            drag_target.set(find_nearest(&states, x, y, w, h));
        });
    }
    {
        let states = states_rc.clone();
        let drag_target = drag_target.clone();
        let drag_start = drag_start.clone();
        let chart = chart.clone();
        gesture.connect_drag_update(move |_, dx, dy| {
            if let Some((fan_idx, point_idx)) = drag_target.get() {
                let (sx, sy) = drag_start.get();
                let w = chart.width() as f64;
                let h = chart.height() as f64;
                let (temp, duty) = screen_to_data(sx + dx, sy + dy, w, h);
                let s = &states[fan_idx];

                let mut temps = s.temps.borrow_mut();
                let mut duties = s.duty.borrow_mut();

                // Keep points from crossing horizontally: clamp this point's
                // temperature between its neighbours.
                let lo_t = if point_idx > 0 { temps[point_idx - 1] } else { 0 };
                let hi_t = if point_idx < 7 { temps[point_idx + 1] } else { 110 };
                let nt = (temp.clamp(0.0, 110.0).round() as u8).max(lo_t).min(hi_t);
                let nd = duty.clamp(0.0, 100.0).round() as u8;
                temps[point_idx] = nt;
                duties[point_idx] = nd;

                // Duty must be non-decreasing with temperature. Pushing a point
                // up drags every higher-temp point up to at least this value;
                // pulling it down drags every lower-temp point down to it.
                for j in (point_idx + 1)..8 {
                    if duties[j] < nd {
                        duties[j] = nd;
                    }
                }
                for j in 0..point_idx {
                    if duties[j] > nd {
                        duties[j] = nd;
                    }
                }

                drop(temps);
                drop(duties);
                chart.queue_draw();
            }
        });
    }
    {
        let drag_target = drag_target.clone();
        gesture.connect_drag_end(move |_, _, _| {
            drag_target.set(None);
        });
    }
    chart.add_controller(gesture);

    // Hover cursor: switch to "grab" near a point, default elsewhere.
    let motion = gtk::EventControllerMotion::new();
    {
        let states = states_rc.clone();
        let chart = chart.clone();
        motion.connect_motion(move |_, x, y| {
            let w = chart.width() as f64;
            let h = chart.height() as f64;
            let hit = find_nearest(&states, x, y, w, h).is_some();
            chart.set_cursor_from_name(Some(if hit { "grab" } else { "default" }));
        });
    }
    chart.add_controller(motion);

    outer.append(&chart);

    // Color legend.
    let legend = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .halign(gtk::Align::Center)
        .build();
    for state in &states {
        legend.append(&legend_swatch(&state.name));
    }
    outer.append(&legend);

    // Bottom action row.
    let actions = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .halign(gtk::Align::Center)
        .margin_top(2)
        .build();

    let apply = gtk::Button::with_label("Apply");
    apply.add_css_class("apply-btn");
    {
        let states = states.clone();
        apply.connect_clicked(move |_| {
            for s in &states {
                let curve = FanCurve {
                    name: s.name.clone(),
                    temps: tuple_from(&s.temps.borrow()),
                    duty: tuple_from(&s.duty.borrow()),
                    enabled: true,
                };
                if let Err(e) = fancurves::set_fan_curve_blocking(profile as u32, curve) {
                    eprintln!("set_fan_curve({}): {}", s.name, e);
                    continue;
                }
                if let Err(e) = fancurves::set_profile_fan_curve_enabled_blocking(
                    profile as u32,
                    &s.name,
                    true,
                ) {
                    eprintln!("enable after apply: {e}");
                }
            }
        });
    }

    let reset = gtk::Button::with_label("Reset firmware defaults");
    reset.add_css_class("flat");
    {
        let parent = parent.clone();
        reset.connect_clicked(move |_| {
            if let Err(e) = fancurves::set_curves_to_defaults_blocking(profile as u32) {
                eprintln!("fan curves reset: {e}");
                return;
            }
            refill(&parent, profile);
        });
    }

    actions.append(&apply);
    actions.append(&reset);
    outer.append(&actions);

    outer
}

fn legend_swatch(name: &str) -> gtk::Box {
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(4)
        .build();
    let swatch = gtk::DrawingArea::builder()
        .content_width(10)
        .content_height(10)
        .valign(gtk::Align::Center)
        .build();
    let (r, g, b) = fan_color(name);
    swatch.set_draw_func(move |_, cr, w, h| {
        cr.set_source_rgba(r, g, b, 1.0);
        cr.rectangle(0.0, 0.0, w as f64, h as f64);
        let _ = cr.fill();
    });
    let label = gtk::Label::new(Some(name));
    label.add_css_class("endpoint-caption");
    row.append(&swatch);
    row.append(&label);
    row
}

fn screen_to_data(x: f64, y: f64, w: f64, h: f64) -> (f64, f64) {
    let plot_l = PAD_LEFT;
    let plot_r = w - PAD_RIGHT;
    let plot_t = PAD_TOP;
    let plot_b = h - PAD_BOTTOM;
    let plot_w = plot_r - plot_l;
    let plot_h = plot_b - plot_t;
    let temp = TEMP_MIN + (x - plot_l) / plot_w * (TEMP_MAX - TEMP_MIN);
    let duty = (plot_b - y) / plot_h * DUTY_MAX;
    (temp, duty)
}

fn data_to_screen(temp: f64, duty: f64, w: f64, h: f64) -> (f64, f64) {
    let plot_l = PAD_LEFT;
    let plot_r = w - PAD_RIGHT;
    let plot_t = PAD_TOP;
    let plot_b = h - PAD_BOTTOM;
    let plot_w = plot_r - plot_l;
    let plot_h = plot_b - plot_t;
    let x = plot_l + plot_w * ((temp - TEMP_MIN) / (TEMP_MAX - TEMP_MIN)).clamp(0.0, 1.0);
    let y = plot_b - plot_h * (duty / DUTY_MAX).clamp(0.0, 1.0);
    (x, y)
}

fn find_nearest(states: &[FanState], x: f64, y: f64, w: f64, h: f64) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize, f64)> = None;
    for (fan_idx, state) in states.iter().enumerate() {
        let t = *state.temps.borrow();
        let d = *state.duty.borrow();
        for i in 0..8 {
            let (px, py) = data_to_screen(t[i] as f64, d[i] as f64, w, h);
            let dist2 = (x - px).powi(2) + (y - py).powi(2);
            if best.map(|b| dist2 < b.2).unwrap_or(true) {
                best = Some((fan_idx, i, dist2));
            }
        }
    }
    best.and_then(|(f, i, d2)| {
        if d2.sqrt() < DRAG_HIT_RADIUS {
            Some((f, i))
        } else {
            None
        }
    })
}

fn draw_chart(cr: &gtk::cairo::Context, w: f64, h: f64, states: &[FanState]) {
    let plot_l = PAD_LEFT;
    let plot_r = w - PAD_RIGHT;
    let plot_t = PAD_TOP;
    let plot_b = h - PAD_BOTTOM;
    let plot_w = plot_r - plot_l;
    let plot_h = plot_b - plot_t;

    cr.set_font_size(9.0);

    cr.set_source_rgba(0.0, 0.0, 0.0, 0.25);
    cr.rectangle(plot_l, plot_t, plot_w, plot_h);
    let _ = cr.fill();

    cr.set_source_rgba(1.0, 1.0, 1.0, 0.08);
    cr.set_line_width(1.0);
    for pct in [25u8, 50, 75] {
        let y = plot_b - plot_h * (pct as f64 / 100.0);
        cr.move_to(plot_l, y);
        cr.line_to(plot_r, y);
        let _ = cr.stroke();
    }

    cr.set_source_rgba(0.55, 0.55, 0.55, 1.0);
    for pct in [0u8, 50, 100] {
        let y = plot_b - plot_h * (pct as f64 / 100.0);
        cr.move_to(2.0, y + 3.0);
        let _ = cr.show_text(&format!("{pct}%"));
    }
    for temp in (30u32..=100).step_by(5) {
        let x = plot_l
            + plot_w * ((temp as f64 - TEMP_MIN) / (TEMP_MAX - TEMP_MIN)).clamp(0.0, 1.0);
        let label = format!("{temp}");
        let ext = cr.text_extents(&label).map(|e| e.width()).unwrap_or(8.0);
        cr.move_to(x - ext / 2.0, h - 2.0);
        let _ = cr.show_text(&label);
    }

    for state in states {
        let (r, g, b) = fan_color(&state.name);
        let t = *state.temps.borrow();
        let d = *state.duty.borrow();

        cr.set_source_rgba(r, g, b, 0.92);
        cr.set_line_width(2.0);
        for (i, (&temp, &dt)) in t.iter().zip(d.iter()).enumerate() {
            let (x, y) = data_to_screen(temp as f64, dt as f64, w, h);
            if i == 0 {
                cr.move_to(x, y);
            } else {
                cr.line_to(x, y);
            }
        }
        let _ = cr.stroke();

        // Nodes with their duty% drawn above, G-Helper style.
        for (&temp, &dt) in t.iter().zip(d.iter()) {
            let (x, y) = data_to_screen(temp as f64, dt as f64, w, h);
            cr.set_source_rgba(r, g, b, 1.0);
            cr.arc(x, y, POINT_RADIUS, 0.0, std::f64::consts::TAU);
            let _ = cr.fill();

            let label = format!("{dt}");
            let ext = cr.text_extents(&label).map(|e| e.width()).unwrap_or(8.0);
            cr.move_to(x - ext / 2.0, (y - 7.0).max(plot_t + 8.0));
            let _ = cr.show_text(&label);
        }
    }
}
