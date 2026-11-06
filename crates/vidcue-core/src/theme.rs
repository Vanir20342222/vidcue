use anyhow::{Context, Result, ensure};
use serde_json::Value;
pub type Rgb = (u8, u8, u8);
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Palette {
    pub name: String,
    pub source: String,
    pub background: Rgb,
    pub surface: Rgb,
    pub text: Rgb,
    pub muted: Rgb,
    pub accent: Rgb,
    pub selection: Rgb,
    pub error: Rgb,
    pub warning: Rgb,
    pub success: Rgb,
    pub light: bool,
}
/// Built-in themes, in picker order: automatic, the vidcue pair, the Nothing pair, then
/// popular editor palettes that also ship as Omarchy desktop themes (so a fixed choice can
/// match the target desktop), and finally the terminal's own colours.
pub const THEMES: &[&str] = &[
    "system",
    "vidcue-dark",
    "vidcue-light",
    "nothing-dark",
    "nothing-light",
    "tokyo-night",
    "catppuccin-mocha",
    "catppuccin-latte",
    "everforest",
    "gruvbox",
    "kanagawa",
    "nord",
    "matte-black",
    "flexoki-light",
    "terminal",
];
/// Themes removed from the picker, with the closest remaining palette. Saved settings that
/// name one keep working and are rewritten to the replacement on load.
pub const RETIRED_THEMES: &[(&str, &str)] = &[
    ("dracula", "catppuccin-mocha"),
    ("rose-pine", "catppuccin-mocha"),
    ("solarized-dark", "nord"),
    ("gruvbox-light", "flexoki-light"),
    // The app's own pair before the ytui → vidcue rename.
    ("ytui-dark", "vidcue-dark"),
    ("ytui-light", "vidcue-light"),
];
/// The current name for a saved theme value: retired built-ins map to their replacement;
/// anything else (current names, "system", "custom", palette file paths) is unchanged.
pub fn canonical(name: &str) -> &str {
    RETIRED_THEMES
        .iter()
        .find(|(old, _)| *old == name)
        .map_or(name, |(_, new)| new)
}
/// Serde hook for `Config::theme` so every load path migrates retired names.
pub fn deserialize_theme<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<String, D::Error> {
    let name = <String as serde::Deserialize>::deserialize(deserializer)?;
    Ok(canonical(&name).to_owned())
}
impl Palette {
    /// Nothing-styled palettes draw meters as dot-matrix rows and, on Android, use
    /// dot-matrix display type. Imported palettes never opt in.
    pub fn dot_matrix(&self) -> bool {
        self.source == "Built-in" && self.name.starts_with("nothing-")
    }
}
pub fn color(value: &str) -> Result<Rgb> {
    let value = value.trim().trim_start_matches('#');
    let value = if value.len() == 3 {
        value.chars().flat_map(|c| [c, c]).collect::<String>()
    } else {
        value.into()
    };
    ensure!(
        value.len() == 6 && value.is_ascii(),
        "Invalid palette color"
    );
    Ok((
        u8::from_str_radix(&value[0..2], 16)?,
        u8::from_str_radix(&value[2..4], 16)?,
        u8::from_str_radix(&value[4..6], 16)?,
    ))
}
pub fn builtin(name: &str) -> Result<Palette> {
    let name = match name {
        "default" | "midnight" | "dark" => "vidcue-dark",
        "light" => "vidcue-light",
        other => other,
    };
    let name = canonical(name);
    // Roles: background, surface, text, muted, accent, selection, error, warning, success.
    let (values, light) = match name {
        "vidcue-dark" | "terminal" => (
            [
                "101418", "1b232c", "e5edf4", "a8b9c9", "6ddbc1", "294053", "ff7f86", "f5c36c",
                "8edd99",
            ],
            false,
        ),
        "vidcue-light" => (
            [
                "f6f8fa", "e9eff3", "202c36", "52687b", "006f62", "d0e3eb", "b42336", "986300",
                "24713e",
            ],
            true,
        ),
        // Nothing: true-black OLED canvas, white type, grey secondary text and one red
        // signal colour used sparingly (as on Nothing OS widgets and the Glyph record dot).
        "nothing-dark" => (
            [
                "000000", "141414", "ffffff", "999999", "d71921", "262626", "ff4d4d", "e6b422",
                "4cbb6c",
            ],
            false,
        ),
        // Nothing light: paper-white surfaces on a pale grey page, black ink, the same red.
        "nothing-light" => (
            [
                "f2f2f2", "ffffff", "000000", "585a5a", "d71921", "dcdcdc", "a3121a", "8a6200",
                "1f7a3a",
            ],
            true,
        ),
        "tokyo-night" => (
            [
                "1a1b26", "24283b", "c0caf5", "9aa5ce", "7aa2f7", "3b4261", "f7768e", "e0af68",
                "9ece6a",
            ],
            false,
        ),
        "catppuccin-mocha" | "system" => (
            [
                "1e1e2e", "313244", "cdd6f4", "a6adc8", "cba6f7", "45475a", "f38ba8", "f9e2af",
                "a6e3a1",
            ],
            false,
        ),
        "catppuccin-latte" => (
            [
                "eff1f5", "e6e9ef", "4c4f69", "66697f", "8839ef", "ccd0da", "d20f39", "946000",
                "2d7a1f",
            ],
            true,
        ),
        "everforest" => (
            [
                "2d353b", "343f44", "d3c6aa", "9da9a0", "a7c080", "475258", "e67e80", "dbbc7f",
                "83c092",
            ],
            false,
        ),
        "gruvbox" => (
            [
                "282828", "3c3836", "ebdbb2", "bdae93", "fabd2f", "504945", "fb4934", "fe8019",
                "b8bb26",
            ],
            false,
        ),
        "kanagawa" => (
            [
                "1f1f28", "2a2a37", "dcd7ba", "a6a69c", "7e9cd8", "2d4f67", "e46876", "e6c384",
                "98bb6c",
            ],
            false,
        ),
        "nord" => (
            [
                "2e3440", "3b4252", "eceff4", "c2c9d6", "88c0d0", "4c566a", "bf616a", "ebcb8b",
                "a3be8c",
            ],
            false,
        ),
        // Omarchy's Matte Black: near-black neutrals with a single amber accent.
        "matte-black" => (
            [
                "121212", "1e1e1e", "bebebe", "8a8a8d", "e68e0d", "333333", "d35f5f", "ffc107",
                "8fae6b",
            ],
            false,
        ),
        "flexoki-light" => (
            [
                "fffcf0", "f2f0e5", "100f0f", "6f6e69", "205ea6", "dad8ce", "af3029", "ad8301",
                "66800b",
            ],
            true,
        ),
        _ => anyhow::bail!(
            "Unknown theme {name}; use :theme to list built-ins or a palette file path"
        ),
    };
    let v = values
        .iter()
        .map(|s| color(s))
        .collect::<Result<Vec<_>>>()?;
    Ok(Palette {
        name: name.into(),
        source: "Built-in".into(),
        background: v[0],
        surface: v[1],
        text: v[2],
        muted: v[3],
        accent: v[4],
        selection: v[5],
        error: v[6],
        warning: v[7],
        success: v[8],
        light,
    })
}

