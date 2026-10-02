// SPDX-License-Identifier: GPL-3.0-only

//! Opt-in desktop integration test for a real Plasma Wayland session.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use thunderbird_tray::kde_wayland::KdeWindowControl;
use thunderbird_tray::lifecycle::LifecycleService;
use thunderbird_tray::window::{ActivationOutcome, DesktopEnvironment, WindowControl};

#[test]
#[ignore = "requires a Plasma Wayland session and a visible Thunderbird window"]
fn controls_a_real_thunderbird_window_without_x11() {
    let (attached_tx, _attached_rx) = mpsc::channel();
    let (report_tx, report_rx) = mpsc::channel();
    let (configuration_tx, _configuration_rx) = mpsc::channel();
    let _lifecycle = LifecycleService::claim(attached_tx, report_tx, configuration_tx)
        .expect("the test must be the only thunderbird-tray process");
    let backend = KdeWindowControl::connect(&DesktopEnvironment::from_process(), report_rx)
        .expect("the KWin runtime probe must complete")
        .expect("the test must run in a supported Plasma Wayland session");

    let capabilities = backend.capabilities();
    assert!(capabilities.detect);
    assert!(capabilities.activate);
    assert!(capabilities.hide);
    assert!(capabilities.show);
    assert!(
        backend.detect().expect("window detection must complete"),
        "start Thunderbird before running this ignored test"
    );

    for _ in 0..3 {
        backend.hide().expect("Thunderbird must minimize");
        thread::sleep(Duration::from_millis(250));
        backend.show().expect("Thunderbird must restore");
        thread::sleep(Duration::from_millis(250));
    }
    assert_eq!(
        backend.activate().expect("Thunderbird must activate"),
        ActivationOutcome::Activated
    );
}
