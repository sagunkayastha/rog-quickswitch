// xyz.ljones.Aura, exposed at /xyz/ljones/aura/<device-id>.
// The device-id suffix (e.g. 19b6_2_3 for this user's keyboard) varies by
// hardware, so the path is discovered at startup via Introspectable.

use zbus::proxy;

#[proxy(interface = "xyz.ljones.Aura", default_service = "xyz.ljones.Asusd")]
pub trait Aura {
    #[zbus(property)]
    fn brightness(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn set_brightness(&self, value: u32) -> zbus::Result<()>;

    #[zbus(property)]
    fn led_mode(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn set_led_mode(&self, value: u32) -> zbus::Result<()>;

    #[zbus(property)]
    fn supported_brightness(&self) -> zbus::Result<Vec<u32>>;

    #[zbus(property)]
    fn supported_basic_modes(&self) -> zbus::Result<Vec<u32>>;
}

const PARENT: &str = "/xyz/ljones/aura";

/// Returns the first concrete aura device path, e.g.
/// "/xyz/ljones/aura/19b6_2_3". Empty if no device is present.
pub fn discover_device_path() -> zbus::Result<Option<String>> {
    let conn = zbus::blocking::Connection::system()?;
    let introspectable =
        zbus::blocking::fdo::IntrospectableProxy::builder(&conn)
            .destination("xyz.ljones.Asusd")?
            .path(PARENT)?
            .build()?;
    let xml = introspectable.introspect()?;

    // Match `<node name="..."/>` — sufficient for asusd's emitted XML.
    let name = xml
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("<node name=\"")?;
            rest.split_once('"').map(|(name, _)| name.to_string())
        })
        .next();
    Ok(name.map(|n| format!("{PARENT}/{n}")))
}

fn proxy_at(path: &str) -> zbus::Result<AuraProxyBlocking<'static>> {
    let conn = zbus::blocking::Connection::system()?;
    AuraProxyBlocking::builder(&conn)
        .path(path.to_string())?
        .build()
}

pub fn get_brightness_blocking(path: &str) -> zbus::Result<u32> {
    proxy_at(path)?.brightness()
}

pub fn set_brightness_blocking(path: &str, value: u32) -> zbus::Result<()> {
    proxy_at(path)?.set_brightness(value)
}

pub fn get_led_mode_blocking(path: &str) -> zbus::Result<u32> {
    proxy_at(path)?.led_mode()
}

pub fn set_led_mode_blocking(path: &str, value: u32) -> zbus::Result<()> {
    proxy_at(path)?.set_led_mode(value)
}

pub fn get_supported_brightness_blocking(path: &str) -> zbus::Result<Vec<u32>> {
    proxy_at(path)?.supported_brightness()
}

pub fn get_supported_basic_modes_blocking(path: &str) -> zbus::Result<Vec<u32>> {
    proxy_at(path)?.supported_basic_modes()
}

/// Best-effort label for a basic-mode ordinal. Verified mapping for
/// SupportedBasicModes=[0,1,2,3,10] on the user's keyboard, where asusctl's
/// internal config listed modes in matching order as
/// [Static, Breathe, RainbowCycle, RainbowWave, Pulse]. Other ordinals fall
/// through to a generic "Mode N".
pub fn led_mode_label(value: u32) -> String {
    match value {
        0 => "Static".into(),
        1 => "Breathe".into(),
        2 => "Rainbow".into(),
        3 => "Wave".into(),
        10 => "Pulse".into(),
        n => format!("Mode {n}"),
    }
}
