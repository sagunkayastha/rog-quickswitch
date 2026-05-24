// supergfxd: org.supergfxctl.Daemon at /org/supergfxctl/Gfx.
// Methods (verified via busctl):
//   Mode() -> u                  current mode
//   SetMode(u) -> u              returns PendingUserAction
//   Supported() -> au            available modes on this hardware
//   PendingMode() -> u
//   PendingUserAction() -> u
//   Vendor() -> s, Version() -> s, Power() -> u
//
// Mode enum values are defined in supergfxctl's source — verify against the
// running daemon's docs before shipping. From `supergfxctl -s` output today,
// this machine reports: [Integrated, Hybrid, AsusMuxDgpu].

use zbus::proxy;

#[proxy(
    interface = "org.supergfxctl.Daemon",
    default_service = "org.supergfxctl.Daemon",
    default_path = "/org/supergfxctl/Gfx"
)]
pub trait SuperGfx {
    fn mode(&self) -> zbus::Result<u32>;

    fn set_mode(&self, mode: u32) -> zbus::Result<u32>;

    fn supported(&self) -> zbus::Result<Vec<u32>>;

    fn pending_mode(&self) -> zbus::Result<u32>;

    fn pending_user_action(&self) -> zbus::Result<u32>;

    fn vendor(&self) -> zbus::Result<String>;

    fn version(&self) -> zbus::Result<String>;

    fn power(&self) -> zbus::Result<u32>;

    #[zbus(signal)]
    fn notify_gfx(&self, mode: u32) -> zbus::Result<()>;

    #[zbus(signal)]
    fn notify_action(&self, action: u32) -> zbus::Result<()>;
}

// Ordinals verified live against this machine's supergfxd via
//   busctl call org.supergfxctl.Daemon /org/supergfxctl/Gfx
//        org.supergfxctl.Daemon Supported   -> au [1, 0, 5]
// matched to `supergfxctl -s` -> [Integrated, Hybrid, AsusMuxDgpu].
// Vfio/AsusEgpu values are educated guesses (not used on this hardware).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Mode {
    Hybrid = 0,
    Integrated = 1,
    Vfio = 3,
    AsusEgpu = 4,
    AsusMuxDgpu = 5,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Hybrid => "Hybrid",
            Mode::Integrated => "Integrated",
            Mode::Vfio => "VFIO",
            Mode::AsusEgpu => "Asus eGPU",
            Mode::AsusMuxDgpu => "MUX → dGPU",
        }
    }
}

// Ordinals match supergfxctl 5.2.x's GfxRequiredUserAction
// (src/pci_device.rs in the supergfxctl repo). Earlier versions used
// different values — re-verify if upgrading the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PendingUserAction {
    Logout = 0,
    Reboot = 1,
    SwitchToIntegrated = 2,
    AsusEgpuDisable = 3,
    None = 4,
}

pub fn get_mode_blocking() -> zbus::Result<u32> {
    let conn = zbus::blocking::Connection::system()?;
    let proxy = SuperGfxProxyBlocking::new(&conn)?;
    proxy.mode()
}

pub fn set_mode_blocking(mode: u32) -> zbus::Result<u32> {
    let conn = zbus::blocking::Connection::system()?;
    let proxy = SuperGfxProxyBlocking::new(&conn)?;
    proxy.set_mode(mode)
}

pub fn get_supported_blocking() -> zbus::Result<Vec<u32>> {
    let conn = zbus::blocking::Connection::system()?;
    let proxy = SuperGfxProxyBlocking::new(&conn)?;
    proxy.supported()
}
