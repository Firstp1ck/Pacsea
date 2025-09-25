use ratatui::style::Color;

pub struct Theme {
    pub base: Color,
    pub mantle: Color,
    pub crust: Color,
    pub surface1: Color,
    pub surface2: Color,
    pub overlay1: Color,
    pub overlay2: Color,
    pub text: Color,
    pub subtext0: Color,
    pub subtext1: Color,
    pub sapphire: Color,
    pub mauve: Color,
    pub green: Color,
    pub yellow: Color,
    pub red: Color,
    pub lavender: Color,
}

fn hex(rgb: (u8, u8, u8)) -> Color {
    Color::Rgb(rgb.0, rgb.1, rgb.2)
}

pub fn theme() -> Theme {
    Theme {
        base: hex((0x1e, 0x1e, 0x2e)),
        mantle: hex((0x18, 0x18, 0x25)),
        crust: hex((0x11, 0x11, 0x1b)),
        surface1: hex((0x45, 0x47, 0x5a)),
        surface2: hex((0x58, 0x5b, 0x70)),
        overlay1: hex((0x7f, 0x84, 0x9c)),
        overlay2: hex((0x93, 0x99, 0xb2)),
        text: hex((0xcd, 0xd6, 0xf4)),
        subtext0: hex((0xa6, 0xad, 0xc8)),
        subtext1: hex((0xba, 0xc2, 0xde)),
        sapphire: hex((0x74, 0xc7, 0xec)),
        mauve: hex((0xcb, 0xa6, 0xf7)),
        green: hex((0xa6, 0xe3, 0xa1)),
        yellow: hex((0xf9, 0xe2, 0xaf)),
        red: hex((0xf3, 0x8b, 0xa8)),
        lavender: hex((0xb4, 0xbe, 0xfe)),
    }
}
