// SPDX-License-Identifier: GPL-3.0-only

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_directory = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo provides CARGO_MANIFEST_DIR"),
    );
    let identifiers_path = manifest_directory.join("../../identifiers.json");
    println!("cargo:rerun-if-changed={}", identifiers_path.display());

    let contents = fs::read_to_string(&identifiers_path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", identifiers_path.display()));
    let identifiers: serde_json::Value = serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("invalid {}: {error}", identifiers_path.display()));

    export_identifier(
        &identifiers,
        "applicationId",
        "THUNDERBIRD_TRAY_APPLICATION_ID",
    );
    export_identifier(
        &identifiers,
        "lifecycleInterface",
        "THUNDERBIRD_TRAY_LIFECYCLE_INTERFACE",
    );
    export_identifier(
        &identifiers,
        "lifecycleObjectPath",
        "THUNDERBIRD_TRAY_LIFECYCLE_OBJECT_PATH",
    );
}

fn export_identifier(identifiers: &serde_json::Value, key: &str, environment_name: &str) {
    let value = identifiers[key]
        .as_str()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| panic!("identifiers.json is missing {key}"));
    assert!(
        !value.contains(['\n', '\r']),
        "identifier {key} must fit one Cargo directive"
    );
    println!("cargo:rustc-env={environment_name}={value}");
}
