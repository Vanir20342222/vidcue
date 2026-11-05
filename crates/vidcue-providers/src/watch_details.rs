//! Watch-page details that the fast playback path does not return: publication date, likes and
//! subscribers. They come from YouTube's `next` response (the same endpoint as related videos),
//! not from the multi-megabyte watch page. Labels stay as the server localized them; only a like
//! count written out in full digits becomes a number, never a guessed "1.2K".

use serde_json::{Map, Value, json};

/// Details found in a `next` response; empty fields are left out.
pub fn parse(next: &Value) -> Value {
    let mut out = Map::new();
    let primary = find(next, "videoPrimaryInfoRenderer");
    let secondary = find(next, "videoSecondaryInfoRenderer");
    if let Some(primary) = primary {
        // "3 days ago" reads like the card labels; the absolute date is the fallback.
        let relative = primary.get("relativeDateText").map(crate::browse::text);
        let absolute = primary.get("dateText").map(crate::browse::text);
        if let Some(date) = relative
            .filter(|t| !t.is_empty())
            .or(absolute.filter(|t| !t.is_empty()))
        {
            out.insert("published_text".into(), date.into());
        }
        if let Some(likes) = find(primary, "likeButtonViewModel")
            .and_then(|like| find(like, "accessibilityText"))
            .and_then(Value::as_str)
            .and_then(full_count)
        {
            out.insert("like_count".into(), likes.into());
        }
    }
    if let Some(subscribers) = secondary
        .and_then(|s| find(s, "subscriberCountText"))
        .map(crate::browse::text)
        .filter(|t| !t.is_empty())
    {
        out.insert("subscribers_text".into(), subscribers.into());
    }
    Value::Object(out)
}

/// The request body for one video's `next` response.
pub fn request(video_id: &str) -> Value {
    json!({ "videoId": video_id })
}

fn find<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => map
            .get(key)
            .or_else(|| map.values().find_map(|v| find(v, key))),
        Value::Array(items) => items.iter().find_map(|v| find(v, key)),
        _ => None,
    }
}

/// The first number in `label` when it is written out in full ("like this video along with
/// 123,456 other people", "123.456 „Mag ich“-Bewertungen"). A compact form such as "1.2K" or
/// "1,2 Mio." is refused rather than misread.
fn full_count(label: &str) -> Option<u64> {
    let chars: Vec<char> = label.chars().collect();
    let start = chars.iter().position(char::is_ascii_digit)?;
    let separators = ['.', ',', '\u{a0}', '\u{202f}', '\'', ' '];
    let mut end = start;
    while end < chars.len()
        && (chars[end].is_ascii_digit()
            || (separators.contains(&chars[end])
                && chars.get(end + 1).is_some_and(char::is_ascii_digit)))
    {
        end += 1;
    }
    let run: String = chars[start..end].iter().collect();
    // Grouping only: every separated group after the first has exactly three digits.
    let groups: Vec<&str> = run
        .split(|c: char| !c.is_ascii_digit())
        .filter(|g| !g.is_empty())
        .collect();
    if groups.iter().skip(1).any(|g| g.len() != 3) || groups.first()?.len() > 3 && groups.len() > 1
    {
        return None;
    }
    // A unit right after the number marks a compact count.
    let rest: String = chars[end..].iter().collect();
    let unit = rest.split_whitespace().next().unwrap_or("");
    let unit = unit
        .trim_end_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase();
    if ["k", "m", "b", "mio", "mrd", "tsd", "mil", "md", "mln", "bn"].contains(&unit.as_str()) {
        return None;
    }
    groups.concat().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(like: &str, relative: Option<&str>) -> Value {
        let mut primary = json!({
            "dateText": {"simpleText": "Nov 2, 2026"},
            "videoActions": {"menuRenderer": {"topLevelButtons": [{
                "segmentedLikeDislikeButtonViewModel": {"likeButtonViewModel": {
                    "likeButtonViewModel": {"toggleButtonViewModel": {"toggleButtonViewModel": {
                        "defaultButtonViewModel": {"buttonViewModel": {"accessibilityText": like}}
                    }}}
                }}
            }]}}
        });
        if let Some(text) = relative {
            primary["relativeDateText"] = json!({"simpleText": text});
        }
        json!({"contents": {"twoColumnWatchNextResults": {"results": {"results": {"contents": [
            {"videoPrimaryInfoRenderer": primary},
            {"videoSecondaryInfoRenderer": {"owner": {"videoOwnerRenderer": {
                "subscriberCountText": {"simpleText": "1.23M subscribers"}
            }}}}
        ]}}}}})
    }

    #[test]
    fn reads_date_likes_and_subscribers() {
        let details = parse(&response(
            "like this video along with 123,456 other people",
            Some("3 days ago"),
        ));
        assert_eq!(details["published_text"], "3 days ago");
        assert_eq!(details["like_count"], 123_456);
        assert_eq!(details["subscribers_text"], "1.23M subscribers");
    }

    #[test]
    fn falls_back_to_the_absolute_date() {
        let details = parse(&response("12 likes", None));
        assert_eq!(details["published_text"], "Nov 2, 2026");
        assert_eq!(details["like_count"], 12);
    }

    #[test]
    fn localized_full_counts_parse_and_compact_counts_are_refused() {
        assert_eq!(full_count("123.456 „Mag ich“-Bewertungen"), Some(123_456));
        assert_eq!(full_count("1\u{a0}234\u{a0}567 J’aime"), Some(1_234_567));
        assert_eq!(full_count("7 likes"), Some(7));
        assert_eq!(full_count("1.2K likes"), None);
        assert_eq!(full_count("1,2 Mio. „Mag ich“-Bewertungen"), None);
        assert_eq!(full_count("12,5 Tsd."), None);
        assert_eq!(full_count("no number"), None);
    }

    #[test]
    fn missing_renderers_give_no_fields() {
        assert_eq!(parse(&json!({})), json!({}));
    }
}
