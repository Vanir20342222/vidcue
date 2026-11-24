//! Content filters (PROTOCOL.md §8.5), pure so the server can mirror them.
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use vidcue_core::Media;

pub const KINDS: &[&str] = &["channel", "keyword", "shorts", "watched", "live"];
pub const MAX_VALUE_CHARS: usize = 200;

/// One synced filter (`filters` collection; key = id, value = `{kind, value?}`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filter {
    pub id: Uuid,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

impl Filter {
    /// Validates and normalizes a new filter the way the server does.
    pub fn new(id: Uuid, kind: &str, value: Option<&str>) -> anyhow::Result<Self> {
        anyhow::ensure!(KINDS.contains(&kind), "Unknown filter kind");
        let value = value
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| v.chars().take(MAX_VALUE_CHARS).collect::<String>());
        if matches!(kind, "channel" | "keyword") {
            anyhow::ensure!(value.is_some(), "This filter needs a value");
            anyhow::ensure!(
                value
                    .as_deref()
                    .is_some_and(|v| !v.chars().any(char::is_control)),
                "The filter value is invalid"
            );
        }
        Ok(Self {
            id,
            kind: kind.into(),
            value: if matches!(kind, "channel" | "keyword") {
                value
            } else {
                None
            },
        })
    }
    /// The register value pushed to the `filters` collection.
    pub fn register_value(&self) -> serde_json::Value {
        let mut value = serde_json::json!({ "kind": self.kind });
        if let Some(text) = &self.value {
            value["value"] = text.clone().into();
        }
        value
    }
}

fn is_short(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|u| u.path().starts_with("/shorts/"))
}

/// Whether one filter hides `media`. `watched` reports whether the resume entry of the
/// media is finished.
pub fn matches(filter: &Filter, media: &Media, watched: &dyn Fn(&Media) -> bool) -> bool {
    let value = filter.value.as_deref().map(str::trim).unwrap_or_default();
    match filter.kind.as_str() {
        "channel" => {
            !value.is_empty()
                && (media.channel_id == value
                    || media.creator.trim().to_lowercase() == value.to_lowercase())
        }
        "keyword" => {
            !value.is_empty() && media.title.to_lowercase().contains(&value.to_lowercase())
        }
        "shorts" => is_short(&media.url),
        "watched" => watched(media),
        "live" => media.live,
        _ => false,
    }
}

/// Whether any filter hides `media`.
pub fn hidden(filters: &[Filter], media: &Media, watched: &dyn Fn(&Media) -> bool) -> bool {
    filters.iter().any(|f| matches(f, media, watched))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media(title: &str, creator: &str, url: &str) -> Media {
        Media {
            title: title.into(),
            creator: creator.into(),
            url: url.into(),
            channel_id: "UCxxxxxxxxxxxxxxxxxxxxxx".into(),
            ..Default::default()
        }
    }

    #[test]
    fn each_kind_matches_as_specified() {
        let f = |kind: &str, value: Option<&str>| Filter::new(Uuid::new_v4(), kind, value).unwrap();
        let normal = media(
            "Cooking Show",
            "Chef",
            "https://www.youtube.com/watch?v=abcdefghijk",
        );
        let short = media("x", "y", "https://www.youtube.com/shorts/abcdefghijk");
        let never = |_: &Media| false;
        let always = |_: &Media| true;
        assert!(matches(
            &f("channel", Some("UCxxxxxxxxxxxxxxxxxxxxxx")),
            &normal,
            &never
        ));
        assert!(matches(&f("channel", Some("  chef ")), &normal, &never));
        assert!(!matches(&f("channel", Some("Che")), &normal, &never));
        assert!(matches(&f("keyword", Some("COOK")), &normal, &never));
        assert!(!matches(&f("keyword", Some("baking")), &normal, &never));
        assert!(matches(&f("shorts", None), &short, &never));
        assert!(!matches(&f("shorts", None), &normal, &never));
        assert!(matches(&f("watched", None), &normal, &always));
        assert!(!matches(&f("watched", None), &normal, &never));
        let mut live = normal.clone();
        live.live = true;
        assert!(matches(&f("live", None), &live, &never));
        assert!(!hidden(&[], &live, &always));
        assert!(Filter::new(Uuid::new_v4(), "keyword", Some("  ")).is_err());
        assert!(Filter::new(Uuid::new_v4(), "other", None).is_err());
        assert_eq!(f("shorts", Some("ignored")).value, None);
        assert_eq!(
            f("keyword", Some("a")).register_value(),
            serde_json::json!({"kind":"keyword","value":"a"})
        );
    }
}
