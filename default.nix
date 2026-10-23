{
  nightly ? false,
  system ? builtins.currentSystem,
  versionFile ? ./nix/version.nix,
}:
let
  sources = import ./npins;
  v = import versionFile;
  version = v.version + lib.optionalString nightly "-rc.${v.rev}";

  pkgsBase = import sources.nixpkgs {
    inherit system;
    overlays = [ (import sources.rust-overlay) ];
  };

  chewing-nix = import sources.chewing-nix { pkgs = pkgsBase; };
  crane = import sources.crane { pkgs = pkgsBase; };

  pkgs = pkgsBase.extend (
    _final: _prev: {
      inherit (chewing-nix) llvmMingw;
      inherit crane;
    }
  );

  inherit (pkgs) lib;
  fs = pkgs.lib.fileset;

  targets = {
    x86 = "i686-pc-windows-gnullvm";
    x64 = "x86_64-pc-windows-gnullvm";
    arm64 = "aarch64-pc-windows-gnullvm";
  };

  tips = lib.mapAttrs (
    _name: target: pkgs.callPackage ./nix/chewing-tip.nix { inherit target; }
  ) targets;

  chewing-cli = pkgs.callPackage ./nix/chewing-cli.nix { target = targets.x64; };

  prefs = pkgs.callPackage (sources.windows-chewing-preferences + "/package.nix") { };
  editor = pkgs.callPackage (sources.windows-chewing-editor + "/package.nix") { };

  wix-cli = pkgs.callPackage ./nix/wix-cli.nix { };
  data = pkgs.callPackage ./nix/libchewing-data.nix { };

  # https://github.com/NixOS/nixpkgs/issues/555837
  mono = pkgs.fetchurl rec {
    version = "10.4.1";
    url = "https://dl.winehq.org/wine/wine-mono/${version}/wine-mono-${version}-x86.msi";
    hash = "sha256-Bx9LKIfhyXoR15H/PWW+lCnu1t7EwnCIiL/VRro1jiM=";
  };
in
pkgs.stdenvNoCC.mkDerivation {
  pname = "windows-chewing-tsf";
  version = "26.9.0.0";
  src = fs.toSource {
    root = ./.;
    fileset = fs.unions [
      ./tip/rc/im.chewing.Chewing.ico
      ./installer
      ./platform
    ];
  };
  nativeBuildInputs = [
    pkgs.glibcLocales
    pkgs.wine64Packages.stableFull
    pkgs.unzip
  ];
  dontConfigure = true;
  dontFixup = true;
  buildPhase = ''
    runHook preBuild

    export FONTCONFIG_FILE=$PWD/fc.conf
    export LANG=zh_TW.UTF-8
    export WINEPREFIX=$PWD/wine
    export WINEDEBUG=-all

    # Make Fontconfig happy to reduce noise.
    cat >fc.conf <<EOF
    <?xml version="1.0"?>
    <!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
    <fontconfig />
    EOF

    wineboot -i
    wine ${mono}
    wine ${wix-cli}

    mkdir build
    cp -a installer build/installer

    mkdir -p build/installer/x86
    mkdir -p build/installer/x64
    mkdir -p build/installer/Dictionary

    cp ${tips.x86}/chewing_tip.dll build/installer/x86/
    cp ${tips.x64}/chewing_tip.dll build/installer/x64/chewing_tip_x64.dll
    cp ${tips.arm64}/chewing_tip.dll build/installer/x64/chewing_tip_arm64.dll
    cp platform/arm64x/chewing_tip.dll build/installer/x64/
    unzip -j ${data} -d build/installer/Dictionary
    cp tip/rc/im.chewing.Chewing.ico build/installer/chewing.ico
    cp ${tips.x64}/*.exe build/installer/
    cp ${prefs}/*.exe build/installer/
    cp ${editor}/*.exe build/installer/
    cp ${editor}/*.dll build/installer/
    cp ${chewing-cli}/bin/chewing-cli.exe build/installer/chewing-cli.exe

    # MSI engine does not like timestamp 0
    touch build/installer/*
    touch build/installer/x86/*
    touch build/installer/x64/*
    touch build/installer/Dictionary/*

    pushd build/installer
    wine wix build -acceptEula wix7 -arch x64 -culture zh-TW \
        -o ../../windows-chewing-tsf-${version}.msi -pdbtype none \
        windows-chewing-tsf.wxs
    popd

    runHook postBuild
  '';
  installPhase = ''
    runHook preInstall

    mkdir $out
    cp windows-chewing-tsf-${version}.msi $out/

    runHook postInstall
  '';
}
