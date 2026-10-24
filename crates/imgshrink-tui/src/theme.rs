use ratatui::style::Color;

#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub surface: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub highlight: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            bg: Color::Reset,
            surface: Color::Rgb(22, 27, 34),
            border: Color::Rgb(48, 58, 72),
            text: Color::Rgb(220, 227, 235),
            muted: Color::Rgb(125, 138, 153),
            accent: Color::Rgb(88, 214, 190),
            success: Color::Rgb(110, 210, 130),
            warning: Color::Rgb(235, 190, 90),
            danger: Color::Rgb(235, 110, 110),
            highlight: Color::Rgb(38, 50, 62),
        }
    }

    pub fn mono() -> Self {
        Self {
            bg: Color::Reset,
            surface: Color::Reset,
            border: Color::Gray,
            text: Color::White,
            muted: Color::DarkGray,
            accent: Color::White,
            success: Color::White,
            warning: Color::White,
            danger: Color::White,
            highlight: Color::DarkGray,
        }
    }
}
