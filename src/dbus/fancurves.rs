// xyz.ljones.FanCurves at /xyz/ljones — fan curve management per profile.
//
// Profile enum (verified live via xyz.ljones.Platform.PlatformProfileChoices=[2,0,1]
// matched to the kernel's platform_profile_choices=[quiet,balanced,performance]):
//   0 = Balanced, 1 = Performance, 2 = Quiet
//
// FanCurveData returns Vec of (name, 8-temp, 8-duty, enabled). Temps in °C,
// duty in %. "enabled=false" means the firmware default curve is in use.

use serde::{Deserialize, Serialize};
use zbus::proxy;
use zbus::zvariant::Type;

pub type Octet = (u8, u8, u8, u8, u8, u8, u8, u8);

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct FanCurve {
    pub name: String,
    pub temps: Octet,
    pub duty: Octet,
    pub enabled: bool,
}

impl FanCurve {
    pub fn temp_points(&self) -> [u8; 8] {
        let (a, b, c, d, e, f, g, h) = self.temps;
        [a, b, c, d, e, f, g, h]
    }

    pub fn duty_points(&self) -> [u8; 8] {
        let (a, b, c, d, e, f, g, h) = self.duty;
        [a, b, c, d, e, f, g, h]
    }
}

#[proxy(
    interface = "xyz.ljones.FanCurves",
    default_service = "xyz.ljones.Asusd",
    default_path = "/xyz/ljones"
)]
pub trait FanCurves {
    fn fan_curve_data(&self, profile: u32) -> zbus::Result<Vec<FanCurve>>;

    fn set_curves_to_defaults(&self, profile: u32) -> zbus::Result<()>;

    fn set_fan_curves_enabled(&self, profile: u32, enabled: bool) -> zbus::Result<()>;

    fn set_profile_fan_curve_enabled(
        &self,
        profile: u32,
        fan: &str,
        enabled: bool,
    ) -> zbus::Result<()>;

    fn set_fan_curve(&self, profile: u32, curve: FanCurve) -> zbus::Result<()>;
}

pub fn fan_curve_data_blocking(profile: u32) -> zbus::Result<Vec<FanCurve>> {
    let conn = zbus::blocking::Connection::system()?;
    let proxy = FanCurvesProxyBlocking::new(&conn)?;
    proxy.fan_curve_data(profile)
}

pub fn set_curves_to_defaults_blocking(profile: u32) -> zbus::Result<()> {
    let conn = zbus::blocking::Connection::system()?;
    let proxy = FanCurvesProxyBlocking::new(&conn)?;
    proxy.set_curves_to_defaults(profile)
}

pub fn set_profile_fan_curve_enabled_blocking(
    profile: u32,
    fan: &str,
    enabled: bool,
) -> zbus::Result<()> {
    let conn = zbus::blocking::Connection::system()?;
    let proxy = FanCurvesProxyBlocking::new(&conn)?;
    proxy.set_profile_fan_curve_enabled(profile, fan, enabled)
}

pub fn set_fan_curve_blocking(profile: u32, curve: FanCurve) -> zbus::Result<()> {
    let conn = zbus::blocking::Connection::system()?;
    let proxy = FanCurvesProxyBlocking::new(&conn)?;
    proxy.set_fan_curve(profile, curve)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Balanced = 0,
    Performance = 1,
    Quiet = 2,
}

impl Profile {
    pub fn label(self) -> &'static str {
        match self {
            Profile::Quiet => "Quiet",
            Profile::Balanced => "Balanced",
            Profile::Performance => "Performance",
        }
    }

    pub const ALL: [Profile; 3] = [Profile::Quiet, Profile::Balanced, Profile::Performance];
}
