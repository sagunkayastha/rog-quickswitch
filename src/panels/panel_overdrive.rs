// Panel overdrive: asus_armoury/panel_overdrive — 0=off, 1=on.

use gtk4 as gtk;
use gtk::prelude::*;

use crate::dbus::asusd;

pub fn build() -> gtk::Box {
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_top(2)
        .margin_bottom(2)
        .margin_start(4)
        .margin_end(4)
        .build();
    row.add_css_class("section");

    let label = gtk::Label::builder()
        .label("Panel overdrive")
        .halign(gtk::Align::Start)
        .hexpand(true)
        .build();

    let switch = gtk::Switch::builder()
        .valign(gtk::Align::Center)
        .build();
    let current = asusd::get_attribute_value_blocking(asusd::PATH_PANEL_OVERDRIVE).unwrap_or(0);
    switch.set_active(current == 1);

    switch.connect_active_notify(|s| {
        let value = if s.is_active() { 1 } else { 0 };
        if let Err(e) = asusd::set_attribute_value_blocking(asusd::PATH_PANEL_OVERDRIVE, value) {
            eprintln!("panel_overdrive: {e}");
        }
    });

    row.append(&label);
    row.append(&switch);
    row
}
