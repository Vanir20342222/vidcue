//! Fractional positions for playlist entries (PROTOCOL.md §7.8).
//!
//! Positions are strings over `0-9A-Za-z` (base 62 in ASCII order) compared bytewise, so a
//! new entry can always be placed between two others without renumbering, and two devices
//! inserting concurrently never conflict. Equal positions order by entry id.

pub const MAX_LEN: usize = 64;
const BASE: u32 = 62;

fn value(byte: u8) -> Option<u32> {
    match byte {
        b'0'..=b'9' => Some(u32::from(byte - b'0')),
        b'A'..=b'Z' => Some(u32::from(byte - b'A') + 10),
        b'a'..=b'z' => Some(u32::from(byte - b'a') + 36),
        _ => None,
    }
}

fn digit(value: u32) -> u8 {
    match value {
        0..=9 => b'0' + value as u8,
        10..=35 => b'A' + (value - 10) as u8,
        _ => b'a' + (value - 36) as u8,
    }
}

/// Whether `position` is a valid stored position (`[0-9A-Za-z]{1,64}`).
pub fn valid(position: &str) -> bool {
    !position.is_empty()
        && position.len() <= MAX_LEN
        && position.bytes().all(|b| value(b).is_some())
}

/// A position strictly between `lo` and `hi` (`None` = open end), never ending in `0`.
///
/// Returns `None` when no position of at most [`MAX_LEN`] characters exists (for example
/// `lo == hi`, or `hi` directly follows `lo` like `"a"`/`"a0"`); the caller then rebalances
/// the playlist with [`spread`].
pub fn between(lo: Option<&str>, hi: Option<&str>) -> Option<String> {
    let lo = lo.unwrap_or("");
    if !lo.is_empty() && !valid(lo) || hi.is_some_and(|h| !valid(h)) {
        return None;
    }
    if let Some(hi) = hi
        && lo >= hi
    {
        return None;
    }
    let mut out = Vec::new();
    midpoint(lo.as_bytes(), hi.map(str::as_bytes), &mut out, b'V')?;
    let text = String::from_utf8(out).ok()?;
    (text.len() <= MAX_LEN).then_some(text)
}

/// Appends to `out` a string `p` with `lo < p < hi`, where `lo < hi` bytewise. `fill` is
/// the digit used when both ends are open: the middle, or `1` after an appended digit
/// overflowed so repeated appends grow by one character per 61 steps.
fn midpoint(lo: &[u8], hi: Option<&[u8]>, out: &mut Vec<u8>, fill: u8) -> Option<()> {
    if out.len() > MAX_LEN {
        return None;
    }
    if let Some(hi) = hi {
        // Copy the shared prefix (a missing `lo` digit counts as `0`).
        let mut n = 0;
        while n < hi.len() && lo.get(n).copied().unwrap_or(b'0') == hi[n] {
            n += 1;
        }
        if n == hi.len() {
            // `hi` is `lo` padded with zeros: nothing fits between them.
            return None;
        }
        if n > 0 {
            out.extend_from_slice(&hi[..n]);
            return midpoint(lo.get(n..).unwrap_or(&[]), Some(&hi[n..]), out, fill);
        }
    }
    let low = lo.first().map_or(Some(0), |b| value(*b))?;
    let Some(hi) = hi else {
        // Open end: step up by one digit so repeated appends grow slowly.
        if lo.is_empty() {
            out.push(fill);
            return Some(());
        }
        if low + 1 < BASE {
            out.push(digit(low + 1));
            return Some(());
        }
        out.push(lo[0]);
        return midpoint(&lo[1..], None, out, b'1');
    };
    let high = value(hi[0])?;
    if high > low + 1 {
        // An empty `lo` means "before `hi`": step down by one so repeated prepends grow
        // slowly; otherwise take the middle digit.
        out.push(digit(if lo.is_empty() {
            high - 1
        } else {
            (low + high).div_ceil(2)
        }));
        return Some(());
    }
    // Adjacent first digits.
    if hi.len() > 1 {
        // `hi[0]` alone is greater than `lo` and smaller than `hi`, and it is not `0`
        // because `hi[0] > lo[0]`.
        out.push(hi[0]);
        return Some(());
    }
    out.push(digit(low));
    midpoint(lo.get(1..).unwrap_or(&[]), None, out, b'V')
}

