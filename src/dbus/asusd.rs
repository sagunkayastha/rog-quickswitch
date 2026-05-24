// asusd exposes one interface (xyz.ljones.AsusArmoury) at many object paths under
// /xyz/ljones/asus_armoury/<attr> — every attribute uses the same shape:
//
//   Name: s           (e.g. "ChargeMode", "PanelOverdrive", "GpuMuxMode")
//   CurrentValue: i   (writable)
//   PossibleValues: ai
//   MinValue/MaxValue/DefaultValue/ScalarIncrement: i
//   AvailableAttrs: as
//   QueuedGpuValue: i
//   methods: ApplyQueuedGpuValue() -> b, RestoreDefault()
//
// One proxy serves every knob — construct with the right path.

use zbus::proxy;

#[proxy(
    interface = "xyz.ljones.AsusArmoury",
    default_service = "xyz.ljones.Asusd"
)]
pub trait AsusArmoury {
    #[zbus(property)]
    fn name(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn current_value(&self) -> zbus::Result<i32>;

    #[zbus(property)]
    fn set_current_value(&self, value: i32) -> zbus::Result<()>;

    #[zbus(property)]
    fn possible_values(&self) -> zbus::Result<Vec<i32>>;

    #[zbus(property)]
    fn min_value(&self) -> zbus::Result<i32>;

    #[zbus(property)]
    fn max_value(&self) -> zbus::Result<i32>;

    #[zbus(property)]
    fn default_value(&self) -> zbus::Result<i32>;

    fn apply_queued_gpu_value(&self) -> zbus::Result<bool>;

    fn restore_default(&self) -> zbus::Result<()>;
}

// Well-known object paths. Add more as panels come online.
pub const PATH_CHARGE_MODE: &str = "/xyz/ljones/asus_armoury/charge_mode";
pub const PATH_PANEL_OVERDRIVE: &str = "/xyz/ljones/asus_armoury/panel_overdrive";
pub const PATH_GPU_MUX_MODE: &str = "/xyz/ljones/asus_armoury/gpu_mux_mode";
pub const PATH_DGPU_DISABLE: &str = "/xyz/ljones/asus_armoury/dgpu_disable";
pub const PATH_NV_DYNAMIC_BOOST: &str = "/xyz/ljones/asus_armoury/nv_dynamic_boost";
pub const PATH_NV_TEMP_TARGET: &str = "/xyz/ljones/asus_armoury/nv_temp_target";
pub const PATH_PPT_PL1_SPL: &str = "/xyz/ljones/asus_armoury/ppt_pl1_spl";
pub const PATH_PPT_PL2_SPPT: &str = "/xyz/ljones/asus_armoury/ppt_pl2_sppt";
pub const PATH_PPT_PL3_FPPT: &str = "/xyz/ljones/asus_armoury/ppt_pl3_fppt";

pub async fn proxy_for<'a>(
    conn: &'a zbus::Connection,
    path: &'static str,
) -> zbus::Result<AsusArmouryProxy<'a>> {
    AsusArmouryProxy::builder(conn).path(path)?.build().await
}

/// Synchronous helper for one-shot GTK callbacks. The blocking call returns
/// within ~ms on the local bus, so freezing the UI for that long is acceptable.
pub fn set_attribute_value_blocking(path: &'static str, value: i32) -> zbus::Result<()> {
    let conn = zbus::blocking::Connection::system()?;
    let proxy = AsusArmouryProxyBlocking::builder(&conn).path(path)?.build()?;
    proxy.set_current_value(value)
}

pub fn get_attribute_value_blocking(path: &'static str) -> zbus::Result<i32> {
    let conn = zbus::blocking::Connection::system()?;
    let proxy = AsusArmouryProxyBlocking::builder(&conn).path(path)?.build()?;
    proxy.current_value()
}

pub fn get_attribute_default_blocking(path: &'static str) -> zbus::Result<i32> {
    let conn = zbus::blocking::Connection::system()?;
    let proxy = AsusArmouryProxyBlocking::builder(&conn).path(path)?.build()?;
    proxy.default_value()
}

pub fn get_attribute_min_blocking(path: &'static str) -> zbus::Result<i32> {
    let conn = zbus::blocking::Connection::system()?;
    let proxy = AsusArmouryProxyBlocking::builder(&conn).path(path)?.build()?;
    proxy.min_value()
}

// xyz.ljones.Platform on /xyz/ljones — owns platform_profile (and EPP, fan-policy
// linkage, etc). Profile enum values are asusd's: 0=Balanced, 1=Performance,
// 2=Quiet, 3=LowPower. Going through D-Bus rather than direct sysfs because
// /sys/firmware/acpi/platform_profile is root-owned; asusd ships the polkit rule.
#[proxy(
    interface = "xyz.ljones.Platform",
    default_service = "xyz.ljones.Asusd",
    default_path = "/xyz/ljones"
)]
pub trait Platform {
    #[zbus(property)]
    fn platform_profile(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn set_platform_profile(&self, value: u32) -> zbus::Result<()>;
}

pub const PROFILE_BALANCED: u32 = 0;
pub const PROFILE_PERFORMANCE: u32 = 1;
pub const PROFILE_QUIET: u32 = 2;

pub fn set_platform_profile_blocking(value: u32) -> zbus::Result<()> {
    let conn = zbus::blocking::Connection::system()?;
    let proxy = PlatformProxyBlocking::new(&conn)?;
    proxy.set_platform_profile(value)
}
