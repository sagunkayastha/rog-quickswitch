// Performance profile: segmented button group.
//   Ultra Eco  → platform_profile=quiet + ppt_pl1_spl=15W + offer switch to iGPU-only
//   Quiet      → platform_profile=quiet + ppt_pl1_spl=firmware default - 10W
//   Balanced   → platform_profile=balanced + ppt_pl1_spl=firmware default
//   Performance→ platform_profile=performance + ppt_pl1_spl=firmware default
//
// Firmware default is read once via asusd (DefaultValue on ppt_pl1_spl). On
// Ryzen 9 6900HS that's 35 W. Going below 15 W is firmware-blocked.
// pl2/pl3 left at defaults — they won't accept values below 35 W anyway.
//
// Ultra Eco asks (every time) whether to switch to Integrated and log out,
// unless already in Integrated mode.

use std::sync::OnceLock;

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

use crate::dbus::fancurves;
use crate::dbus::{asusd, supergfx};
use crate::panels::{self, ProfileBus};
use crate::sysfs::platform_profile;

const ULTRA_ECO_PPT: i32 = 15;
const QUIET_PPT_DELTA: i32 = 10;
const FALLBACK_DEFAULT_PPT: i32 = 35;

fn firmware_default_ppt() -> i32 {
    static CACHE: OnceLock<i32> = OnceLock::new();
    *CACHE.get_or_init(|| {
        asusd::get_attribute_default_blocking(asusd::PATH_PPT_PL1_SPL)
            .unwrap_or(FALLBACK_DEFAULT_PPT)
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    UltraEco,
    Quiet,
    Balanced,
    Performance,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::UltraEco => "Ultra Eco",
            Mode::Quiet => "Quiet",
            Mode::Balanced => "Balanced",
            Mode::Performance => "Performance",
        }
    }

    fn platform_profile(self) -> u32 {
        match self {
            Mode::UltraEco | Mode::Quiet => asusd::PROFILE_QUIET,
            Mode::Balanced => asusd::PROFILE_BALANCED,
            Mode::Performance => asusd::PROFILE_PERFORMANCE,
        }
    }

    fn ppt_pl1(self) -> i32 {
        let default = firmware_default_ppt();
        match self {
            Mode::UltraEco => ULTRA_ECO_PPT,
            Mode::Quiet => (default - QUIET_PPT_DELTA).max(ULTRA_ECO_PPT),
            Mode::Balanced | Mode::Performance => default,
        }
    }

    fn apply_power(self) {
        if let Err(e) = asusd::set_platform_profile_blocking(self.platform_profile()) {
            eprintln!("platform_profile: {e}");
        }
        if let Err(e) = asusd::set_attribute_value_blocking(asusd::PATH_PPT_PL1_SPL, self.ppt_pl1())
        {
            eprintln!("ppt_pl1_spl: {e}");
        }
    }

    fn fan_profile(self) -> fancurves::Profile {
        match self {
            Mode::UltraEco | Mode::Quiet => fancurves::Profile::Quiet,
            Mode::Balanced => fancurves::Profile::Balanced,
            Mode::Performance => fancurves::Profile::Performance,
        }
    }
}

fn detect_active() -> Mode {
    let profile = platform_profile::current().unwrap_or_default();
    let ppt = asusd::get_attribute_value_blocking(asusd::PATH_PPT_PL1_SPL)
        .unwrap_or_else(|_| firmware_default_ppt());
    match (profile.as_str(), ppt) {
        ("quiet", p) if p <= ULTRA_ECO_PPT => Mode::UltraEco,
        ("quiet", _) => Mode::Quiet,
        ("performance", _) => Mode::Performance,
        _ => Mode::Balanced,
    }
}

fn ask_logout(parent: Option<gtk::Window>) {
    let dialog = adw::AlertDialog::new(
        Some("Switch to iGPU-only mode?"),
        Some(
            "Ultra Eco works best with the discrete GPU disabled. \
             Switching GPU modes requires logging out. Continue?",
        ),
    );
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("logout", "Switch & Log Out");
    dialog.set_response_appearance("logout", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    dialog.connect_response(None, move |_d, response| {
        if response != "logout" {
            return;
        }
        if let Err(e) = supergfx::set_mode_blocking(supergfx::Mode::Integrated as u32) {
            eprintln!("supergfx SetMode: {e}");
            return;
        }
        if let Err(e) = std::process::Command::new("gnome-session-quit")
            .args(["--logout", "--no-prompt"])
            .spawn()
        {
            eprintln!("gnome-session-quit: {e}");
        }
    });

    dialog.present(parent.as_ref());
}

fn handle_ultra_eco_gpu(parent: Option<gtk::Window>) {
    match supergfx::get_mode_blocking() {
        Ok(m) if m == supergfx::Mode::Integrated as u32 => {
            // Already iGPU-only, nothing to do.
        }
        Ok(_) => ask_logout(parent),
        Err(e) => eprintln!("supergfx get_mode: {e}"),
    }
}

pub fn build(bus: ProfileBus) -> gtk::Box {
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::Fill)
        .hexpand(true)
        .build();
    row.add_css_class("linked");

    let modes = [Mode::UltraEco, Mode::Quiet, Mode::Balanced, Mode::Performance];
    let active = detect_active();

    let mut first: Option<gtk::ToggleButton> = None;
    for mode in modes {
        let btn = gtk::ToggleButton::with_label(mode.label());
        btn.set_hexpand(true);
        if let Some(g) = &first {
            btn.set_group(Some(g));
        }
        if mode == active {
            btn.set_active(true);
        }
        let bus_clone = bus.clone();
        btn.connect_toggled(move |b| {
            if !b.is_active() {
                return;
            }
            mode.apply_power();
            bus_clone.publish(mode.fan_profile());
            if mode == Mode::UltraEco {
                let parent = b.root().and_downcast::<gtk::Window>();
                handle_ultra_eco_gpu(parent);
            }
        });
        row.append(&btn);
        if first.is_none() {
            first = Some(btn);
        }
    }

    panels::section("PROFILE", &row)
}
