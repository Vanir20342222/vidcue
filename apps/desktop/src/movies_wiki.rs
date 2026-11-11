//! Keyless film metadata from Wikimedia: Wikidata links TMDB ids (P4947 films, P4983 series)
//! to English Wikipedia articles, whose page image is usually the poster and whose intro is a
//! short description. Used for the curated shelf; legacy search is retained for live tests.

use std::collections::HashMap;

use anyhow::{Context, Result};
use serde_json::Value;

#[cfg(test)]
use super::tmdb::Title;

const UA: &str = "Vidcue/0.2 (desktop app; https://github.com/Vanir20342222/vidcue)";
const SPARQL: &str = "https://query.wikidata.org/sparql";
const WIKI_API: &str = "https://en.wikipedia.org/w/api.php";
#[cfg(test)]
const WIKIDATA_API: &str = "https://www.wikidata.org/w/api.php";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    pub title: String,
    pub overview: String,
    pub year: String,
    pub poster: String,
}

async fn get_json(client: &reqwest::Client, url: &str, query: &[(&str, &str)]) -> Result<Value> {
    let r = client
        .get(url)
        .header("User-Agent", UA)
        .header("Accept", "application/json")
        .query(query)
        .send()
        .await?;
    anyhow::ensure!(
        r.status().is_success(),
        "Wikimedia answered {}",
        r.status().as_u16()
    );
    let text = vidcue_providers::browse::limited_text(r, 4 * 1024 * 1024).await?;
    Ok(serde_json::from_str(&text)?)
}

/// `https://en.wikipedia.org/wiki/Fight_Club` → `Fight Club`.
pub fn article_title(url: &str) -> Option<String> {
    let path = url.split("/wiki/").nth(1)?;
    let mut bytes = Vec::with_capacity(path.len());
    let raw = path.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%'
            && i + 2 < raw.len()
            && let Ok(b) = u8::from_str_radix(&path[i + 1..i + 3], 16)
        {
            bytes.push(b);
            i += 3;
            continue;
        }
        bytes.push(if raw[i] == b'_' { b' ' } else { raw[i] });
        i += 1;
    }
    String::from_utf8(bytes).ok().filter(|t| !t.is_empty())
}

