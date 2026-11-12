//! Opaque, expiring references to resources issued by the application.
use rand::RngCore;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
const TTL: Duration = Duration::from_secs(6 * 3600);
const MAX_GRANTS: usize = 25_000;
pub struct MediaGrants {
    secret: [u8; 32],
    entries: HashMap<String, (String, Instant)>,
}
impl Default for MediaGrants {
    fn default() -> Self {
        let mut secret = [0; 32];
        rand::thread_rng().fill_bytes(&mut secret);
        Self {
            secret,
            entries: HashMap::new(),
        }
    }
}
impl MediaGrants {
    pub fn issue(&mut self, target: &str, route: &str) -> String {
        let mut hash = Sha256::new();
        hash.update(self.secret);
        hash.update(route.as_bytes());
        hash.update(target.as_bytes());
        let token = hash
            .finalize()
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect::<String>();
        if !self.entries.contains_key(&token) && self.entries.len() >= MAX_GRANTS {
            let now = Instant::now();
            self.entries.retain(|_, (_, expiry)| *expiry > now);
            if self.entries.len() >= MAX_GRANTS {
                let mut old: Vec<_> = self
                    .entries
                    .iter()
                    .map(|(k, (_, e))| (k.clone(), *e))
                    .collect();
                old.sort_by_key(|(_, e)| *e);
                for (k, _) in old.into_iter().take(2500) {
                    self.entries.remove(&k);
                }
            }
        }
        self.entries
            .insert(token.clone(), (target.to_owned(), Instant::now() + TTL));
        format!("/media/{route}?t={token}")
    }
    pub fn get(&mut self, token: &str, route: &str) -> Option<String> {
        let (target, expiry) = self.entries.get(token)?;
        if *expiry <= Instant::now() {
            return None;
        }
        let target = target.clone();
        let mut hash = Sha256::new();
        hash.update(self.secret);
        hash.update(route.as_bytes());
        hash.update(target.as_bytes());
        let expected = hash
            .finalize()
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect::<String>();
        if expected != token {
            return None;
        }
        self.entries.get_mut(token)?.1 = Instant::now() + TTL;
        Some(target)
    }
    pub fn rewrite_api(&mut self, value: &mut Value, key: &str) {
        match value {
            Value::Object(map) => {
                for (k, v) in map {
                    self.rewrite_api(v, k)
                }
            }
            Value::Array(items) => {
                for v in items {
                    self.rewrite_api(v, key)
                }
            }
            Value::String(s) if s.starts_with("https://") || s.starts_with("http://") => {
                let route = match key {
                    "cover" | "avatar" | "poster" | "thumbnail" | "cover_url" | "image_url" => {
                        Some("img")
                    }
                    "video_url" | "video_url_hevc" | "main" | "hevc" | "preview" => {
                        Some(if s.split('?').next().unwrap_or("").ends_with(".m3u8") {
                            "hls"
                        } else {
                            "seg"
                        })
                    }
                    _ => None,
                };
                if let Some(route) = route {
                    *s = self.issue(s, route);
                }
            }
            _ => {}
        }
    }
    pub fn rewrite_playlist(&mut self, text: &str) -> String {
        let marker = "/media/seg?u=";
        let mut rest = text;
        let mut out = String::with_capacity(text.len());
        while let Some(i) = rest.find(marker) {
            out.push_str(&rest[..i]);
            rest = &rest[i + marker.len()..];
            let end = rest
                .find(|c: char| c == '"' || c == '\r' || c == '\n' || c.is_whitespace())
                .unwrap_or(rest.len());
            let encoded = &rest[..end];
            let target = percent_encoding::percent_decode_str(encoded).decode_utf8_lossy();
            out.push_str(&self.issue(&target, "seg"));
            rest = &rest[end..];
        }
        out.push_str(rest);
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grants_are_opaque_and_bound_to_route() {
        let mut g = MediaGrants::default();
        let u = g.issue("https://example.com/image.jpg", "img");
        assert!(!u.contains("example.com"));
        let t = u.split("?t=").nth(1).unwrap();
        assert_eq!(
            g.get(t, "img").as_deref(),
            Some("https://example.com/image.jpg")
        );
        assert!(g.get(t, "seg").is_none());
        assert!(g.get("invented", "img").is_none());
    }
    #[test]
    fn rewrites_keys_and_nested_playlist_references() {
        let mut g = MediaGrants::default();
        let p=g.rewrite_playlist("#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"/media/seg?u=https%3A%2F%2Fexample.com%2Fkey\"\n/media/seg?u=https%3A%2F%2Fexample.com%2Fseg.ts\n");
        assert!(!p.contains("?u="));
        assert!(!p.contains("example.com"));
        assert_eq!(p.matches("?t=").count(), 2);
    }
    #[test]
    fn api_media_fields_get_opaque_references() {
        let mut g = MediaGrants::default();
        let mut v = serde_json::json!({"cover":"https://example.com/a.jpg","streams":{"main":"https://example.com/a.m3u8"}});
        g.rewrite_api(&mut v, "");
        assert!(v["cover"].as_str().unwrap().starts_with("/media/img?t="));
        assert!(v["streams"]["main"]
            .as_str()
            .unwrap()
            .starts_with("/media/hls?t="));
    }
}
