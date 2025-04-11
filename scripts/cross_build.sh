#!/bin/sh

set -x
set -e

cargo install chewing-cli

export SQLITE3_STATIC=1
export SQLITE3_LIB_DIR=/usr/x86_64-w64-mingw32/sys-root/mingw/lib/
cargo build -p chewing_tip --release --target x86_64-pc-windows-gnu

export SQLITE3_LIB_DIR=/usr/i686-w64-mingw32/sys-root/mingw/lib/
cargo install chewing-cli --root build --target i686-pc-windows-gnu
cargo build -p chewing_tip --release --target i686-pc-windows-gnu
cargo build -p chewing-preferences --release --target i686-pc-windows-gnu
cargo build -p tsfreg --release --target i686-pc-windows-gnu

rm -rf dist build/installer
mkdir -p build/installer/assets
cp assets/* build/installer/assets/
cp installer/* build/installer/
cp chewing_tip/rc/im.chewing.Chewing.ico build/installer/chewing.ico

mkdir -p build/installer/Dictionary
cp libchewing/data/*.dat build/installer/Dictionary/
chewing-cli init-database \
    -c "Copyright (c) 2025 libchewing Core Team" \
    -l "LGPL-2.1-or-later" \
    -r "2025.04.11" \
    -t trie -n 內建詞庫 libchewing/data/tsi.src build/installer/Dictionary/tsi.dat
chewing-cli init-database \
    -c "Copyright (c) 2025 libchewing Core Team" \
    -l "LGPL-2.1-or-later" \
    -r "2025.04.11" \
    -t trie -n 內建字庫 libchewing/data/word.src build/installer/Dictionary/word.dat

mkdir -p build/installer/x86
cp target/i686-pc-windows-gnu/release/chewing_tip.dll build/installer/x86/
cp target/i686-pc-windows-gnu/release/ChewingPreferences.exe build/installer/
cp target/i686-pc-windows-gnu/release/tsfreg.exe build/installer/
cp build/bin/chewing-cli.exe build/installer/

mkdir build/installer/x64
cp target/x86_64-pc-windows-gnu/release/chewing_tip.dll build/installer/x64
