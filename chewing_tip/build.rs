use std::{env, process::Command};

fn main() -> anyhow::Result<()> {
    let out_dir = env::var("OUT_DIR").unwrap();
    let target = env::var("TARGET").unwrap();
    let midl = embed_resource::find_windows_sdk_tool("midl.exe").expect("cannot find midl.exe");
    let cl = cc::windows_registry::find_tool(&target, "cl.exe").expect("cannot find cl.exe");

    let _ = Command::new(midl)
        .envs(cl.env().iter().cloned())
        .arg("/client")
        .arg("none")
        .arg("/server")
        .arg("none")
        .arg("/out")
        .arg(&out_dir)
        .arg("idl/libime2.idl")
        .status()?;

    embed_resource::compile("ChewingTextService.rc", embed_resource::NONE).manifest_required()?;
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .define("_UNICODE", "1")
        .define("UNICODE", "1")
        .define("_CRT_SECURE_NO_WARNINGS", "1")
        .flag("/utf-8")
        .flag("/EHsc")
        .file("CClassFactory.cpp")
        .file("ChewingConfig.cpp")
        .file("ChewingTextService.cpp")
        .file("DllEntry.cpp")
        .file("EditSession.cpp")
        .file("KeyEvent.cpp")
        .file("LangBarButton.cpp")
        .file("TextService.cpp")
        .file("Utils.cpp")
        .include("../libchewing/include")
        .include(&out_dir)
        .compile("chewing_tip");
    Ok(())
}
