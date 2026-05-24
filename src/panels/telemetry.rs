// Live sensors rendered as a dense horizontal grid: each metric is a
// (title, value) column. Missing sensors render as "—" so the layout stays
// stable when the dGPU power-cycles.

use std::path::PathBuf;
use std::rc::Rc;

use gtk4 as gtk;
use gtk::prelude::*;

use crate::sysfs::hwmon;

struct Probe {
    title: &'static str,
    value: gtk::Label,
    sample: Box<dyn Fn() -> Option<String>>,
}

pub fn build() -> gtk::Box {
    let outer = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .build();
    outer.add_css_class("section");

    let probes = discover();

    let grid = gtk::Grid::builder()
        .column_spacing(18)
        .row_spacing(0)
        .halign(gtk::Align::Center)
        .build();

    for (i, p) in probes.iter().enumerate() {
        let col = i as i32;

        let title = gtk::Label::builder()
            .label(p.title)
            .halign(gtk::Align::Center)
            .build();
        title.add_css_class("metric-label");

        p.value.set_halign(gtk::Align::Center);
        p.value.add_css_class("metric-value");

        grid.attach(&title, col, 1, 1, 1);
        grid.attach(&p.value, col, 0, 1, 1);
    }

    outer.append(&grid);

    let probes = Rc::new(probes);
    tick(&probes);

    let probes_tick = probes.clone();
    glib::timeout_add_seconds_local(1, move || {
        tick(&probes_tick);
        glib::ControlFlow::Continue
    });

    outer
}

fn tick(probes: &[Probe]) {
    for p in probes {
        let value = (p.sample)().unwrap_or_else(|| "—".to_string());
        p.value.set_text(&value);
    }
}

fn temp_probe(title: &'static str, path: PathBuf, label: Option<&'static str>) -> Probe {
    let value = gtk::Label::new(Some("…"));
    let sample: Box<dyn Fn() -> Option<String>> = Box::new(move || {
        let c = label
            .and_then(|l| hwmon::read_temp_c_by_label(&path, l))
            .or_else(|| hwmon::read_temp_c(&path, 1))?;
        Some(format!("{c:.0}°"))
    });
    Probe { title, value, sample }
}

fn fan_probe(title: &'static str, path: PathBuf, idx: u32) -> Probe {
    let value = gtk::Label::new(Some("…"));
    let sample: Box<dyn Fn() -> Option<String>> = Box::new(move || {
        hwmon::read_fan_rpm(&path, idx).map(|rpm| {
            if rpm == 0 {
                "off".to_string()
            } else {
                format!("{rpm}")
            }
        })
    });
    Probe { title, value, sample }
}

fn discover() -> Vec<Probe> {
    let mut probes = Vec::new();

    for e in hwmon::find(&["k10temp"]) {
        probes.push(temp_probe("CPU", e.path, Some("Tctl")));
    }
    for e in hwmon::find(&["amdgpu"]) {
        probes.push(temp_probe("iGPU", e.path, Some("edge")));
    }
    for e in hwmon::find(&["nvidia"]) {
        probes.push(temp_probe("dGPU", e.path, None));
    }

    let fan_hosts = hwmon::find(&[
        "asus_custom_fan_curve",
        "asus",
        "asus-nb-wmi",
        "asus_wmi",
    ]);
    for e in fan_hosts {
        let mut added = false;
        for (idx, title) in [(1u32, "CPU rpm"), (2u32, "GPU rpm")] {
            if e.path.join(format!("fan{idx}_input")).exists() {
                probes.push(fan_probe(title, e.path.clone(), idx));
                added = true;
            }
        }
        if added {
            break;
        }
    }

    if probes.is_empty() {
        let value = gtk::Label::new(Some("no hwmon"));
        probes.push(Probe {
            title: "Sensors",
            value,
            sample: Box::new(|| None),
        });
    }

    probes
}
