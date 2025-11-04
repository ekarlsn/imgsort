use iced::Color;

pub struct ButtonStyle {
    pub basic: Color,
    pub hover: Color,
    pub press: Color,
}

pub const RED_BUTTON_STYLE: ButtonStyle = ButtonStyle {
    basic: Color::from_rgb(1.0, 0.0, 0.0),
    hover: Color::from_rgb(1.0, 0.4, 0.4),
    press: Color::from_rgb(5.0, 0.0, 0.0),
};

pub const GREEN_BUTTON_STYLE: ButtonStyle = ButtonStyle {
    basic: Color::from_rgb(0.0, 0.6, 0.0),
    hover: Color::from_rgb(0.2, 0.6, 0.2),
    press: Color::from_rgb(0.0, 0.3, 0.0),
};

pub const YELLOW_BUTTON_STYLE: ButtonStyle = ButtonStyle {
    basic: Color::from_rgb(0.8, 0.8, 0.0),
    hover: Color::from_rgb(0.8, 0.8, 0.6),
    press: Color::from_rgb(0.3, 0.3, 0.0),
};

pub const BLUE_BUTTON_STYLE: ButtonStyle = ButtonStyle {
    basic: Color::from_rgb(0.0, 0.0, 1.0),
    hover: Color::from_rgb(0.4, 0.4, 1.0),
    press: Color::from_rgb(0.0, 0.0, 0.5),
};

pub const PURPLE_BUTTON_STYLE: ButtonStyle = ButtonStyle {
    basic: Color::from_rgb(1.0, 0.0, 1.0),
    hover: Color::from_rgb(1.0, 0.4, 1.0),
    press: Color::from_rgb(5.0, 0.0, 5.0),
};

pub const ORANGE_BUTTON_STYLE: ButtonStyle = ButtonStyle {
    basic: Color::from_rgb(1.0, 0.5, 0.0),
    hover: Color::from_rgb(1.0, 0.7, 0.4),
    press: Color::from_rgb(5.0, 2.5, 0.0),
};

pub const GRAY_BUTTON_STYLE: ButtonStyle = ButtonStyle {
    basic: Color::from_rgb(0.5, 0.5, 0.5),
    hover: Color::from_rgb(0.7, 0.7, 0.7),
    press: Color::from_rgb(2.5, 2.5, 2.5),
};

pub const CYAN_BUTTON_STYLE: ButtonStyle = ButtonStyle {
    basic: Color::from_rgb(0.0, 1.0, 1.0),
    hover: Color::from_rgb(0.4, 1.0, 1.0),
    press: Color::from_rgb(0.0, 0.5, 0.5),
};
