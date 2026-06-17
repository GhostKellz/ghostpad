use std::pin::Pin;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};
use ghostpad_core::{APP_ID, APP_NAME, APP_VERSION};

fn main() {
    configure_kde_wayland_environment();

    let mut app = QGuiApplication::new();
    {
        let app_pin = app.pin_mut();
        assign_application_metadata(app_pin);
    }

    let mut engine = QQmlApplicationEngine::new();
    let qml_url = main_qml_url();
    {
        let engine_pin = engine.pin_mut();
        engine_pin.load(&qml_url);
    }

    let exit_code = app.pin_mut().exec();
    std::process::exit(exit_code);
}

fn configure_kde_wayland_environment() {
    unsafe {
        std::env::set_var("QT_QUICK_CONTROLS_STYLE", "org.kde.desktop");
        std::env::set_var("QT_WAYLAND_DISABLE_WINDOWDECORATION", "1");
    }
}

fn assign_application_metadata(mut app: Pin<&mut QGuiApplication>) {
    let name = QString::from(APP_NAME);
    let version = QString::from(APP_VERSION);
    let organization = QString::from("GhostPad Project");
    let desktop_file = QString::from(format!("{APP_ID}.desktop"));

    app.as_mut().set_application_name(&name);
    app.as_mut().set_organization_name(&organization);
    app.as_mut().set_application_version(&version);
    QGuiApplication::set_desktop_file_name(&desktop_file);
}

fn main_qml_url() -> QUrl {
    // Check for installed QML path first (for packaged builds)
    let installed_path = std::path::PathBuf::from("/usr/share/ghostpad/qml/Main.qml");
    let dev_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("qml/Main.qml");

    let qml_path = if installed_path.exists() {
        installed_path
    } else {
        dev_path
    };

    let path = QString::from(qml_path.to_string_lossy().as_ref());
    QUrl::from_local_file(&path)
}
