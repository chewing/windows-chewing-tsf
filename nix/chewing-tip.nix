{
  crane,
  lib,
  llvmMingw,
  target ? "x86_64-pc-windows-gnullvm",
  nightly ? true,
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
  CHEWING_BUILD_TYPE = if nightly then "development" else "stable";
  RUSTFLAGS = "-Ctarget-feature=+crt-static";
  CARGO_BUILD_TARGET = target;
  TARGET_CC = "${llvmMingw}/bin/clang";
  RC = "${llvmMingw}/bin/llvm-rc";
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
      "-p chewing_tip -p tsfreg"
    else
      "-p chewing_tip";
  doNotPostBuildInstallCargoBinaries = true;
  doCheck = false;
  installPhaseCommand = ''
    mkdir $out
    cp target/${target}/release/{*.dll,*.exe} $out/
  '';
}
