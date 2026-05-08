use ratatui::style::Color;

pub fn heat_color(ratio: f64) -> Color {
    match ratio {
        value if value >= 0.85 => Color::Red,
        value if value >= 0.65 => Color::LightRed,
        value if value >= 0.40 => Color::Yellow,
        value if value >= 0.15 => Color::Cyan,
        _ => Color::Blue,
    }
}
