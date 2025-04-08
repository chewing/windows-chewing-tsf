mkdir dist
pushd build\installer
msbuild -p:Configuration=Release -restore windows-chewing-tsf.wixproj
popd
copy build\installer\bin\Release\zh-TW\windows-chewing-tsf.msi dist\windows-chewing-tsf-unsigned.msi
