cmake -B build\x86 -A Win32 -DBUILD_TESTING=OFF -DVCPKG_TARGET_TRIPLET=x86-windows-static
cmake --build build\x86 --config Debug
cmake -B build\x64 -A x64 -DBUILD_TESTING=OFF -DVCPKG_TARGET_TRIPLET=x64-windows-static
cmake --build build\x64 --config Debug
pushd tsfreg
cargo build --release
popd
mkdir dist
mkdir build\installer
mkdir build\installer\assets
copy assets\* build\installer\assets\
copy installer\* build\installer\
copy ChewingTextService\mainicon2.ico build\installer\chewing.ico
mkdir build\installer\Dictionary
copy libchewing\data\*.dat build\installer\Dictionary\
copy build\x64\libchewing\data\*.dat build\installer\Dictionary\
mkdir build\installer\x86
copy build\x86\ChewingTextService\Debug\*.dll build\installer\x86\
copy build\x86\libchewing\Debug\*.dll build\installer\x86\
copy build\x86\ChewingPreferences\Debug\*.exe build\installer\
copy build\x86\libchewing\chewing-cli.exe build\installer\
mkdir build\installer\x64
copy build\x64\ChewingTextService\Debug\*.dll build\installer\x64\
copy build\x64\libchewing\Debug\*.dll build\installer\x64\
copy target\release\tsfreg.exe build\installer\
pushd build\installer
msbuild -p:Configuration=Release -restore windows-chewing-tsf.wixproj
popd
copy build\installer\bin\Release\zh-TW\windows-chewing-tsf.msi dist\windows-chewing-tsf-unsigned.msi