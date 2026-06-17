use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    // Check for KWindowSystem (KF6)
    let kwindowsystem = pkg_config::Config::new()
        .atleast_version("6.0")
        .probe("KF6WindowSystem");

    // If KWindowSystem is found, link against it and enable the blur effect.
    if let Ok(lib) = &kwindowsystem {
        for lib_name in &lib.libs {
            println!("cargo:rustc-link-lib={lib_name}");
        }
        for path in &lib.link_paths {
            println!("cargo:rustc-link-search=native={}", path.display());
        }
        println!("cargo:rustc-cfg=has_kwindoweffects");
    } else {
        println!("cargo:warning=KF6WindowSystem not found, blur effects will be limited");
    }

    // Build the cxx-qt bridge and register the Backend QObject in the GhostPad QML module.
    // kwin_blur.cpp uses QWindow, so the Gui module is required (Qml is added automatically).
    let mut builder = CxxQtBuilder::new_qml_module(QmlModule::new("GhostPad").version(1, 0))
        .file("src/bridge.rs")
        .qt_module("Gui");

    builder = unsafe {
        builder.cc_builder(move |cc| {
            cc.file("src/kwin_blur.cpp");
            cc.include("src");
            if let Ok(lib) = &kwindowsystem {
                for path in &lib.include_paths {
                    cc.include(path);
                }
                cc.define("HAS_KWINDOWEFFECTS", None);
            }
        })
    };

    let _ = builder.build();
}
