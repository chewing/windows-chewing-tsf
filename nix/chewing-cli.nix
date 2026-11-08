{
  fetchFromCodeberg,
  llvmMingw,
  rust-bin,
  crane,
  target ? "x86_64-pc-windows-gnullvm",
}:
let
  craneLib = crane.overrideToolchain (
    p:
    rust-bin.stable.latest.default.override {
      targets = [ target ];
    }
  );
  version = "v0.14.0-rc.1";
  src = fetchFromCodeberg {
    owner = "chewing";
    repo = "libchewing";
    rev = version;
    hash = "sha256-d2qAo7i+uwxwwbxoSZi3j0UUWoNEWyAyKD9x3wnmF2o=";
  };
in
craneLib.buildPackage {
  inherit src;
  nativeBuildInputs = [
    llvmMingw
  ];
  RUSTFLAGS = "-Ctarget-feature=+crt-static";
  CARGO_BUILD_TARGET = target;
  cargoBuildExtraArgs = "-p chewing-cli";
  doCheck = false;
}
