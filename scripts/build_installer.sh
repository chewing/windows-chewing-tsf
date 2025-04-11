#!/bin/sh

set -x
set -e

set SQLITE3_STATIC=1

export MINGW_CHOST=x86_64-w64-mingw32
export SQLITE3_LIB_DIR=/ucrt64/lib

cargo +stable-x86_64-pc-windows-gnu build -p chewing_tip --release --target x86_64-pc-windows-gnu

export MINGW_CHOST=i686-w64-mingw32
export SQLITE3_LIB_DIR=/mingw32/lib

cargo +stable-i686-pc-windows-gnu install chewing-cli --target i686-pc-windows-gnu --root build
cargo +stable-i686-pc-windows-gnu   build -p chewing_tip --release --target i686-pc-windows-gnu
cargo +stable-i686-pc-windows-gnu   build -p chewing-preferences --release --target i686-pc-windows-gnu
cargo +stable-i686-pc-windows-gnu   build -p tsfreg --release --target i686-pc-windows-gnu

rm -rf dist build/installer
mkdir -p build/installer/assets
cp assets/* build/installer/assets/
cp installer/* build/installer/
cp chewing_tip/rc/im.chewing.Chewing.ico build/installer/chewing.ico

mkdir -p build/installer/Dictionary
cp libchewing/data/*.dat build/installer/Dictionary/
cp build/x86/libchewing/data/*.dat build/installer/Dictionary/
build/bin/chewing-cli init-database -t trie -n 內建詞庫 libchewing/data/tsi.src build/installer/Dictionary/tsi.dat
build/bin/chewing-cli init-database -t trie -n 內建字庫 libchewing/data/word.src build/installer/Dictionary/word.dat

mkdir -p build\installer\x86
cp target/i686-pc-windows-gnu/release/chewing_tip.dll build/installer/x86/
cp target/i686-pc-windows-gnu/release/ChewingPreferences.exe build/installer/
cp target/i686-pc-windows-gnu/release/tsfreg.exe build/installer/
cp build/bin/chewing-cli.exe build/installer/

mkdir build/installer/x64
cp target/x86_64-pc-windows-gnu/release/chewing_tip.dll build/installer/x64

pushd build/installer
/c/'Program Files (x86)'/'Microsoft Visual Studio'/2022/BuildTools//MSBuild/Current/Bin/amd64/MSBuild.exe -p:Configuration=Release -restore windows-chewing-tsf.wixproj
popd
cp build/installer/bin/Release/zh-TW/windows-chewing-tsf.msi dist/windows-chewing-tsf-unsigned.msi
