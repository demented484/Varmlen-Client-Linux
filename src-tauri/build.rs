fn main() {
    // Windows: embed a manifest requesting administrator elevation. The VPN data
    // plane (Wintun adapter, routing table, firewall killswitch) all require
    // admin, so the whole app runs elevated (UAC prompt at launch).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>"#;
        let attrs = tauri_build::Attributes::new().windows_attributes(
            tauri_build::WindowsAttributes::new().app_manifest(manifest),
        );
        tauri_build::try_build(attrs).expect("failed to run tauri-build");
    } else {
        tauri_build::build()
    }
}
