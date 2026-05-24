// GPU mode: Integrated / Hybrid / MUX → dGPU.
// Buttons mirror what `supergfxctl -s` reports as Supported. Greyed otherwise.
// Click → SetMode → the daemon returns a PendingUserAction. We surface it as
// a dialog asking "every time" per user's request.

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

use crate::dbus::supergfx::{self, PendingUserAction};
use crate::panels;

struct Entry {
    label: &'static str,
    mode: supergfx::Mode,
}

const ENTRIES: &[Entry] = &[
    Entry { label: "Integrated", mode: supergfx::Mode::Integrated },
    Entry { label: "Hybrid",     mode: supergfx::Mode::Hybrid },
    Entry { label: "MUX → dGPU", mode: supergfx::Mode::AsusMuxDgpu },
];

fn ask_pending(parent: Option<gtk::Window>, pending: u32) {
    let (title, body, action_label, cmd, args): (&str, &str, &str, &str, &[&str]) =
        match pending {
            x if x == PendingUserAction::Logout as u32 => (
                "Logout required",
                "GPU mode change is queued. Log out now to apply it?\n\n\
                 The switch won't happen until every desktop session has ended.",
                "Log Out",
                "gnome-session-quit",
                &["--logout", "--no-prompt"],
            ),
            x if x == PendingUserAction::Reboot as u32 => (
                "Reboot required",
                "MUX switching is queued. Reboot now to apply it?",
                "Reboot",
                "systemctl",
                &["reboot"],
            ),
            x if x == PendingUserAction::SwitchToIntegrated as u32 => (
                "Switch to Integrated first",
                "This mode requires switching to Integrated and logging out first. \
                 Log out now?",
                "Log Out",
                "gnome-session-quit",
                &["--logout", "--no-prompt"],
            ),
            _ => return,
        };

    let dialog = adw::AlertDialog::new(Some(title), Some(body));
    dialog.add_response("cancel", "Later");
    dialog.add_response("go", action_label);
    dialog.set_response_appearance("go", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    let cmd = cmd.to_string();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    dialog.connect_response(None, move |_, response| {
        if response != "go" {
            return;
        }
        if let Err(e) = std::process::Command::new(&cmd).args(&args).spawn() {
            eprintln!("{cmd}: {e}");
        }
    });

    dialog.present(parent.as_ref());
}

pub fn build() -> gtk::Box {
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::Fill)
        .hexpand(true)
        .build();
    row.add_css_class("linked");

    let current = supergfx::get_mode_blocking().unwrap_or(supergfx::Mode::Hybrid as u32);
    let supported = supergfx::get_supported_blocking().unwrap_or_default();

    let mut first: Option<gtk::ToggleButton> = None;
    for entry in ENTRIES {
        let value = entry.mode as u32;
        let btn = gtk::ToggleButton::with_label(entry.label);
        btn.set_hexpand(true);
        btn.set_sensitive(supported.contains(&value));
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
            match supergfx::set_mode_blocking(value) {
                Ok(pending) if pending != PendingUserAction::None as u32 => {
                    let parent = b.root().and_downcast::<gtk::Window>();
                    ask_pending(parent, pending);
                }
                Ok(_) => {}
                Err(e) => eprintln!("supergfx SetMode: {e}"),
            }
        });

        row.append(&btn);
        if first.is_none() {
            first = Some(btn);
        }
    }

    panels::section("GPU MODE", &row)
}
