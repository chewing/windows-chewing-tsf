cmake -B x86 -A Win32 -DBUILD_TESTING=OFF
cmake --build x86 --config Release
cmake -B x64 -A x64 -DBUILD_TESTING=OFF
cmake --build x64 --config Release
mkdir dist
mkdir nsis
copy installer\* nsis\
copy COPYING.txt nsis\
mkdir nsis\Dictionary
copy x64\libchewing\data\*.dat nsis\Dictionary\
mkdir nsis\x86
copy x86\ChewingTextService\Release\*.dll nsis\x86\
copy x86\libchewing\Release\*.dll nsis\x86\
copy x86\ChewingPreferences\Release\*.exe nsis\
copy x86\libchewing\chewing-cli.exe nsis\
mkdir nsis\x64
copy x64\ChewingTextService\Release\*.dll nsis\x64\
copy x64\libchewing\Release\*.dll nsis\x64\
pushd nsis
makensis installer.nsi
popd
copy nsis\windows-chewing-tsf.exe dist\windows-chewing-tsf-unsigned.exe