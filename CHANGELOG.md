# Changelog

All notable changes to this project will be documented in this file.

## [unreleased]

### 🚀 Features

- (prefs) Add slint UI skeleton
- (prefs) Move dialog UI into the same window
- (prefs) Add bopomofo keyboard widget
- (prefs) Implement basic dict editor
- (prefs) Better error handling
- (prefs) Add about page for editor
- (prefs) Implement new config app
- (prefs) Add about page for config window
- (prefs) Use assets and version string from globals
- (prefs) Replace ChewingPreferences with chewing-preferences-rs
- Make tsfreg uninstall more robust against failures
- Add help url to menu
- Use zhconv crate for Simplified Chinese conversion
- Enable user-mode minidump in test build
- Enable ChewingTIP by default
- Log to files
- Use chewing::path module to get user data path
- Make autolearn configurable
- Make phrase-choice-rearward configurable
- Add Ctrl+F12 shortcut key for toggle Simplified Chinese conversion
- Make font configurable
- Make font color configurable
- Implement UILess mode
- Allow configuring background color
- Make notification configurable

### 🐛 Bug Fixes

- Allow NSIS uninstall custom action to fail
- Find configs using runtime env
- Check chewing_new error
- Only use key state for modifier keys
- Load correct mode icon after reactivation
- Always use 64-bit program files path
- Ensure sub-pixel sampling works well with fractional scale
- Always initialize windbg logger
- Detect user dir even with MIC
- Bring back keyboard openclose monitor
- Skip focus event during key down
- Set composition string and cursor in the same edit session
- Use GetDpiForMonitor to detect monitor DPI
- Ensure to resize empty candidate window correctly
- (editor) Reload dict when start editing
- Load config after activation
- Better handling of capslock state
- Allow invalid HWND for UILess console

### 🚜 Refactor

- Remove almost empty source files
- Move Utils.cpp to ChewingTextService
- Remove unused virtual methods
- Remove unused TSF interfaces
- Move body of empty virtual methods to header
- Xtask use jiff without default features to cut deps
- Compile chewing_tip.dll directly using cargo
- Simplify unused compartment code
- Delete unused ChewingTextService.def
- Delete unused chewing_tip/CMakeLists.txt
- Move lang bar button to rust
- Set default log level to info
- Remove unused methods
- Remove more unused code
- Avoid cyclical reference between buttons and text service
- Simplify lang bar buttons handling
- Remove unused or unnecessary compartment code
- Oxidize TextService
- Reduce COM interfaes and unsafe methods
- Handle context owner interrupted composition
- Simplify KeyEvent interface
- Fix menu memory leak
- Reduce unwraps to avoid panic
- Fix conversion to Simplified Chinese
- Fix menu reuse
- End composition when losing focus
- Use windows-registry crate to read registry
- (prefs) Use windows-registry crate to read registry
- Always apply configs after activation
- Better HiDPI handling
- Better error logging
- Clippy warnings
- Add menubar to preferences and reuse about dialog
- Move editor about tab to about window
- Rewrite candidate window drawing routine
- Adjust candidate window client area size
- Move UIElement handling into CandidateList
- Reimplement notification window
- Clarify notification config string
- Try to improve dark mode detection

### 📚 Documentation

- Remove unused build scripts and references

### 🎨 Styling

- (prefs) Remove default font setting

### ⚙️ Miscellaneous Tasks

- Use sqlite from vcpkg
- Rebuild if cpp files change
- Use rust-lld by default
- (deps) Bump actions/checkout in the github-actions group
- Install stable rust toolchain
- Attach debug script to nightly release
- Update nightly release wording
- Clean-up unused files
- Move rc files to a subdirectory
- Fix version.rc include path
- Update WixToolset to v6
- (deps) Bump crossbeam-channel from 0.5.14 to 0.5.15
- Cross-compile in linux container
- Mark workspace as safe directory
- Use libchewing from git
- Use xtask to drive build and package
- Fix nightly code signing
- Allow nightly workflow to update releases
- Run cargo fmt
- (deps) Bump signpath/github-action-submit-signing-request
- Build nightly with release profile
- Attach ChewingPreferences.exe to nightly build
- Download Installer Artifact in nightly workflow
- Extract chewing-editor as separate executable
- Add cache

## [24.10.1] - 2024-12-22

### 🚀 Features

