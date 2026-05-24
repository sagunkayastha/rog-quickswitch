// Battery charge mode (xyz.ljones.AsusArmoury → asus_armoury/charge_mode).
// Firmware exposes [0, 1, 2] as an enum. ASUS BIOS labels these as charging
// "modes" without a fixed published percent mapping; common convention is
// 0=Full, 1=Balanced (~80%), 2=Lifespan (~60%). Confirm in BIOS if exact
// numbers matter.

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

use crate::dbus::asusd;
use crate::panels;

struct Entry {
    label: &'static str,
    value: i32,
}

const ENTRIES: &[Entry] = &[
    Entry { label: "Full",     value: 0 },
    Entry { label: "Balanced", value: 1 },
    Entry { label: "Lifespan", value: 2 },
];

pub fn build() -> gtk::Box {
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::Fill)
        .hexpand(true)
        .build();
    row.add_css_class("linked");

    let current = asusd::get_attribute_value_blocking(asusd::PATH_CHARGE_MODE).unwrap_or(0);

    let mut first: Option<gtk::ToggleButton> = None;
    for entry in ENTRIES {
        let value = entry.value;
        let btn = gtk::ToggleButton::with_label(entry.label);
        btn.set_hexpand(true);
        if let Some(g) = &first {
            btn.set_group(Some(g));
        }
        if value == current {
            btn.set_active(true);
        }

        btn.connect_toggled(move |b| {
            if !b.is_active() {
                return;
            }
            if let Err(e) = asusd::set_attribute_value_blocking(asusd::PATH_CHARGE_MODE, value) {
                eprintln!("charge_mode: {e}");
            }
        });

        row.append(&btn);
        if first.is_none() {
            first = Some(btn);
        }
    }

    panels::section("BATTERY CHARGE", &row)
}
