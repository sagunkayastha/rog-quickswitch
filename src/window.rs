use adw::prelude::*;
use gtk4 as gtk;
use gtk::gdk;
use libadwaita as adw;

use crate::panels;

const STYLE: &str = include_str!("style.css");

pub fn build(app: &adw::Application) {
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);

    let provider = gtk::CssProvider::new();
    provider.load_from_string(STYLE);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(10)
        .margin_end(10)
        .build();

    let bus = panels::ProfileBus::new();

    content.append(&panels::telemetry::build());
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    content.append(&panels::profile::build(bus.clone()));
    content.append(&panels::gpu_mode::build());
    content.append(&panels::charge_limit::build());
    content.append(&panels::panel_overdrive::build());
    content.append(&panels::aura::build());
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    content.append(&panels::fan_curve::build(bus.clone()));

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&content)
        .build();

    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_show_title(false);
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scrolled));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("ROG Quickswitch")
        .default_width(460)
        .default_height(720)
        .content(&toolbar)
        .build();
    window.present();
}
