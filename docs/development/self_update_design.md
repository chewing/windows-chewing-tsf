# Windows Chewing TSF Self-update Checker

## Features

Initially the self-update Checker supports the following features:

1. Check updates by channel (stable vs development)
2. Periodical checks; default every 24 hours
3. Notify that updates are available via registry
4. Opt-out via registry

Notably auto-download and auto-update are not supported. Installing the update
requires UAC prompt so auto-update is hard to support. Redirecting users to the
release page has the benefit of displaying detailed release information and we
can spend less time on implementing the update UI.

## Releases Schema

We use the [release information][1] schema defined by the freedesktop.org
[AppStream][2] 1.0 spec.

[1]: https://www.freedesktop.org/software/appstream/docs/sect-Metadata-Releases.html
[2]: https://www.freedesktop.org/software/appstream/docs/

Currently only the `<release/>` tag, `version` and `type` attributes, and the
`<url/>` tag are used.

Example:

```xml
<releases>
  <!-- https://www.freedesktop.org/software/appstream/docs/sect-Metadata-Releases.html -->
  <release version="25.8.1.468" date="2025-08-17" type="development" urgency="low">
    <url>https://github.com/chewing/windows-chewing-tsf/releases/tag/nightly-25.8.1.468</url>
    <artifacts>
      <artifact type="binary" platform="x86_64-windows-msvc">
        <location>https://github.com/chewing/windows-chewing-tsf/releases/download/nightly-25.8.1.468/windows-chewing-tsf.msi</location>
        <checksum type="sha256">3d31cb52739346fba754af1697e487284b9255e7a620632583c43093e9b95e6a</checksum>
      </artifact>
    </artifacts>
  </release>
  <release version="25.8.1.0" date="2025-07-31" type="stable" urgency="medium">
    <url>https://github.com/chewing/windows-chewing-tsf/releases/tag/v25.8.1.0</url>
    <artifacts>
      <artifact type="binary" platform="x86_64-windows-msvc">
        <location>https://github.com/chewing/windows-chewing-tsf/releases/download/v25.8.1.0/windows-chewing-tsf-25.8.1.0-installer.msi</location>
        <checksum type="sha256">710f01d8957ab226f6b8ced47f921ce40d6fe12619ce34b576114012c150e6ee</checksum>
      </artifact>
    </artifacts>
  </release>
</releases>
```

## Comparing Versions

The latest release information metadata should be fetched periodically from URL
<https://chewing.im/releases/im.chewing.windows_chewing_tsf.releases.xml>.

Then the version is compared to the version number of the `chewing_tip.dll`
file. Release channel is "stable" by default, can be set to "development" in
preferences.

## Notify Updates

Whenever a new update is detected, `chewing_tip_host` shall store the update
URL to the registry key `HKCU\Software\ChewingTextService`, attribute name
UpdateAvailable. Otherwise, this attribute should be removed.
