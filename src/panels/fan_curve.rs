// Fan curves: G-Helper-style combined editor with draggable chart points.
//   - Profile selector (Quiet / Balanced / Performance) at top
//   - Single chart showing CPU (yellow) and GPU (cyan) curves overlaid
//   - Drag any point on the chart to edit; matching spinboxes update live
//   - Per-fan compact rows of 8 temp + 8 duty% spinboxes
//   - One Apply button writes both curves
//   - "Reset to firmware defaults" resets the whole profile

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4 as gtk;
use gtk::prelude::*;

use crate::dbus::fancurves::{self, FanCurve, Octet, Profile};
use crate::panels::{self, ProfileBus};
use crate::sysfs::platform_profile;

const CHART_W: i32 = 400;
const CHART_H: i32 = 150;
const PAD_LEFT: f64 = 26.0;
const PAD_BOTTOM: f64 = 16.0;
const PAD_TOP: f64 = 6.0;
const PAD_RIGHT: f64 = 6.0;
const TEMP_MIN: f64 = 30.0;
const TEMP_MAX: f64 = 100.0;
const DUTY_MAX: f64 = 100.0;
const DRAG_HIT_RADIUS: f64 = 18.0;
const POINT_RADIUS: f64 = 4.0;

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
    enable: gtk::Switch,
    temp_spins: Vec<gtk::SpinButton>,
    duty_spins: Vec<gtk::SpinButton>,
}

fn build_editor(profile: Profile, curves: Vec<FanCurve>, parent: gtk::Box) -> gtk::Box {
    let outer = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .build();

    // Build fan states (state cells + spinbuttons) up front so the chart's
    // gesture handler can drive both.
    let mut states: Vec<FanState> = Vec::with_capacity(curves.len());
    for curve in &curves {
        let temps = Rc::new(RefCell::new(curve.temp_points()));
        let duty = Rc::new(RefCell::new(curve.duty_points()));
        let enable = gtk::Switch::builder()
            .active(curve.enabled)
            .valign(gtk::Align::Center)
            .build();

        let mut temp_spins = Vec::with_capacity(8);
        let mut duty_spins = Vec::with_capacity(8);
        for i in 0..8 {
            let t = gtk::SpinButton::with_range(0.0, 110.0, 1.0);
            t.set_value(temps.borrow()[i] as f64);
            t.set_width_chars(3);
            temp_spins.push(t);

            let d = gtk::SpinButton::with_range(0.0, 100.0, 1.0);
            d.set_value(duty.borrow()[i] as f64);
            d.set_width_chars(3);
            duty_spins.push(d);
        }

        states.push(FanState {
            name: curve.name.clone(),
            temps,
            duty,
            enable,
            temp_spins,
            duty_spins,
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

    // Wire spinbutton value_changed -> state + chart redraw.
    for state in &states {
        for i in 0..8 {
            {
                let temps = state.temps.clone();
                let chart = chart.clone();
                state.temp_spins[i].connect_value_changed(move |s| {
                    temps.borrow_mut()[i] = s.value().clamp(0.0, 255.0).round() as u8;
                    chart.queue_draw();
                });
            }
            {
                let duty = state.duty.clone();
                let chart = chart.clone();
                state.duty_spins[i].connect_value_changed(move |s| {
                    duty.borrow_mut()[i] = s.value().clamp(0.0, 100.0).round() as u8;
                    chart.queue_draw();
                });
            }
        }
    }

    // GestureDrag on the chart for point-dragging.
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
                let x = sx + dx;
                let y = sy + dy;
                let w = chart.width() as f64;
                let h = chart.height() as f64;
                let (temp, duty) = screen_to_data(x, y, w, h);
                let s = &states[fan_idx];
                s.temp_spins[point_idx].set_value(temp.clamp(0.0, 110.0).round());
                s.duty_spins[point_idx].set_value(duty.clamp(0.0, 100.0).round());
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

    // Per-fan T/% spinbox rows.
    for state in &states {
        outer.append(&build_fan_rows(state));
    }

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
                s.enable.set_active(true);
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

fn build_fan_rows(state: &FanState) -> gtk::Box {
    let outer = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .build();

    let header = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    let name = gtk::Label::builder()
        .label(format!("{} fan", state.name))
        .halign(gtk::Align::Start)
        .hexpand(true)
        .build();
    name.add_css_class("fan-heading");
    header.append(&name);
    header.append(&state.enable);
    outer.append(&header);

    let grid = gtk::Grid::builder().column_spacing(2).row_spacing(2).build();

    let t_label = gtk::Label::builder()
        .label("T")
        .halign(gtk::Align::End)
        .build();
    t_label.add_css_class("endpoint-caption");
    grid.attach(&t_label, 0, 0, 1, 1);

    let d_label = gtk::Label::builder()
        .label("%")
        .halign(gtk::Align::End)
        .build();
    d_label.add_css_class("endpoint-caption");
    grid.attach(&d_label, 0, 1, 1, 1);

    for i in 0..8 {
        grid.attach(&state.temp_spins[i], (i + 1) as i32, 0, 1, 1);
        grid.attach(&state.duty_spins[i], (i + 1) as i32, 1, 1, 1);
    }

    outer.append(&grid);
    outer
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
    for temp in [30u32, 50, 70, 90] {
        let x = plot_l
            + plot_w * ((temp as f64 - TEMP_MIN) / (TEMP_MAX - TEMP_MIN)).clamp(0.0, 1.0);
        cr.move_to(x - 8.0, h - 2.0);
        let _ = cr.show_text(&format!("{temp}°"));
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

        cr.set_source_rgba(r, g, b, 1.0);
        for (&temp, &dt) in t.iter().zip(d.iter()) {
            let (x, y) = data_to_screen(temp as f64, dt as f64, w, h);
            cr.arc(x, y, POINT_RADIUS, 0.0, std::f64::consts::TAU);
            let _ = cr.fill();
        }
    }
}
