//! QR codes as rows of `0`/`1` (1 = dark) for native drawing, quiet zone included.
use anyhow::Result;
use qrcode::{Color, EcLevel, QrCode};

pub const QUIET_ZONE: usize = 4;

/// Error correction level M, 4-module quiet zone.
pub fn rows(text: &str) -> Result<Vec<String>> {
    let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M)?;
    let width = code.width();
    let colors = code.to_colors();
    let size = width + 2 * QUIET_ZONE;
    let mut out = Vec::with_capacity(size);
    let blank = "0".repeat(size);
    for _ in 0..QUIET_ZONE {
        out.push(blank.clone());
    }
    for y in 0..width {
        let mut row = String::with_capacity(size);
        row.push_str(&"0".repeat(QUIET_ZONE));
        for x in 0..width {
            row.push(if colors[y * width + x] == Color::Dark {
                '1'
            } else {
                '0'
            });
        }
        row.push_str(&"0".repeat(QUIET_ZONE));
        out.push(row);
    }
    for _ in 0..QUIET_ZONE {
        out.push(blank.clone());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn rows_are_square_with_a_quiet_zone() {
        let rows =
            super::rows("vidcue://pair?server=https%3A%2F%2Fa.example&code=ABCDEFGH23").unwrap();
        let n = rows.len();
        assert!(rows.iter().all(|r| r.len() == n));
        assert!(rows[..4].iter().all(|r| !r.contains('1')));
        assert!(
            rows.iter()
                .all(|r| r.starts_with("0000") && r.ends_with("0000"))
        );
        // Finder pattern top-left: 7 dark modules after the quiet zone.
        assert_eq!(&rows[4][4..11], "1111111");
    }
}
