// SPDX-License-Identifier: GPL-3.0-or-later

use windows_registry::CURRENT_USER;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum WindowsTheme {
    Light,
    Dark,
    Unknown,
}

pub(crate) struct ThemeDetector;

impl ThemeDetector {
    const PERSONALIZE_KEY: &'static str =
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize";

    /// Detect current Windows theme
    pub(crate) fn detect_theme() -> WindowsTheme {
        Self::get_system_theme()
            .unwrap_or_else(|| Self::get_app_theme().unwrap_or(WindowsTheme::Light))
    }

    /// Get system-wide theme setting (taskbar, start menu, etc.)
    pub(crate) fn get_system_theme() -> Option<WindowsTheme> {
        Self::read_theme_value("SystemUsesLightTheme")
    }

    /// Get application theme setting
    pub(crate) fn get_app_theme() -> Option<WindowsTheme> {
        Self::read_theme_value("AppsUseLightTheme")
    }

    fn read_theme_value(value_name: &str) -> Option<WindowsTheme> {
        CURRENT_USER
            .open(Self::PERSONALIZE_KEY)
            .and_then(|key| key.get_u32(value_name))
            .map(|value| match value {
                0 => WindowsTheme::Dark,
                1 => WindowsTheme::Light,
                _ => WindowsTheme::Unknown,
            })
            .ok()
    }

    /// Get detailed theme information
    pub(crate) fn get_theme_info() -> ThemeInfo {
        ThemeInfo {
            system_theme: Self::get_system_theme().unwrap_or(WindowsTheme::Unknown),
            app_theme: Self::get_app_theme().unwrap_or(WindowsTheme::Unknown),
            overall_theme: Self::detect_theme(),
        }
    }
}

#[allow(unused)]
#[derive(Debug)]
pub struct ThemeInfo {
    pub system_theme: WindowsTheme,
    pub app_theme: WindowsTheme,
    pub overall_theme: WindowsTheme,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_detection() {
        let theme = ThemeDetector::detect_theme();
        assert!(matches!(
            theme,
            WindowsTheme::Light | WindowsTheme::Dark | WindowsTheme::Unknown
        ));
    }

    #[test]
    fn test_theme_info() {
        let info = ThemeDetector::get_theme_info();
        // Should not panic and return valid enum variants
        println!("{:?}", info);
    }
}