fn year_of(date: &str) -> String {
    let d = date.trim_start_matches('+');
    d.get(..4)
        .filter(|y| y.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or_default()
        .to_owned()
}

/// Posters, intros and years for TMDB ids, keyed by `(tmdb, tv)`.
pub async fn enrich(
    client: &reqwest::Client,
    ids: &[(u64, bool)],
) -> Result<HashMap<(u64, bool), Meta>> {
    // One plain query per kind: Wikidata's planner times out on a UNION of VALUES blocks.
    let mut by_article: HashMap<String, ((u64, bool), String)> = HashMap::new();
    for tv in [false, true] {
        let list = ids
            .iter()
            .filter(|(_, t)| *t == tv)
            .map(|(id, _)| format!("\"{id}\""))
            .collect::<Vec<_>>()
            .join(" ");
        if list.is_empty() {
            continue;
        }
        let query = format!(
            "SELECT ?id ?article ?date WHERE {{ VALUES ?id {{ {list} }} ?item wdt:{} ?id . \
             ?article schema:about ?item ; schema:isPartOf <https://en.wikipedia.org/> . \
             OPTIONAL {{ ?item wdt:P577 ?date }} }}",
            if tv { "P4983" } else { "P4947" }
        );
        let v = get_json(client, SPARQL, &[("query", &query), ("format", "json")])
            .await
            .context("Wikidata lookup failed")?;
        for b in v["results"]["bindings"].as_array().into_iter().flatten() {
            let (Some(id), Some(article)) = (
                b["id"]["value"]
                    .as_str()
                    .and_then(|s| s.parse::<u64>().ok()),
                b["article"]["value"].as_str().and_then(article_title),
            ) else {
                continue;
            };
            let year = b["date"]["value"].as_str().map(year_of).unwrap_or_default();
            by_article.entry(article).or_insert(((id, tv), year));
        }
    }
    let articles: Vec<String> = by_article.keys().cloned().collect();
    let pages = pages(client, &articles).await?;
    let mut out = HashMap::new();
    for (article, (key, year)) in by_article {
        let page = pages.get(&article).cloned().unwrap_or_default();
        out.insert(
            key,
            Meta {
                title: strip_suffix(&article),
                year,
                ..page
            },
        );
    }
    Ok(out)
}

/// "Fight Club (film)" → "Fight Club".
fn strip_suffix(t: &str) -> String {
    match t.rfind(" (") {
        Some(i)
            if t.ends_with(')')
                && (t[i..].contains("film")
                    || t[i..].contains("series")
                    || t[i..].contains("TV")) =>
        {
            t[..i].to_owned()
        }
        _ => t.to_owned(),
    }
}

/// Poster and intro for Wikipedia articles, 20 per request (the extracts limit).
async fn pages(client: &reqwest::Client, titles: &[String]) -> Result<HashMap<String, Meta>> {
    let mut out = HashMap::new();
    for chunk in titles.chunks(20) {
        let joined = chunk.join("|");
        let v = get_json(
            client,
            WIKI_API,
            &[
                ("action", "query"),
                ("format", "json"),
                ("formatversion", "2"),
                ("redirects", "1"),
                ("prop", "pageimages|extracts"),
                ("piprop", "thumbnail"),
                ("pithumbsize", "500"),
                ("pilicense", "any"),
                ("exintro", "1"),
                ("explaintext", "1"),
                ("exsentences", "3"),
                ("exlimit", "20"),
                ("titles", &joined),
            ],
        )
        .await?;
        // Normalized and redirected titles map back to what was asked.
        let mut alias: HashMap<String, String> = HashMap::new();
        for key in ["normalized", "redirects"] {
            for n in v["query"][key].as_array().into_iter().flatten() {
                if let (Some(from), Some(to)) = (n["from"].as_str(), n["to"].as_str()) {
                    let original = alias.get(from).cloned().unwrap_or_else(|| from.to_owned());
                    alias.insert(to.to_owned(), original);
                }
            }
        }
        for p in v["query"]["pages"].as_array().into_iter().flatten() {
            let Some(title) = p["title"].as_str() else {
                continue;
            };
            let asked = alias
                .get(title)
                .cloned()
                .unwrap_or_else(|| title.to_owned());
            out.insert(
                asked,
                Meta {
                    title: title.to_owned(),
                    overview: p["extract"].as_str().unwrap_or_default().trim().to_owned(),
                    poster: p["thumbnail"]["source"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                    year: String::new(),
                },
            );
        }
    }
    Ok(out)
}

/// Films and series matching `q` that have a TMDB id, in Wikipedia's relevance order.
#[cfg(test)]
pub async fn search(client: &reqwest::Client, q: &str) -> Result<Vec<Title>> {
    let v = get_json(
        client,
        WIKI_API,
        &[
            ("action", "query"),
            ("format", "json"),
            ("formatversion", "2"),
            ("generator", "search"),
            ("gsrsearch", q),
            ("gsrlimit", "20"),
            ("prop", "pageimages|pageprops|extracts"),
            ("piprop", "thumbnail"),
            ("pithumbsize", "500"),
            ("pilicense", "any"),
            ("ppprop", "wikibase_item"),
            ("exintro", "1"),
            ("explaintext", "1"),
            ("exsentences", "3"),
            ("exlimit", "20"),
        ],
    )
    .await
    .context("Wikipedia search failed")?;
    let mut pages: Vec<&Value> = v["query"]["pages"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    pages.sort_by_key(|p| p["index"].as_i64().unwrap_or(i64::MAX));
    let items: Vec<String> = pages
        .iter()
        .filter_map(|p| p["pageprops"]["wikibase_item"].as_str().map(str::to_owned))
        .collect();
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let joined = items.join("|");
    let claims = get_json(
        client,
        WIKIDATA_API,
        &[
            ("action", "wbgetentities"),
            ("format", "json"),
            ("props", "claims"),
            ("ids", &joined),
        ],
    )
    .await
    .context("Wikidata lookup failed")?;
    let claim = |q: &str, prop: &str| -> Option<String> {
        claims["entities"][q]["claims"][prop].as_array()?.first()?["mainsnak"]["datavalue"]["value"]
            .as_str()
            .map(str::to_owned)
    };
    let date = |q: &str| -> String {
        claims["entities"][q]["claims"]["P577"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|c| c["mainsnak"]["datavalue"]["value"]["time"].as_str())
            .map(year_of)
            .unwrap_or_default()
    };
    let mut out = Vec::new();
    for p in pages {
        let Some(item) = p["pageprops"]["wikibase_item"].as_str() else {
            continue;
        };
        let (id, tv) = match (claim(item, "P4947"), claim(item, "P4983")) {
            (Some(m), _) => (m, false),
            (None, Some(s)) => (s, true),
            _ => continue,
        };
        let Ok(tmdb) = id.parse::<u64>() else {
            continue;
        };
        out.push(Title {
            tmdb,
            tv,
            title: strip_suffix(p["title"].as_str().unwrap_or_default()),
            overview: p["extract"].as_str().unwrap_or_default().trim().to_owned(),
            year: date(item),
            poster: p["thumbnail"]["source"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            season: tv.then_some(1),
            episode: tv.then_some(1),
            ..Default::default()
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn article_urls_decode_to_titles() {
        assert_eq!(
            article_title("https://en.wikipedia.org/wiki/Fight_Club").as_deref(),
            Some("Fight Club")
        );
        assert_eq!(
            article_title("https://en.wikipedia.org/wiki/Am%C3%A9lie").as_deref(),
            Some("Amélie")
        );
        assert_eq!(
            article_title("https://en.wikipedia.org/wiki/Dune_(2021_film)").as_deref(),
            Some("Dune (2021 film)")
        );
        assert!(article_title("https://example.com/x").is_none());
    }

    #[test]
    fn titles_and_years_are_tidied() {
        assert_eq!(strip_suffix("Dune (2021 film)"), "Dune");
        assert_eq!(strip_suffix("Breaking Bad"), "Breaking Bad");
        assert_eq!(strip_suffix("Sherlock (TV series)"), "Sherlock");
        assert_eq!(
            strip_suffix("Mission: Impossible (1996)"),
            "Mission: Impossible (1996)"
        );
        assert_eq!(year_of("+1999-10-15T00:00:00Z"), "1999");
        assert_eq!(year_of("1999-10-15T00:00:00Z"), "1999");
        assert_eq!(year_of(""), "");
    }
}
