set RUSTFLAGS=-Ctarget-feature=+crt-static
set SQLITE3_STATIC=1

cmake -B build\x86 -A Win32 -DBUILD_TESTING=OFF -DVCPKG_TARGET_TRIPLET=x86-windows-static
cmake --build build\x86 -t libchewing\data\data --config Release
cmake --build build\x86 -t ChewingPreferences --config Release

cargo build -p chewing_tip --release --target x86_64-pc-windows-msvc
cargo build -p chewing_tip --release --target i686-pc-windows-msvc
cargo build -p tsfreg --release --target i686-pc-windows-msvc

mkdir dist
mkdir build\installer
mkdir build\installer\assets
copy assets\* build\installer\assets\
copy installer\* build\installer\
copy chewing_tip\im.chewing.Chewing.ico build\installer\chewing.ico
mkdir build\installer\Dictionary
copy libchewing\data\*.dat build\installer\Dictionary\
copy build\x86\libchewing\data\*.dat build\installer\Dictionary\
mkdir build\installer\x86
copy target\i686-pc-windows-msvc\release\chewing_tip.dll build\installer\x86\
copy build\x86\ChewingPreferences\Release\*.exe build\installer\
copy build\x86\libchewing\chewing-cli.exe build\installer\
mkdir build\installer\x64
copy target\x86_64-pc-windows-msvc\release\chewing_tip.dll build\installer\x64
copy target\i686-pc-windows-msvc\release\tsfreg.exe build\installer\
pushd build\installer
msbuild -p:Configuration=Release -restore windows-chewing-tsf.wixproj
popd
copy build\installer\bin\Release\zh-TW\windows-chewing-tsf.msi dist\windows-chewing-tsf-unsigned.msi
