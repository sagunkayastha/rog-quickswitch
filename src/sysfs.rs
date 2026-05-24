pub mod hwmon {
    // Walks /sys/class/hwmon to find devices whose `name` matches one of `wanted`.
    // Caller picks specific temp/fan inputs by index or by label. Sensors that
    // disappear at runtime (e.g. nvidia hwmon while dGPU is suspended) return None
    // rather than erroring.

    use std::fs;
    use std::path::{Path, PathBuf};

    pub struct Entry {
        pub path: PathBuf,
        pub name: String,
    }

    pub fn find(wanted: &[&str]) -> Vec<Entry> {
        let mut out = Vec::new();
        let Ok(dir) = fs::read_dir("/sys/class/hwmon") else { return out };
        for e in dir.flatten() {
            let path = e.path();
            if let Ok(raw) = fs::read_to_string(path.join("name")) {
                let name = raw.trim().to_string();
                if wanted.iter().any(|w| *w == name) {
                    out.push(Entry { path, name });
                }
            }
        }
        out
    }

    pub fn read_temp_c(path: &Path, idx: u32) -> Option<f64> {
        let raw = fs::read_to_string(path.join(format!("temp{idx}_input"))).ok()?;
        Some(raw.trim().parse::<f64>().ok()? / 1000.0)
    }

    pub fn read_temp_c_by_label(path: &Path, label: &str) -> Option<f64> {
        for i in 1..=8u32 {
            if let Ok(lbl) = fs::read_to_string(path.join(format!("temp{i}_label"))) {
                if lbl.trim().eq_ignore_ascii_case(label) {
                    return read_temp_c(path, i);
                }
            }
        }
        None
    }

    pub fn read_fan_rpm(path: &Path, idx: u32) -> Option<u32> {
        let raw = fs::read_to_string(path.join(format!("fan{idx}_input"))).ok()?;
        raw.trim().parse().ok()
    }
}

pub mod platform_profile {
    use anyhow::{Context, Result};
    use std::fs;

    const PATH: &str = "/sys/firmware/acpi/platform_profile";
    const CHOICES: &str = "/sys/firmware/acpi/platform_profile_choices";

    pub fn current() -> Result<String> {
        Ok(fs::read_to_string(PATH).context("read platform_profile")?.trim().to_string())
    }

    pub fn choices() -> Result<Vec<String>> {
        Ok(fs::read_to_string(CHOICES)
            .context("read platform_profile_choices")?
            .split_whitespace()
            .map(String::from)
            .collect())
    }

    pub fn set(name: &str) -> Result<()> {
        // Writing platform_profile requires CAP_SYS_ADMIN or a polkit-allowed daemon.
        // asusd already sets up a polkit rule so its bus methods don't need root;
        // direct sysfs write here will fail for unprivileged users. Prefer the
        // asusd D-Bus path once wired — keeping this for the v0 wiring demo.
        fs::write(PATH, name).context("write platform_profile")
    }
}