- Show chi/eng mode toast after toggle capslock
- Add version number to dialog title
- Listen to registry change event

### 🐛 Bug Fixes

- Remove init code from DllMain and static link libchewing
- Setting CHEWING_USER_PATH
- Correctly provide display attribute
- End composition and hide windows on blur
- Attempt to fix incorrect light theme detection

### 🚜 Refactor

- Remove debug log
- Remove debug log from display attribute provider

### 🎨 Styling

- Update icon

### ⚙️ Miscellaneous Tasks

- Update nightly build description in nightly.yml
- Automatically generate version info
- Trigger version info generation in PR context
- Fix cargo xtask command argument
- Log generated version info
- Only generate build revision in nightly mode
- Simplify version number scheme
- Use github_run_number
- Use different version for PR and nightly
- Fix version mentioned in nightly title

## [24.10] - 2024-12-15

### 🚀 Features

- Change icon based on system theme

### 🚜 Refactor

- Convert .rc file to UTF-8 encoding

### 🎨 Styling

- Update application icon

### ⚙️ Miscellaneous Tasks

- Do not link with libcmtd
- Call vcdevcmd.bat before build

## [24.10-rc.1] - 2024-11-09

### 'fix

- Copy static data files to installer'

### 🚀 Features

- Build MSI installer with WiX
- Register COM Server and TSF from MSI
- Uninstall NSIS installation from MSI
- Register dll as icon (not working)
- (msi) Correctly quote icon path
- (msi) Use ITfInputProcessorProfileMgr to register our TS
- (msi) Allow upgrade and downgrades
- (msi) [**breaking**] Remove nsis installer
- (prefs) Enable new keyboard layouts
- Use chewing_ack
- Support config conv engine
- Use 9-patch bitmap to draw candidate window
- Show chi/eng mode toast after manual toggle

### 🐛 Bug Fixes

- Macro expansion
- Rednering on hidpi device
- Reset composition buffer after toggle chi/eng mode
- Shift key handling
- Only use GetSysColor color index

### 🚜 Refactor

- (msi) Use high compression level
- (libime) Decouple Window and Dialog
- (pref) Decouple with libIME
- Delete unused chewingwrapper
- Draw candidate window with Direct2D
- Draw message window with Direct2D
- Validate client region after drawing
- Remove special immersive mode CandidateWindow
- Use winrt::com_ptr and VersionHelper
- Reimplement MessageWindow in rust
- Hide unused CandidateWindow public methods
- Implement CandidateWindow in rust
- Draw message window background using bitmap
- Cleanup font handling
- Add missing asset
- Simplify candidate window size calculation
- Remove unused DLL registration code (moved to tsfreg)

### 📚 Documentation

- Refresh and add code signing policy info
- Introduce CHANGELOG.md

### ⚙️ Miscellaneous Tasks

- Update dependencies
- Add github actions ci.yaml
- Fix build with Ninja
- Add nightly build workflow
- Introduce dependabot version updates
- Specify LANGUAGES in build script
- Use CMake variable to set MSVC runtime library
- Use CMake variable for enabling LTCG
- Drop redundant macro define
- Drop compiler flag override files
- Reorder override flag
- Remove redundant compile defines
- Specify source code encoding as UTF-8
- Set LANGUAGES property properly
- Add msbuild to path
- Remove libIME as submodule
- Merge libIME back as subdirectory
- Bump libchewing to 0.9.0-rc.3
- Remove cmake minimum version for libIME
- (libime) Remove UNICODE defines
- (libime) Move files only used by preferences
- Bump libchewing to 0.9.0
- Update signpath action to 1.0
- Simplify cmake files
- Use c++17
- Merge rustlib and libime2
- Fix release workflow
- Bump version to 24.10.258.0
- Bump libchewing to v0.9.0
- Use vcpkg for dependencies
- Update dependencies
- Fix tsfreg artifact location
- Update gitignore
- Remove unused ImeEngine files
- Add git-cliff config file

### README

- Initialize all submodules after cloning

### Refactor

- Add PIME::LangBarButton and move language button handling out of PIME::Client.

### Nsis

- Revise license page instructions
- Compress data in one block
- Declare the installer is DPI-aware
- Remove unused strings
- Use default branding text
- Bundle `chewing-cli.exe`

<!-- generated by git-cliff -->
