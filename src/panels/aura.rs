// Aura keyboard backlight: brightness + preset mode.
// Per-key RGB and Anime Matrix stay in rog-control-center.

use gtk4 as gtk;
use gtk::prelude::*;

use crate::dbus::aura;
use crate::panels;

const BRIGHTNESS_LABELS: &[&str] = &["Off", "Low", "Med", "High"];

pub fn build() -> gtk::Box {
    let path = match aura::discover_device_path() {
        Ok(Some(p)) => p,
        _ => {
            let label = gtk::Label::builder()
                .label("No Aura device found")
                .halign(gtk::Align::Start)
                .build();
            label.add_css_class("endpoint-caption");
            return panels::section("AURA", &label);
        }
    };

    let inner = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .build();

    inner.append(&build_brightness_row(&path));
    inner.append(&build_mode_row(&path));

    panels::section("AURA", &inner)
}

fn build_brightness_row(path: &str) -> gtk::Box {
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .build();

    let label = gtk::Label::builder()
        .label("Brightness")
        .halign(gtk::Align::Start)
        .width_chars(10)
        .xalign(0.0)
        .build();

    let buttons_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .hexpand(true)
        .build();
    buttons_box.add_css_class("linked");

    let current = aura::get_brightness_blocking(path).unwrap_or(0);
    let supported = aura::get_supported_brightness_blocking(path).unwrap_or_else(|_| vec![0, 1, 2, 3]);

    let mut first: Option<gtk::ToggleButton> = None;
    for value in &supported {
        let value = *value;
        let lbl = BRIGHTNESS_LABELS
            .get(value as usize)
            .copied()
            .unwrap_or("?");
        let btn = gtk::ToggleButton::with_label(lbl);
        btn.set_hexpand(true);
        if let Some(g) = &first {
            btn.set_group(Some(g));
        }
        if value == current {
            btn.set_active(true);
        }

        let path_owned = path.to_string();
        btn.connect_toggled(move |b| {
            if !b.is_active() {
                return;
            }
            if let Err(e) = aura::set_brightness_blocking(&path_owned, value) {
                eprintln!("aura brightness: {e}");
            }
        });

        buttons_box.append(&btn);
        if first.is_none() {
            first = Some(btn);
        }
    }

    row.append(&label);
    row.append(&buttons_box);
    row
}

fn build_mode_row(path: &str) -> gtk::Box {
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .build();

    let label = gtk::Label::builder()
        .label("Effect")
        .halign(gtk::Align::Start)
        .width_chars(10)
        .xalign(0.0)
        .build();

    let buttons_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .hexpand(true)
        .build();
    buttons_box.add_css_class("linked");

    let current = aura::get_led_mode_blocking(path).unwrap_or(0);
    let supported = aura::get_supported_basic_modes_blocking(path).unwrap_or_else(|_| vec![0]);

    let mut first: Option<gtk::ToggleButton> = None;
    for value in &supported {
        let value = *value;
        let btn = gtk::ToggleButton::with_label(&aura::led_mode_label(value));
        btn.set_hexpand(true);
        if let Some(g) = &first {
            btn.set_group(Some(g));
        }
        if value == current {
            btn.set_active(true);
        }

        let path_owned = path.to_string();
        btn.connect_toggled(move |b| {
            if !b.is_active() {
                return;
            }
            if let Err(e) = aura::set_led_mode_blocking(&path_owned, value) {
                eprintln!("aura led_mode: {e}");
            }
        });

        buttons_box.append(&btn);
        if first.is_none() {
            first = Some(btn);
        }
    }

    row.append(&label);
    row.append(&buttons_box);
    row
}
