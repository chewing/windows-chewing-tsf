{
  crane,
  lib,
  llvmMingw,
  target ? "x86_64-pc-windows-gnullvm",
}:
let
  fs = lib.fileset;
  version = (fromTOML (builtins.readFile ../Cargo.toml)).workspace.package.version;
  craneLib = crane.overrideToolchain (
    p:
    p.rust-bin.stable.latest.default.override {
      targets = [
        "x86_64-pc-windows-gnullvm"
        "aarch64-pc-windows-gnullvm"
        "i686-pc-windows-gnullvm"
      ];
    }
  );
in
craneLib.buildPackage {
  inherit version;
  pname = "chewing-tip";
  strictDeps = true;
  RUSTFLAGS = "-Ctarget-feature=+crt-static";
  CARGO_BUILD_TARGET = target;
  TARGET_CC = "${llvmMingw}/bin/clang";
  src = fs.toSource {
    root = ../.;
    fileset = fs.unions [
      ../Cargo.lock
      ../Cargo.toml
      ../crates
      ../tip
      ../xtask
    ];
  };
  cargoVendorDir = craneLib.vendorCargoDeps { cargoLock = ../Cargo.lock; };
  nativeBuildInputs = [
    llvmMingw
  ];
  cargoBuildExtraArgs =
    if target == "x86_64-pc-windows-gnullvm" then
      "-p chewing_tip -p chewing_tip_host -p tsfreg"
    else
      "-p chewing_tip";
  doNotPostBuildInstallCargoBinaries = true;
  doCheck = false;
  installPhaseCommand = ''
    mkdir $out
    cp target/${target}/release/{*.dll,*.exe} $out/
  '';
}