pub fn caelestia(raw: &str) -> Result<Palette> {
    let data: Value = serde_json::from_str(raw)?;
    let colours = data
        .get("colours")
        .context("Caelestia palette has no colours map")?;
    let light = data.get("mode").and_then(Value::as_str) == Some("light");
    let mut p = builtin(if light { "vidcue-light" } else { "vidcue-dark" })?;
    p.name = data
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Caelestia")
        .into();
    p.source = "Caelestia".into();
    let get = |names: &[&str]| -> Result<Rgb> {
        let value = names
            .iter()
            .find_map(|n| colours.get(n).and_then(Value::as_str))
            .context("Missing Caelestia semantic color")?;
        color(value)
    };
    p.background = get(&["surface", "background"])?;
    p.surface = get(&["surfaceContainer", "surfaceVariant", "surface"])?;
    p.text = get(&["onSurface", "onBackground"])?;
    p.muted = get(&["onSurfaceVariant", "onSurface"])?;
    p.accent = get(&["primary"])?;
    p.selection = get(&[
        "primaryContainer",
        "surfaceContainerHigh",
        "surfaceContainer",
    ])?;
    p.error = get(&["error"])?;
    Ok(p)
}
pub fn omarchy(raw: &str, legacy_light: bool) -> Result<Palette> {
    let data: toml::Value = toml::from_str(raw)?;
    let light = data.get("mode").and_then(toml::Value::as_str) == Some("light") || legacy_light;
    let mut p = builtin(if light { "vidcue-light" } else { "vidcue-dark" })?;
    p.name = "Omarchy".into();
    p.source = "Omarchy".into();
    let get = |keys: &[&str], fallback: Rgb| -> Result<Rgb> {
        keys.iter()
            .find_map(|k| data.get(*k).and_then(toml::Value::as_str))
            .map(color)
            .unwrap_or(Ok(fallback))
    };
    ensure!(
        data.get("background").is_some() && data.get("foreground").is_some(),
        "Omarchy palette lacks background/foreground"
    );
    p.background = get(&["background"], p.background)?;
    p.surface = get(&["lighter_background", "color0"], p.surface)?;
    p.text = get(&["foreground"], p.text)?;
    p.muted = get(&["dark_foreground", "muted", "color8"], p.muted)?;
    p.accent = get(&["accent", "blue", "color4"], p.accent)?;
    p.selection = get(&["selection", "color8"], p.selection)?;
    p.error = get(&["red", "color1"], p.error)?;
    p.warning = get(&["yellow", "color3"], p.warning)?;
    p.success = get(&["green", "color2"], p.success)?;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_unicode_palette_colors_are_errors_instead_of_slice_panics() {
        for invalid in ["éabcd", "💜ab", "#éa", "#ggg", "#12345"] {
            assert!(color(invalid).is_err());
        }
        assert_eq!(color("#abc").unwrap(), (170, 187, 204));
        assert_eq!(color("012345").unwrap(), (1, 35, 69));
    }
    #[test]
    fn retired_themes_resolve_to_a_listed_replacement() {
        for (old, new) in RETIRED_THEMES {
            assert!(!THEMES.contains(old), "{old} is still listed");
            assert!(THEMES.contains(new), "{new} is not listed");
            assert_eq!(builtin(old).unwrap(), builtin(new).unwrap());
            assert_eq!(canonical(old), *new);
        }
        for name in ["system", "custom", "/home/me/palette.toml", "nothing-dark"] {
            assert_eq!(canonical(name), name);
        }
    }
    #[test]
    fn saved_settings_with_a_retired_theme_load_as_its_replacement() {
        let toml: crate::Config = toml::from_str("theme = 'dracula'").unwrap();
        assert_eq!(toml.theme, "catppuccin-mocha");
        let json: crate::Config = serde_json::from_str(r#"{"theme":"gruvbox-light"}"#).unwrap();
        assert_eq!(json.theme, "flexoki-light");
        let kept: crate::Config = toml::from_str("theme = 'nord'").unwrap();
        assert_eq!(kept.theme, "nord");
        let missing: crate::Config = toml::from_str("").unwrap();
        assert_eq!(missing.theme, "system");
    }
    #[test]
    fn only_built_in_nothing_palettes_use_dot_matrix_styling() {
        assert!(builtin("nothing-dark").unwrap().dot_matrix());
        assert!(builtin("nothing-light").unwrap().dot_matrix());
        assert!(!builtin("vidcue-dark").unwrap().dot_matrix());
        let mut imported = builtin("nothing-dark").unwrap();
        imported.source = "Custom palette".into();
        assert!(!imported.dot_matrix());
        let nothing = builtin("nothing-dark").unwrap();
        assert_eq!(nothing.background, (0, 0, 0));
        assert_eq!(nothing.accent, (0xd7, 0x19, 0x21));
    }
}
