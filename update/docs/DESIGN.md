# Windows Chewing TSF Self-update Service

## Features

Initially the self-update service supports the following features:

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

## Comparing Versions

The latest release information metadata should be fetched periodically from URL
<https://chewing.im/releases/im.chewing.windows_chewing_tsf.releases.xml>.

Then the version is compared to the version number of the `chewing_tip.dll`
file. Release channel is "stable" by default, can be set to "development" in
preferences.

## Notify Updates

Whenever a new update is detected, `chewing-update-svc` shall store the update
URL to the registry key `HKCU\Software\ChewingTextService`, attribute name
UpdateAvailable. Otherwise, this attribute should be removed.
