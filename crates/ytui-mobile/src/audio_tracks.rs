//! Choose language before codec; a compatible dub must not displace original audio.
use anyhow::{Result, ensure};
use serde_json::{Value, json};

pub(crate) fn valid_language(language: &str) -> bool {
    !language.is_empty()
        && language.len() <= 64
        && language
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

pub(crate) fn selector(base: &str, language: Option<&str>) -> Result<String> {
    let filters = if let Some(language) = language {
        ensure!(valid_language(language), "Invalid audio language");
        // An explicit choice must fail rather than silently play another language.
        vec![format!("[language={language}]")]
    } else {
        vec![
            "[language_preference>=10]".into(),
            "[language_preference>=5]".into(),
            String::new(),
        ]
    };
    Ok(filters
        .iter()
        .flat_map(|filter| {
            base.split('/').map(move |part| {
                // Apply the language to audio-only and muxed fallback formats.
                part.replace("bestaudio", &format!("bestaudio{filter}"))
                    .split('+')
                    .map(|term| {
                        if term == "best" || term.starts_with("best[") {
                            term.replacen("best", &format!("best{filter}"), 1)
                        } else {
                            term.to_owned()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("+")
            })
        })
        .collect::<Vec<_>>()
        .join("/"))
}

pub(crate) fn choices(data: &Value) -> Vec<Value> {
    let mut choices = Vec::new();
    for row in data["formats"].as_array().into_iter().flatten().take(1000) {
        let Some(language) = row["language"].as_str().filter(|s| valid_language(s)) else {
            continue;
        };
        if row["has_drm"] == true || row["acodec"].as_str().is_none_or(|s| s == "none") {
            continue;
        }
        if choices.iter().any(|v: &Value| v["language"] == language) {
            continue;
        }
        let original = row["language_preference"]
            .as_f64()
            .is_some_and(|n| n >= 10.0);
        choices.push(json!({"language":language,"label":format!("{language}{}", if original {" · Original"} else {""})}));
    }
    choices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_opus_beats_dubbed_aac_and_explicit_language_never_falls_back() {
        let formats = vec![
            json!({"format_id":"original","url":"https://media.test/en","vcodec":"none","acodec":"opus","language":"en","language_preference":10}),
            json!({"format_id":"dub","url":"https://media.test/zh","vcodec":"none","acodec":"mp4a.40.2","language":"zh","language_preference":5}),
            json!({"format_id":"video","url":"https://media.test/v","vcodec":"avc1","acodec":"none","height":1080}),
        ];
        let base = "bestvideo+bestaudio[acodec^=mp4a]/bestvideo+bestaudio/best";
        let select = |language| {
            yt_dlp_rs::format_select::parse(&selector(base, language).unwrap())
                .unwrap()
                .select(&formats, &Default::default(), None)
        };
        assert_eq!(select(None).unwrap()[0].formats, vec![2, 0]);
        assert_eq!(select(Some("zh")).unwrap()[0].formats, vec![2, 1]);
        assert!(select(Some("fr")).unwrap().is_empty());
        assert!(selector(base, Some("en]/best")).is_err());
        assert_eq!(choices(&json!({"formats":formats})).len(), 2);
    }

    #[test]
    fn default_and_unlabelled_audio_remain_playable() {
        for preference in [Some(5), None] {
            let mut row = json!({"vcodec":"none","acodec":"aac","url":"https://media.test/a"});
            if let Some(n) = preference {
                row["language_preference"] = n.into();
            }
            let selected =
                yt_dlp_rs::format_select::parse(&selector("bestaudio/best", None).unwrap())
                    .unwrap()
                    .select(&[row], &Default::default(), None)
                    .unwrap();
            assert_eq!(selected[0].formats, vec![0]);
        }
    }

    #[test]
    fn split_stream_exposes_all_languages_but_reports_the_selected_audio() {
        let video =
            json!({"format_id":"v","url":"https://media.test/v","vcodec":"avc1","acodec":"none"});
        let original = json!({"format_id":"en","url":"https://media.test/en","vcodec":"none","acodec":"opus","language":"en","language_preference":10});
        let dub = json!({"format_id":"zh","url":"https://media.test/zh","vcodec":"none","acodec":"aac","language":"zh","language_preference":5});
        let data =
            json!({"formats":[video,original,original,dub], "requested_formats":[video,original]});
        let stream = crate::stream(&data, false).unwrap();
        assert_eq!(stream["audio_language"], "en");
        assert_eq!(stream["audio_url"], "https://media.test/en");
        assert_eq!(stream["audio_tracks"].as_array().unwrap().len(), 2);
        assert_eq!(stream["audio_tracks"][0]["label"], "en · Original");
        assert_eq!(stream["audio_tracks"][1]["language"], "zh");
    }
}
