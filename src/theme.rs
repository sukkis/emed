/// provide a way to change themes
/// user can change theme by changing the theme name in settings.toml
///
use crossterm::style::Color;

/// Named colours used in theme definitions.
/// Keeps theme descriptions human-readable instead of raw crossterm values.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum ThemeColor {
    Black,
    White,
    Magenta,
    DarkGreen,
    DarkCyan,
    DarkGrey,
    Grey,
    Cyan,
}

impl ThemeColor {
    /// Convert to the crossterm Color used for terminal rendering.
    pub fn to_crossterm(self) -> Color {
        match self {
            ThemeColor::Black => Color::Black,
            ThemeColor::White => Color::White,
            ThemeColor::Magenta => Color::Magenta,
            ThemeColor::DarkGreen => Color::DarkGreen,
            ThemeColor::DarkCyan => Color::DarkCyan,
            ThemeColor::DarkGrey => Color::DarkGrey,
            ThemeColor::Grey => Color::Grey,
            ThemeColor::Cyan => Color::Cyan,
        }
    }
}

#[allow(dead_code)]
pub struct Theme {
    name: String,
    pub fg: ThemeColor,
    pub bg: ThemeColor,
    pub status_fg: ThemeColor,
    pub status_bg: ThemeColor,
    pub tilde_fg: ThemeColor,
}

impl Theme {
    /// Look up a built-in theme by name. Falls back to "pink" if unknown.
    pub fn from_name(name: &str) -> Self {
        match name {
            "ocean" => Self::ocean(),
            _ => Self::pink(),
        }
    }

    /// The default theme — magenta on black (matches the current hardcoded colours).
    fn pink() -> Self {
        Self {
            name: "pink".into(),
            fg: ThemeColor::Magenta,
            bg: ThemeColor::Black,
            status_fg: ThemeColor::Black,
            status_bg: ThemeColor::Magenta,
            tilde_fg: ThemeColor::Magenta,
        }
    }

    /// A calmer alternative — green/cyan on dark background.
    fn ocean() -> Self {
        Self {
            name: "ocean".into(),
            fg: ThemeColor::Cyan,
            bg: ThemeColor::Black,
            status_fg: ThemeColor::Black,
            status_bg: ThemeColor::DarkCyan,
            tilde_fg: ThemeColor::DarkGreen,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::style::Color;

    #[test]
    fn pink_is_the_default_theme() {
        let theme = Theme::from_name("pink");
        assert_eq!(theme.name, "pink");
    }

    #[test]
    fn ocean_theme_is_recognised() {
        let theme = Theme::from_name("ocean");
        assert_eq!(theme.name, "ocean");
    }

    #[test]
    fn unknown_theme_falls_back_to_pink() {
        let theme = Theme::from_name("doesnotexist");
        assert_eq!(theme.name, "pink");
    }

    #[test]
    fn fg_and_bg_differ_in_all_builtin_themes() {
        // Catch invisible text: foreground must not equal background.
        for name in &["pink", "ocean"] {
            let theme = Theme::from_name(name);
            assert_ne!(
                theme.fg.to_crossterm(),
                theme.bg.to_crossterm(),
                "theme '{}': fg and bg must differ",
                name
            );
        }
    }

    #[test]
    fn status_bar_colours_differ_in_all_builtin_themes() {
        for name in &["pink", "ocean"] {
            let theme = Theme::from_name(name);
            assert_ne!(
                theme.status_fg.to_crossterm(),
                theme.status_bg.to_crossterm(),
                "theme '{}': status_fg and status_bg must differ",
                name
            );
        }
    }

    #[test]
    fn theme_color_converts_to_expected_crossterm_values() {
        assert_eq!(ThemeColor::Black.to_crossterm(), Color::Black);
        assert_eq!(ThemeColor::Magenta.to_crossterm(), Color::Magenta);
        assert_eq!(ThemeColor::Cyan.to_crossterm(), Color::Cyan);
        assert_eq!(ThemeColor::DarkGreen.to_crossterm(), Color::DarkGreen);
    }
}