/// `count` evenly spaced positions, all of the same length (two characters for up to 1921
/// entries), never ending in `0`, for rebalancing a whole playlist.
pub fn spread(count: usize) -> Vec<String> {
    if count == 0 {
        return Vec::new();
    }
    let mut width = 1u32;
    let mut space = u128::from(BASE);
    // At least two slots per entry, so skipping values that end in `0` never collides.
    while space < 2 * (count as u128 + 1) {
        width += 1;
        space *= u128::from(BASE);
    }
    let width = width.max(2);
    let space = u128::from(BASE).pow(width);
    let mut out = Vec::with_capacity(count);
    let mut previous = 0u128;
    for i in 1..=count as u128 {
        let mut n = (space * i / (count as u128 + 1)).max(previous + 1);
        if n % u128::from(BASE) == 0 {
            n += 1;
        }
        previous = n;
        let mut text = vec![b'0'; width as usize];
        let mut rest = n;
        for slot in text.iter_mut().rev() {
            *slot = digit((rest % u128::from(BASE)) as u32);
            rest /= u128::from(BASE);
        }
        out.push(String::from_utf8(text).expect("ASCII"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};

    #[test]
    fn simple_cases() {
        assert_eq!(between(None, None).unwrap(), "V");
        let a = between(None, None).unwrap();
        let after = between(Some(&a), None).unwrap();
        assert!(after > a);
        let before = between(None, Some(&a)).unwrap();
        assert!(before < a);
        assert_eq!(between(Some("a"), Some("b")).unwrap(), "aV");
        assert_eq!(between(Some("a"), Some("c")).unwrap(), "b");
        assert_eq!(between(Some("a1"), Some("b")).unwrap(), "a2");
        assert_eq!(between(Some("a"), Some("a1")).unwrap(), "a0V");
        assert_eq!(between(None, Some("1")).unwrap(), "0V");
        assert_eq!(between(None, Some("01")).unwrap(), "00V");
        assert_eq!(between(Some("z"), None).unwrap(), "z1");
        assert_eq!(between(Some("V"), None).unwrap(), "W");
        assert_eq!(between(None, Some("V")).unwrap(), "U");
        assert!(between(Some("a"), Some("a")).is_none());
        assert!(between(Some("b"), Some("a")).is_none());
        assert!(between(Some("a"), Some("a0")).is_none());
        assert!(between(Some("a-"), None).is_none());
        for p in [&a, &after, &before] {
            assert!(!p.ends_with('0'));
        }
    }

    fn check(lo: Option<&str>, hi: Option<&str>, p: &str) {
        assert!(valid(p), "{p}");
        assert!(!p.ends_with('0'), "{p}");
        if let Some(lo) = lo {
            assert!(lo < p, "{lo} < {p}");
        }
        if let Some(hi) = hi {
            assert!(p < hi, "{p} < {hi}");
        }
    }

    #[test]
    fn ten_thousand_random_inserts_keep_order_and_limits() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(0x5eed);
        let mut list: Vec<String> = Vec::new();
        let mut rebalanced = 0;
        for _ in 0..10_000 {
            let at = rng.gen_range(0..=list.len());
            let lo = at.checked_sub(1).map(|i| list[i].as_str());
            let hi = list.get(at).map(String::as_str);
            match between(lo, hi) {
                Some(p) => {
                    check(lo, hi, &p);
                    list.insert(at, p);
                }
                None => {
                    // Only allowed when the gap is exhausted; rebalance and retry.
                    list = spread(list.len());
                    rebalanced += 1;
                    let lo = at.checked_sub(1).map(|i| list[i].as_str());
                    let hi = list.get(at).map(String::as_str);
                    let p = between(lo, hi).expect("room after rebalancing");
                    check(lo, hi, &p);
                    list.insert(at, p);
                }
            }
            assert!(list.windows(2).all(|w| w[0] < w[1]));
            assert!(list.iter().all(|p| p.len() <= MAX_LEN));
        }
        // Random inserts rarely exhaust 64 characters.
        assert!(rebalanced < 10, "{rebalanced}");
    }

    #[test]
    fn appending_and_prepending_stay_short() {
        let mut last: Option<String> = None;
        for _ in 0..2000 {
            let p = between(last.as_deref(), None).unwrap();
            last = Some(p);
        }
        assert!(last.unwrap().len() <= 40);
        let mut first: Option<String> = None;
        for _ in 0..200 {
            first = Some(between(None, first.as_deref()).unwrap());
        }
        assert!(first.unwrap().len() <= MAX_LEN);
    }

    #[test]
    fn spread_is_sorted_and_never_ends_in_zero() {
        for n in [0, 1, 2, 61, 62, 1000, 1921, 1922, 5000] {
            let list = spread(n);
            assert_eq!(list.len(), n);
            assert!(list.windows(2).all(|w| w[0] < w[1]));
            assert!(list.iter().all(|p| valid(p) && !p.ends_with('0')));
            if n <= 1921 {
                assert!(list.iter().all(|p| p.len() == 2), "{n}");
            }
            // There is room between neighbours.
            for w in list.windows(2) {
                assert!(between(Some(&w[0]), Some(&w[1])).is_some());
            }
        }
    }
}
