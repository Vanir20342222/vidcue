//! Public title/episode metadata, separate from stream availability. No embedded API keys.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use url::Url;

const BASE: &str = "https://v3-cinemeta.strem.io/";
const MAX_BYTES: usize = 4 * 1024 * 1024;

pub struct MovieCatalog {
    client: reqwest::Client,
    base: Url,
}

impl MovieCatalog {
    pub fn new() -> Result<Self> {
        Self::with_proxy(None)
    }

    pub fn with_proxy(proxy: Option<&str>) -> Result<Self> {
        Ok(Self {
            client: crate::http_client(proxy)?,
            base: Url::parse(BASE)?,
        })
    }

    async fn fetch(&self, segments: &[&str]) -> Result<Value> {
        let mut url = self.base.clone();
        url.set_path(&format!("/{}", segments.join("/")));
        // Do not include provider URLs or response bodies in diagnostics.
        let mut response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("Movie catalog is unreachable"))?;
        ensure!(
            response.status().is_success(),
            "Movie catalog returned HTTP {}",
            response.status().as_u16()
        );
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| anyhow::anyhow!("Movie catalog response interrupted"))?
        {
            ensure!(
                body.len() + chunk.len() <= MAX_BYTES,
                "Movie catalog response too large"
            );
            body.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&body).map_err(|_| anyhow::anyhow!("Invalid movie catalog response"))
    }

    /// Search both media types; keep successful results if one catalog is unavailable.
    pub async fn search(&self, query: &str) -> Result<Value> {
        let query = query.trim();
        ensure!(
            !query.is_empty() && query.len() <= 512,
            "Enter a title (up to 512 bytes)"
        );
        let encoded = url::form_urlencoded::byte_serialize(query.as_bytes())
            .collect::<String>()
            .replace('+', "%20");
        let extra = format!("search={encoded}.json");
        let movie_path = ["catalog", "movie", "top", &extra];
        let series_path = ["catalog", "series", "top", &extra];
        let (movies, series) = tokio::join!(self.fetch(&movie_path), self.fetch(&series_path));
        let mut titles = Vec::new();
        let mut warnings = Vec::new();
        let mut succeeded = false;
        for (kind, result) in [("movie", movies), ("series", series)] {
            match result.and_then(|v| {
                v["metas"]
                    .as_array()
                    .cloned()
                    .context("Invalid catalog results")
            }) {
                Ok(items) => {
                    succeeded = true;
                    titles.extend(items.into_iter().filter(|v| v["type"] == kind));
                }
                Err(_) => warnings.push(format!("{kind} search unavailable; try again")),
            }
        }
        ensure!(succeeded, "Movie and series search unavailable; try again");
        Ok(json!({"titles": titles, "warnings": warnings}))
    }

    /// Complete episode metadata (including specials) is fetched only when opening a title.
    pub async fn details(&self, kind: &str, imdb: &str) -> Result<Value> {
        ensure!(
            matches!(kind, "movie" | "series"),
            "Invalid catalog media type"
        );
        ensure!(
            imdb.starts_with("tt")
                && imdb.len() > 2
                && imdb.len() <= 20
                && imdb[2..].bytes().all(|b| b.is_ascii_digit()),
            "Invalid catalog title ID"
        );
        let value = self.fetch(&["meta", kind, &format!("{imdb}.json")]).await?;
        let meta = &value["meta"];
        ensure!(
            meta.is_object() && meta["id"] == imdb && meta["type"] == kind,
            "Catalog title not found"
        );
        Ok(meta.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn failures_are_visible_and_search_extras_are_encoded() {
        let (base, task) = crate::test_http::serve(|path| {
            assert!(path.ends_with("/search=A%2FB%20%26%20C%3D.json"), "{path}");
            if path.contains("/movie/") {
                (503, String::new(), Vec::new())
            } else {
                (200, String::new(), br#"{"metas":[]}"#.to_vec())
            }
        })
        .await;
        let catalog = MovieCatalog {
            client: crate::http_client(None).unwrap(),
            base: Url::parse(&base).unwrap(),
        };
        let result = catalog.search("A/B & C=").await.unwrap();
        assert!(result["titles"].as_array().unwrap().is_empty());
        assert_eq!(result["warnings"].as_array().unwrap().len(), 1);
        task.abort();
        let (base, task) = crate::test_http::serve(|_| (503, String::new(), Vec::new())).await;
        let catalog = MovieCatalog {
            client: crate::http_client(None).unwrap(),
            base: Url::parse(&base).unwrap(),
        };
        assert!(catalog.search("cyberpunk").await.is_err());
        task.abort();
    }

    #[tokio::test]
    async fn searches_both_types_and_loads_all_episode_metadata() {
        let (base, task) = crate::test_http::serve(|path| {
            let body = if path == "/catalog/movie/top/search=cyberpunk.json" {
                json!({"metas":[]})
            } else if path == "/catalog/series/top/search=cyberpunk.json" {
                json!({"metas":[{"id":"tt12590266","type":"series","name":"Cyberpunk: Edgerunners"}]})
            } else if path == "/meta/series/tt12590266.json" {
                json!({"meta":{"id":"tt12590266","type":"series","moviedb_id":105248,"videos":(1..=10).map(|episode| json!({"season":1,"episode":episode})).collect::<Vec<_>>()}})
            } else { panic!("Unexpected request: {path}") };
            (200, "Content-Type: application/json\r\n".into(), body.to_string().into_bytes())
        }).await;
        let catalog = MovieCatalog {
            client: crate::http_client(None).unwrap(),
            base: Url::parse(&base).unwrap(),
        };
        let result = catalog.search(" cyberpunk ").await.unwrap();
        assert_eq!(result["titles"][0]["name"], "Cyberpunk: Edgerunners");
        let detail = catalog.details("series", "tt12590266").await.unwrap();
        assert_eq!(detail["moviedb_id"], 105248);
        assert_eq!(detail["videos"].as_array().unwrap().len(), 10);
        assert!(catalog.details("series", "../secret").await.is_err());
        task.abort();
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;
    #[tokio::test]
    #[ignore = "explicit public metadata check; no stream requests"]
    async fn live_cyberpunk_discovery_and_episode_listing() {
        let catalog = MovieCatalog::new().unwrap();
        let found = catalog.search("cyberpunk").await.unwrap();
        let title = found["titles"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == "tt12590266" && m["type"] == "series")
            .unwrap();
        assert_eq!(title["name"], "Cyberpunk: Edgerunners");
        let meta = catalog.details("series", "tt12590266").await.unwrap();
        assert_eq!(meta["moviedb_id"], 105248);
        let episodes = meta["videos"].as_array().unwrap();
        for episode in 1..=10 {
            assert!(
                episodes
                    .iter()
                    .any(|e| e["season"] == 1 && e["episode"] == episode)
            );
        }
    }
}
