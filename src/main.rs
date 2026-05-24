mod dbus;
mod panels;
mod sysfs;
mod window;

use adw::prelude::*;
use libadwaita as adw;

const APP_ID: &str = "dev.kayasth.RogQuickswitch";

fn main() -> glib::ExitCode {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let _guard = runtime.enter();

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(window::build);
    app.run()
}
