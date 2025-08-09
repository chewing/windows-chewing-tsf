# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### 🐛 Bug Fixes

- Add missing bopomofo mapping to the GinYieh layout
- Correct IBM layout mapping

### 🚜 Refactor

- (gfx) Use WARP renderer and cache the D3DDevice to speed up candidate window
  creation ([#420](https://github.com/chewing/windows-chewing-tsf/issues/420))
- (dict) Dictionary are built directly from the libchewing-data repository

## [25.8.1.0] - 2025-07-31

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
- Show file path in editor window
- Display localized font names in preferences
- Show Simplified Chinese mode icon
- Enable using Shift for easy symbol input
- Support sorting and filtering in dictionary editor ([#367](https://github.com/chewing/windows-chewing-tsf/issues/367))
- Support sorting and filtering in dictionary editor
- Only toggle lang mode when short press Shift
- Allow enable/disable fullwidth toggle key
- Add menu item to toggle fullwidth mode
- Add apply button to preferences
- Support easy symbol input with Shift+Ctrl combo
- Support semi-transparent candidate window by setting color in RRGGBBAA format

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
- Use near black icon to avoid it becomes transparent
- Use GDI to enumerate localized font names correctly
- Open registry with read-only option
- Fallback to default config if load from registry failed
- Get DWrite font names from GDI font names
- Refresh systray icon if config changes
- Set DEFAULT_CHARSET when querying GDI fonts
- Normalize key code to lowercase by default ([#362](https://github.com/chewing/windows-chewing-tsf/issues/362))
- Add missing 大千 26 鍵 in preference
- Always load inputmode lang bar button ([#370](https://github.com/chewing/windows-chewing-tsf/issues/370))
- Correctly handle shift space toggle fullshape mode
- Correctly handle all Simplified Chinese conversion condition
- Ignore keys with ctrl or alt modifiers
- Always show user phrase notification
- Preserve precision when converting window coordinates
- Scale font size with DPI
- Retrieve DPI after window is moved to right position
- Grant app container access to registry
- Raise font_size limit to 256
- Menubar is no longer semi-transparent
- Fix open popup menu from the start menu
- Make config launcher compatiable with sandboxed applications
- Use correct method to calculate scaling factor for notification window
- Sort localized font names first correctly
- Compatible with pre-2023 Delphi created application
- Correctly parse 000000FF to RRGGBBAA

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
- Bound candidate window to screen boundry
- Fix clippy warnings
- Update label for frequency in editor
- Set default-font-family for all slint window

### 📚 Documentation

- Remove unused build scripts and references
- Add comment to explain why never inline color_uf

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
- Adjust how we draft nightly release
- Fix cross-build cache
- Adjust nightly release name
- Sign nightly releases with release signing key
- Remove env_logger to reduce binary size
- Switch back to released chewing-rs
- Do not build libchewing-data with released chewing-cli
- Do not build libchewing-data with released chewing-cli

### Icon

- Redesign systray icons

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
