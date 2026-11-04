//! Reloadable service catalog. Unverified entries are hidden from the service picker.
use serde::Deserialize;
use std::{io, path::Path};
#[derive(Clone, Deserialize)]
pub struct Server {
    pub name: String,
    pub url: String,
}
#[derive(Clone, Deserialize)]
pub struct Service {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub servers: Vec<Server>,
    pub max_video_kbps: Option<u32>,
    pub max_resolution: Option<[u32; 2]>,
    pub max_fps: Option<u32>,
    pub keyframe_interval_s: Option<u32>,
}
#[derive(Deserialize)]
pub struct Catalog {
    pub schema_version: u32,
    pub service: Vec<Service>,
}
impl Catalog {
    pub fn load(path: Option<&Path>) -> io::Result<Self> {
        let text = match path {
            Some(path) if path.exists() => std::fs::read_to_string(path)?,
            _ => include_str!("../../../assets/services.toml").to_owned(),
        };
        let catalog: Self =
            toml::from_str(&text).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid services.toml"))?;
        if catalog.schema_version != 1 || catalog.service.is_empty() || catalog.service.len() > 128 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "unsupported service catalog"));
        }
        let mut ids = std::collections::HashSet::new();
        if catalog.service.iter().any(|s| !ids.insert(&s.id) || s.id.is_empty()) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "duplicate or empty service ID"));
        }
        Ok(catalog)
    }
    pub fn installed() -> io::Result<Self> {
        let path = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.join("assets/services.toml")));
        Self::load(path.as_deref())
    }
    pub fn find(&self, id: &str) -> Option<&Service> {
        self.service.iter().find(|s| s.id == id)
    }
    pub fn server(&self, id: &str, configured: &str) -> io::Result<String> {
        let s = self
            .find(id)
            .filter(|s| s.verified)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "stream service is not verified"))?;
        if configured != "auto" {
            return Ok(configured.into());
        }
        s.servers
            .first()
            .map(|s| s.url.clone())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "enter a stream server URL"))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_and_unverified_services() {
        let c = Catalog::load(None).expect("catalog");
        assert!(c.server("twitch", "auto").expect("twitch").starts_with("rtmp://"));
        assert!(c.server("youtube", "auto").expect("youtube").starts_with("rtmps://"));
        assert!(c.server("kick", "rtmp://localhost/live").is_err());
        assert!(c.server("custom", "auto").is_err());
    }
}
