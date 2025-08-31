# windows-chewing-tsf

Implement chewing in Windows via Text Services Framework:

* chewing_tip contains an implementation of Windows text service for libchewing.
* tsfreg contains TSF registration helper used in the installer.
* preferences contains the user preference and phrase editor GUI.

All parts are licensed under GPL-3.0-or-later license.

# Development

## Tool Requirements

**Build natively on Windows using MSVC**

* [Build Tools for Visual Studio 2022](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)
* [Rust](https://rustup.rs/)

**Cross compile on Windows using MinGW-W64**

* [MSYS2](https://www.msys2.org/)
* [Rust](https://rustup.rs/)

**Cross compile on Fedora using MinGW-W64**

* [Rust](https://rustup.rs/)

## How to Build

* Get source from github
    ```bash
    git clone --recursive https://github.com/chewing/windows-chewing-tsf.git
    cd windows-chewing-tsf
    ```
* Use this xtask command to build the installer
    ```
    cargo xtask build-installer --target msvc --release
    cargo xtask package-installer
    ```

## TSF References

* [Text Services Framework](http://msdn.microsoft.com/en-us/library/windows/desktop/ms629032%28v=vs.85%29.aspx)
* [Guidelines and checklist for IME development (Windows Store apps)](http://msdn.microsoft.com/en-us/library/windows/apps/hh967425.aspx)
* [Input Method Editors (Windows Store apps)](http://msdn.microsoft.com/en-us/library/windows/apps/hh967426.aspx)
* [Third-party input method editors](http://msdn.microsoft.com/en-us/library/windows/desktop/hh848069%28v=vs.85%29.aspx)
* [Strategies for App Communication between Windows 8 UI and Windows 8 Desktop](http://software.intel.com/en-us/articles/strategies-for-app-communication-between-windows-8-ui-and-windows-8-desktop)
* [TSF Aware, Dictation, Windows Speech Recognition, and Text Services Framework. (blog)](http://blogs.msdn.com/b/tsfaware/?Redirected=true)
* [Win32 and COM for Windows Store apps](http://msdn.microsoft.com/en-us/library/windows/apps/br205757.aspx)
* [Input Method Editor (IME) sample supporting Windows 8](http://code.msdn.microsoft.com/windowsdesktop/Input-Method-Editor-IME-b1610980)

## Windows ACL (Access Control List) references

* [The Windows Access Control Model Part 1](http://www.codeproject.com/Articles/10042/The-Windows-Access-Control-Model-Part-1#SID)
* [The Windows Access Control Model: Part 2](http://www.codeproject.com/Articles/10200/The-Windows-Access-Control-Model-Part-2#SidFun)
* [Windows 8 App Container Security Notes - Part 1](http://recxltd.blogspot.tw/2012/03/windows-8-app-container-security-notes.html)
* [How AccessCheck Works](http://msdn.microsoft.com/en-us/library/windows/apps/aa446683.aspx)
* [GetAppContainerNamedObjectPath function (enable accessing object outside app containers using ACL)](http://msdn.microsoft.com/en-us/library/windows/desktop/hh448493)
* [Creating a DACL](http://msdn.microsoft.com/en-us/library/windows/apps/ms717798.aspx)

# Code Signing Policy

This [project](https://signpath.org/projects/chewing-im/) is sponsored by SignPath. We use free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

**People and roles:**

* Committers and reviewers: [Chewing core team](https://github.com/orgs/chewing/teams/core), [Windows Chewing maintainers](https://github.com/orgs/chewing/teams/windows)
* Approvers: [Chewing core team](https://github.com/orgs/chewing/teams/core)

# Privacy Policy

This program will not transfer any information to other networked systems unless
specifically requested by the user or the person installing or operating it.

# Bug Report
Please report any issue to [here](https://github.com/chewing/windows-chewing-tsf/issues).
