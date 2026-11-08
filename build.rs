use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let icon = manifest_dir.join("res").join("icon.ico");
    let manifest = manifest_dir.join("res").join("app.manifest");

    let icon = icon.to_string_lossy().replace('\\', "/");
    let manifest = manifest.to_string_lossy().replace('\\', "/");

    // Resource id 1 is what the code loads with LoadImageW(MAKEINTRESOURCE(1)).
    let rc = format!(
        r#"#include <winver.h>

1 ICON "{icon}"
1 24 "{manifest}"

1 VERSIONINFO
 FILEVERSION 1,0,0,0
 PRODUCTVERSION 1,0,0,0
 FILEFLAGSMASK 0x3fL
 FILEFLAGS 0x0L
 FILEOS VOS_NT_WINDOWS32
 FILETYPE VFT_APP
 FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName", "LightSwitch Lite"
            VALUE "FileDescription", "Automatic light/dark theme switching for Windows"
            VALUE "FileVersion", "1.0.0.0"
            VALUE "InternalName", "LightSwitchLite"
            VALUE "OriginalFilename", "LightSwitchLite.exe"
            VALUE "ProductName", "LightSwitch Lite"
            VALUE "ProductVersion", "1.0.0.0"
            VALUE "LegalCopyright", "MIT License"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#
    );

    let rc_path = out_dir.join("lightswitch.rc");
    fs::write(&rc_path, rc).expect("write generated .rc");
    if let Err(error) = embed_resource::compile(&rc_path, embed_resource::NONE).manifest_optional()
    {
        panic!("resource compilation failed: {error}");
    }

    println!("cargo:rerun-if-changed=res/icon.ico");
    println!("cargo:rerun-if-changed=res/app.manifest");
    println!("cargo:rerun-if-changed=build.rs");
}
