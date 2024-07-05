To build an installer:

1. Put libchewing/data/*.dat dictionary files in Dictionary subdir.

2. Put 64-bit ChewingTextService.dll in x64 subdir.

3. Put 32-bit ChewingTextService.dll in x86 subdir.

4. Put `32-bit` ChewingPreferences.exe in this dir.

5. Put `32-bit` chewing-cli.exe in this dir.

6. Compile windows-chewing-tsf.wixproj with msbuild.

All steps can be automated by scripts\build_installer.bat