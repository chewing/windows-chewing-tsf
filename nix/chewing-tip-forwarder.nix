{
  lib,
  llvmMingw,
  runCommand,
  versionSuffix
}:
let
  fs = lib.fileset;
in
runCommand "chewing_tip.dll" {
  nativeBuildInputs = [
    llvmMingw
  ];
}
''
cat >chewing_tip_arm64.def <<EOF
EXPORTS
DllMain = chewing_tip_arm64_${versionSuffix}.DllMain
DllGetClassObject = chewing_tip_arm64_${versionSuffix}.DllGetClassObject
EOF
cat >chewing_tip_x64.def <<EOF
EXPORTS
DllMain = chewing_tip_x64_${versionSuffix}.DllMain
DllGetClassObject = chewing_tip_x64_${versionSuffix}.DllGetClassObject
EOF
aarch64-w64-mingw32-clang -v -marm64x chewing_tip_x64.def -Wl,--native-def=chewing_tip_arm64.def -shared -o $out
''
