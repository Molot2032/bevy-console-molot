use std::collections::HashSet;

use ansi_parser::AnsiParser;

#[cfg(feature = "ui")]
use bevy_egui::egui::Color32;

/// Remove dependence on egui's Color32
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Colour {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Colour {
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[cfg(feature = "ui")]
impl Into<Color32> for Colour {
    fn into(self) -> Color32 {
        Color32::from_rgb(self.r, self.g, self.b)
    }
}

pub(crate) fn parse_ansi_styled_str(
    ansi_string: &str,
) -> Vec<(usize, HashSet<TextFormattingOverride>)> {
    let mut result: Vec<(usize, HashSet<TextFormattingOverride>)> = Vec::new();
    let mut offset = 0;
    for element in ansi_string.ansi_parse() {
        match element {
            ansi_parser::Output::TextBlock(t) => {
                offset += t.len();
            }
            ansi_parser::Output::Escape(escape) => {
                if let ansi_parser::AnsiSequence::SetGraphicsMode(mode) = escape {
                    let modes = parse_graphics_mode(mode.as_slice());
                    if let Some((last_offset, last)) = result.last_mut() {
                        if *last_offset == offset {
                            last.extend(modes);
                            continue;
                        }
                    }

                    result.push((offset, modes));
                };
            }
        }
    }
    result
}

fn parse_graphics_mode(modes: &[u8]) -> HashSet<TextFormattingOverride> {
    let mut results = HashSet::new();
    for mode in modes.iter() {
        let result = match *mode {
            0 => TextFormattingOverride::Reset,
            1 => TextFormattingOverride::Bold,
            2 => TextFormattingOverride::Dim,
            3 => TextFormattingOverride::Italic,
            4 => TextFormattingOverride::Underline,
            9 => TextFormattingOverride::Strikethrough,
            30..=37 => TextFormattingOverride::Foreground(ansi_color_code_to_color32(mode - 30)),
            40..=47 => TextFormattingOverride::Background(ansi_color_code_to_color32(mode - 40)),
            _ => TextFormattingOverride::Reset,
        };
        results.insert(result);
    }
    results
}

fn ansi_color_code_to_color32(color_code: u8) -> Colour {
    match color_code {
        1 => Colour::from_rgb(222, 56, 43),    // red
        2 => Colour::from_rgb(57, 181, 74),    // green
        3 => Colour::from_rgb(255, 199, 6),    // yellow
        4 => Colour::from_rgb(0, 111, 184),    // blue
        5 => Colour::from_rgb(118, 38, 113),   // magenta
        6 => Colour::from_rgb(44, 181, 233),   // cyan
        7 => Colour::from_rgb(204, 204, 204),  // white
        8 => Colour::from_rgb(128, 128, 128),  // bright black
        9 => Colour::from_rgb(255, 0, 0),      // bright red
        10 => Colour::from_rgb(0, 255, 0),     // bright green
        11 => Colour::from_rgb(255, 255, 0),   // bright yellow
        12 => Colour::from_rgb(0, 0, 255),     // bright blue
        13 => Colour::from_rgb(255, 0, 255),   // bright magenta
        14 => Colour::from_rgb(0, 255, 255),   // bright cyan
        15 => Colour::from_rgb(255, 255, 255), // bright white
        _ => Colour::from_rgb(1, 1, 1),        // black
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TextFormattingOverride {
    Reset,
    Bold,
    Dim,
    Italic,
    Underline,
    Strikethrough,
    Foreground(Colour),
    Background(Colour),
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_bold_text() {
        let ansi_string = color_print::cstr!(r#"<bold>12345</bold>"#);
        let result = parse_ansi_styled_str(ansi_string);
        assert_eq!(
            result,
            vec![
                (0, HashSet::from([TextFormattingOverride::Bold])),
                (5, HashSet::from([TextFormattingOverride::Reset]))
            ]
        );
    }

    #[test]
    fn test_underlined_text() {
        let ansi_string = color_print::cstr!(r#"<underline>12345</underline>"#);
        let result = parse_ansi_styled_str(ansi_string);
        assert_eq!(
            result,
            vec![
                (0, HashSet::from([TextFormattingOverride::Underline])),
                (5, HashSet::from([TextFormattingOverride::Reset]))
            ]
        );
    }

    #[test]
    fn test_italics_text() {
        let ansi_string = color_print::cstr!(r#"<italic>12345</italic>"#);
        let result = parse_ansi_styled_str(ansi_string);
        assert_eq!(
            result,
            vec![
                (0, HashSet::from([TextFormattingOverride::Italic])),
                (5, HashSet::from([TextFormattingOverride::Reset]))
            ]
        );
    }

    #[test]
    fn test_dim_text() {
        let ansi_string = color_print::cstr!(r#"<dim>12345</dim>"#);
        let result = parse_ansi_styled_str(ansi_string);
        assert_eq!(
            result,
            vec![
                (0, HashSet::from([TextFormattingOverride::Dim])),
                (5, HashSet::from([TextFormattingOverride::Reset]))
            ]
        );
    }

    #[test]
    fn test_strikethrough_text() {
        let ansi_string = color_print::cstr!(r#"<strike>12345</strike>"#);
        let result = parse_ansi_styled_str(ansi_string);
        assert_eq!(
            result,
            vec![
                (0, HashSet::from([TextFormattingOverride::Strikethrough])),
                (5, HashSet::from([TextFormattingOverride::Reset]))
            ]
        );
    }

    #[test]
    fn test_foreground_color() {
        let ansi_string = color_print::cstr!(r#"<red>12345</red>"#);
        let result = parse_ansi_styled_str(ansi_string);
        assert_eq!(
            result,
            vec![
                (
                    0,
                    HashSet::from([TextFormattingOverride::Foreground(Colour::from_rgb(
                        222, 56, 43
                    ))])
                ),
                (5, HashSet::from([TextFormattingOverride::Reset]))
            ]
        );
    }

    #[test]
    fn test_background_color() {
        let ansi_string = color_print::cstr!(r#"<bg:red>12345</bg:red>"#);
        let result = parse_ansi_styled_str(ansi_string);
        assert_eq!(
            result,
            vec![
                (
                    0,
                    HashSet::from([TextFormattingOverride::Background(Colour::from_rgb(
                        222, 56, 43
                    ))])
                ),
                (5, HashSet::from([TextFormattingOverride::Reset]))
            ]
        );
    }

    #[test]
    fn test_multiple_styles() {
        let ansi_string = color_print::cstr!(r#"<bold><red>12345</red></bold>"#);
        let result = parse_ansi_styled_str(ansi_string);
        assert_eq!(
            result,
            vec![
                (
                    0,
                    HashSet::from([
                        TextFormattingOverride::Foreground(Colour::from_rgb(222, 56, 43)),
                        TextFormattingOverride::Bold,
                    ])
                ),
                (5, HashSet::from([TextFormattingOverride::Reset]))
            ]
        );
    }

    #[test]
    fn non_overlapping_styles() {
        let ansi_string = color_print::cstr!(r#"<bold>12345</bold><red>12345</red>"#);
        let result = parse_ansi_styled_str(ansi_string);
        assert_eq!(
            result,
            vec![
                (0, HashSet::from([TextFormattingOverride::Bold])),
                (
                    5,
                    HashSet::from([
                        TextFormattingOverride::Reset,
                        TextFormattingOverride::Foreground(Colour::from_rgb(222, 56, 43))
                    ])
                ),
                (10, HashSet::from([TextFormattingOverride::Reset]))
            ]
        );
    }

    #[test]
    fn overlapping_non_symmetric_styles() {
        let ansi_string = color_print::cstr!(r#"<bold>12345<red>12345</red></bold>"#);
        let result = parse_ansi_styled_str(ansi_string);
        assert_eq!(
            result,
            vec![
                (0, HashSet::from([TextFormattingOverride::Bold])),
                (
                    5,
                    HashSet::from([TextFormattingOverride::Foreground(Colour::from_rgb(
                        222, 56, 43
                    ))])
                ),
                (10, HashSet::from([TextFormattingOverride::Reset]))
            ]
        );
    }
}
