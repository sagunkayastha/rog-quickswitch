pub mod aura;
pub mod charge_limit;
pub mod fan_curve;
pub mod gpu_mode;
pub mod panel_overdrive;
pub mod profile;
pub mod telemetry;

use std::cell::RefCell;
use std::rc::Rc;

use gtk4 as gtk;
use gtk::prelude::*;

use crate::dbus::fancurves::Profile;

// Simple pub/sub used to link the top Profile selector and the Fan Curves
// panel. GTK is single-threaded, so Rc<RefCell<...>> is fine here.
#[derive(Clone, Default)]
pub struct ProfileBus {
    listeners: Rc<RefCell<Vec<Box<dyn Fn(Profile)>>>>,
}

impl ProfileBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe<F: Fn(Profile) + 'static>(&self, f: F) {
        self.listeners.borrow_mut().push(Box::new(f));
    }

    pub fn publish(&self, p: Profile) {
        for f in self.listeners.borrow().iter() {
            f(p);
        }
    }
}

pub fn section(title: &str, body: &impl IsA<gtk::Widget>) -> gtk::Box {
    let outer = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .build();
    outer.add_css_class("section");

    let label = gtk::Label::builder()
        .label(title)
        .halign(gtk::Align::Start)
        .build();
    label.add_css_class("section-title");
    outer.append(&label);
    outer.append(body);
    outer
}
