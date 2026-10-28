//! Разбор аргументов командной строки (без внешних зависимостей).

use std::path::PathBuf;

pub const USAGE: &str = "\
Использование: capture-studio [ПАРАМЕТРЫ]
  --mock-ui                 только UI на демо-данных (без захвата и сети)
  --verbose                 подробный лог (debug)
  --data-dir <папка>        папка данных вместо %APPDATA%\\CaptureStudio
  --screenshot <экран> <файл.png> [--theme dark|light] [--scale 1.0] [--lang ru|en]
                            экраны: 1a 1b 1c 1d 1e 1f (main settings mixer stats mini)";

/// Параметры скриншот-режима.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenshotArgs {
    pub screen: String,
    pub out: PathBuf,
    pub light: bool,
    pub scale: f32,
    pub lang: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Args {
    pub mock_ui: bool,
    pub verbose: bool,
    pub data_dir: Option<PathBuf>,
    pub screenshot: Option<ScreenshotArgs>,
}

impl Args {
    pub fn parse(it: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut a = Self::default();
        let mut theme: Option<String> = None;
        let mut scale = 1.0f32;
        let mut lang = "ru".to_owned();
        let mut shot: Option<(String, PathBuf)> = None;
        let mut it = it.into_iter();
        while let Some(arg) = it.next() {
            let mut value = |name: &str| it.next().ok_or_else(|| format!("{name}: нет значения"));
            match arg.as_str() {
                "--mock-ui" => a.mock_ui = true,
                "--verbose" => a.verbose = true,
                "--data-dir" => a.data_dir = Some(PathBuf::from(value("--data-dir")?)),
                "--screenshot" => {
                    let screen = value("--screenshot")?;
                    let out = value("--screenshot")?;
                    shot = Some((screen, PathBuf::from(out)));
                }
                "--theme" => theme = Some(value("--theme")?),
                "--scale" => {
                    scale = value("--scale")?.parse().map_err(|_| "--scale: ожидается число".to_owned())?;
                }
                "--lang" => lang = value("--lang")?,
                "-h" | "--help" => return Err("справка".into()),
                other => return Err(format!("неизвестный параметр: {other}")),
            }
        }
        if let Some((screen, out)) = shot {
            // 1b — светлая тема главного окна.
            let light = theme.as_deref() == Some("light") || screen == "1b";
            a.screenshot = Some(ScreenshotArgs { screen, out, light, scale: scale.clamp(0.5, 4.0), lang });
        }
        Ok(a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Args, String> {
        Args::parse(s.split_whitespace().map(str::to_owned))
    }

    #[test]
    fn flags() {
        let a = parse("--mock-ui --verbose").expect("ok");
        assert!(a.mock_ui && a.verbose);
        assert!(parse("--bogus").is_err());
    }

    #[test]
    fn screenshot() {
        let a = parse("--screenshot 1b out.png --scale 1.5").expect("ok");
        let s = a.screenshot.expect("shot");
        assert!(s.light);
        assert_eq!(s.scale, 1.5);
        assert_eq!(s.out, PathBuf::from("out.png"));
        assert!(parse("--screenshot 1a").is_err());
    }
}
