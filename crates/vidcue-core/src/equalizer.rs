//! Ten-band audio equalizer: built-in presets, user-named custom presets and the
//! mpv/FFmpeg filter that applies them. "off" adds no filter at all.
use anyhow::{Result, ensure};
use std::collections::BTreeMap;

/// Centre frequencies (Hz) of the ten octave bands.
pub const BANDS: [u32; 10] = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
/// Largest boost or cut per band, in dB.
pub const MAX_GAIN: f64 = 12.0;
/// Most user-named presets kept in settings.
pub const MAX_CUSTOM: usize = 16;
/// The preset name that disables the equalizer.
pub const OFF: &str = "off";
/// The name setup uses for a new custom curve.
pub const CUSTOM: &str = "custom";

/// Built-in presets in display order. Gains are dB per band in [`BANDS`] order.
pub const PRESETS: &[(&str, [f64; 10])] = &[
    ("flat", [0.0; 10]),
    (
        "bass-boost",
        [6.0, 5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ),
    (
        "bass-reduce",
        [-6.0, -5.0, -4.0, -2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ),
    (
        "treble-boost",
        [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 5.0, 6.0],
    ),
    (
        "vocal",
        [-2.0, -2.0, -1.0, 1.0, 3.0, 4.0, 3.0, 1.0, 0.0, -1.0],
    ),
    (
        "rock",
        [5.0, 4.0, 2.0, -1.0, -2.0, -1.0, 1.0, 3.0, 4.0, 4.0],
    ),
    ("pop", [-1.0, 0.0, 2.0, 3.0, 4.0, 3.0, 1.0, 0.0, -1.0, -1.0]),
    ("jazz", [3.0, 2.0, 1.0, 2.0, -1.0, -1.0, 0.0, 1.0, 2.0, 3.0]),
    (
        "classical",
        [4.0, 3.0, 2.0, 1.0, -1.0, -1.0, 0.0, 2.0, 3.0, 4.0],
    ),
    (
        "electronic",
        [5.0, 4.0, 1.0, 0.0, -2.0, 1.0, 0.0, 1.0, 4.0, 5.0],
    ),
    (
        "acoustic",
        [3.0, 3.0, 2.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 1.0],
    ),
    (
        "loudness",
        [6.0, 4.0, 0.0, 0.0, -2.0, 0.0, -1.0, -3.0, 4.0, 2.0],
    ),
    (
        "spoken-word",
        [-4.0, -3.0, -1.0, 1.0, 3.0, 4.0, 4.0, 2.0, 0.0, -3.0],
    ),
];

/// Every preset name the user can choose: off, the built-ins, then custom names.
pub fn names(custom: &BTreeMap<String, Vec<f64>>) -> Vec<String> {
    std::iter::once(OFF)
        .chain(PRESETS.iter().map(|(name, _)| *name))
        .map(str::to_owned)
        .chain(custom.keys().cloned())
        .collect()
}

pub fn is_builtin(name: &str) -> bool {
    name == OFF || PRESETS.iter().any(|(n, _)| *n == name)
}

/// Gains for `name`, or `None` for "off" and unknown names.
pub fn gains(name: &str, custom: &BTreeMap<String, Vec<f64>>) -> Option<[f64; 10]> {
    if let Some((_, gains)) = PRESETS.iter().find(|(n, _)| *n == name) {
        return Some(*gains);
    }
    let values = custom.get(name)?;
    let mut gains = [0.0; 10];
    for (slot, value) in gains.iter_mut().zip(values) {
        *slot = *value;
    }
    Some(gains)
}

/// The mpv `--af` value for a curve, labelled `@vidcue-eq` so it can be replaced
/// live. A curve with every band at 0 dB needs no filter.
pub fn filter(gains: &[f64; 10]) -> Option<String> {
    if gains.iter().all(|g| *g == 0.0) {
        return None;
    }
    let chain = BANDS
        .iter()
        .zip(gains)
        .filter(|(_, g)| **g != 0.0)
        .map(|(f, g)| format!("equalizer=f={f}:t=o:w=1:g={g}"))
        .collect::<Vec<_>>()
        .join(",");
    Some(format!("@vidcue-eq:lavfi=[{chain}]"))
}

/// Validates a preset name and the custom table.
pub fn validate(name: &str, custom: &BTreeMap<String, Vec<f64>>) -> Result<()> {
    ensure!(
        custom.len() <= MAX_CUSTOM,
        "At most {MAX_CUSTOM} custom equalizer presets are allowed"
    );
    for (key, values) in custom {
        ensure!(
            valid_name(key) && !is_builtin(key),
            "Custom equalizer names use letters, digits and '-', up to 24 characters, and cannot reuse a built-in name ({key})"
        );
        ensure!(
            values.len() == BANDS.len(),
            "Custom equalizer {key} needs {} band values",
            BANDS.len()
        );
        ensure!(
            values.iter().all(|g| g.is_finite() && g.abs() <= MAX_GAIN),
            "Equalizer gains must be between -{MAX_GAIN} and {MAX_GAIN} dB"
        );
    }
    ensure!(
        is_builtin(name) || custom.contains_key(name),
        "Unknown equalizer preset {name}"
    );
    Ok(())
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 24
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Settings text form: `name=g1,g2,…,g10; other=…`. Names are lower-cased.
pub fn format_custom(custom: &BTreeMap<String, Vec<f64>>) -> String {
    custom
        .iter()
        .map(|(name, gains)| {
            let gains = gains.iter().map(|g| g.to_string()).collect::<Vec<_>>();
            format!("{name}={}", gains.join(","))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn parse_custom(text: &str) -> Result<BTreeMap<String, Vec<f64>>> {
    let mut custom = BTreeMap::new();
    for entry in text.split(';').map(str::trim).filter(|e| !e.is_empty()) {
        let (name, gains) = entry
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("Write custom equalizers as name=g1,…,g10"))?;
        let gains = gains
            .split(',')
            .map(|g| g.trim().parse::<f64>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| anyhow::anyhow!("Equalizer gains must be numbers in dB"))?;
        let name = name.trim().to_lowercase();
        ensure!(
            custom.insert(name.clone(), gains).is_none(),
            "Custom equalizer {name} is listed twice"
        );
    }
    Ok(custom)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presets_are_valid_and_off_or_flat_add_no_filter() {
        let none = BTreeMap::new();
        for name in names(&none) {
            validate(&name, &none).unwrap();
        }
        assert_eq!(gains(OFF, &none), None);
        assert_eq!(filter(&gains("flat", &none).unwrap()), None);
        let bass = filter(&gains("bass-boost", &none).unwrap()).unwrap();
        assert_eq!(
            bass,
            "@vidcue-eq:lavfi=[equalizer=f=31:t=o:w=1:g=6,equalizer=f=62:t=o:w=1:g=5,equalizer=f=125:t=o:w=1:g=4,equalizer=f=250:t=o:w=1:g=2]"
        );
        assert!(
            PRESETS
                .iter()
                .all(|(_, g)| g.iter().all(|v| v.abs() <= MAX_GAIN))
        );
    }
    #[test]
    fn custom_presets_roundtrip_through_text_and_are_validated() {
        let custom =
            parse_custom(" Night = -3,-2,0,0,1,2,2,1,0,-1 ; car=1,1,1,1,1,1,1,1,1,1.5").unwrap();
        assert_eq!(custom["night"][0], -3.0);
        assert_eq!(parse_custom(&format_custom(&custom)).unwrap(), custom);
        validate("night", &custom).unwrap();
        assert_eq!(names(&custom).last().unwrap(), "night");
        assert!(
            filter(&gains("car", &custom).unwrap())
                .unwrap()
                .contains("g=1.5")
        );
        assert!(validate("missing", &custom).is_err());
        assert!(
            parse_custom("x=1,2")
                .map(|c| validate(OFF, &c))
                .unwrap()
                .is_err()
        );
        assert!(
            parse_custom("rock=0,0,0,0,0,0,0,0,0,0")
                .map(|c| validate(OFF, &c))
                .unwrap()
                .is_err()
        );
        assert!(
            parse_custom("x=13,0,0,0,0,0,0,0,0,0")
                .map(|c| validate(OFF, &c))
                .unwrap()
                .is_err()
        );
        assert!(parse_custom("a=1;a=2").is_err());
        assert!(parse_custom("a=loud").is_err());
        assert!(parse_custom("").unwrap().is_empty());
    }
}
