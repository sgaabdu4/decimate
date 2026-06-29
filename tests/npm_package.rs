use std::fs;
use std::path::Path;

use serde_json::Value;

#[test]
fn npm_package_exposes_decimate_bin() -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string("package.json")?;
    let package = serde_json::from_str::<Value>(&source)?;

    assert_eq!(package["name"], "@sgaabdu4/decimate");
    assert_eq!(package["bin"]["decimate"], "npm/bin/decimate.js");
    assert_eq!(
        package["scripts"]["postinstall"],
        "node npm/scripts/postinstall.js"
    );
    assert!(Path::new("npm/bin/decimate.js").is_file());
    assert!(Path::new("npm/scripts/postinstall.js").is_file());

    Ok(())
}
