use std::{env, path::PathBuf, process::Command};

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
    let idl_client = PathBuf::from(&out_dir).join("libime2_i.c");

    embed_resource::compile("ChewingTextService.rc", embed_resource::NONE).manifest_required()?;
    cc::Build::new()
        .cpp(true)
        .std("c++20")
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
        .file("TextService.cpp")
        .file("Utils.cpp")
        .file(idl_client)
        .include("../libchewing/include")
        .include(&out_dir)
        .compile("chewing_tip");

    println!("cargo::rerun-if-changed=idl/libime2.idl");
    println!("cargo::rerun-if-changed=CClassFactory.cpp");
    println!("cargo::rerun-if-changed=CClassFactory.h");
    println!("cargo::rerun-if-changed=ChewingConfig.cpp");
    println!("cargo::rerun-if-changed=ChewingConfig.h");
    println!("cargo::rerun-if-changed=ChewingTextService.cpp");
    println!("cargo::rerun-if-changed=ChewingTextService.h");
    println!("cargo::rerun-if-changed=DllEntry.cpp");
    println!("cargo::rerun-if-changed=EditSession.cpp");
    println!("cargo::rerun-if-changed=EditSession.h");
    println!("cargo::rerun-if-changed=KeyEvent.cpp");
    println!("cargo::rerun-if-changed=KeyEvent.h");
    println!("cargo::rerun-if-changed=TextService.cpp");
    println!("cargo::rerun-if-changed=TextService.h");
    println!("cargo::rerun-if-changed=Utils.cpp");
    println!("cargo::rerun-if-changed=Utils.h");
    Ok(())
}
