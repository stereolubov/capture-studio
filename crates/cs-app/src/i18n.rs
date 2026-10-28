//! Локализация на Fluent (§11.7): `assets/locales/{ru,en}/*.ftl`, никаких строк в коде.

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use std::borrow::Cow;
use unic_langid::LanguageIdentifier;

const RU: &str = include_str!("../../../assets/locales/ru/main.ftl");
const EN: &str = include_str!("../../../assets/locales/en/main.ftl");

/// Набор строк для одного языка. Отсутствующие ключи берутся из русского (основного) набора.
pub struct I18n {
    lang: String,
    primary: FluentBundle<FluentResource>,
    fallback: Option<FluentBundle<FluentResource>>,
}

fn bundle(lang: &str, src: &'static str) -> FluentBundle<FluentResource> {
    let id: LanguageIdentifier = lang.parse().unwrap_or_default();
    let mut b = FluentBundle::new(vec![id]);
    // Без изолирующих символов Unicode вокруг подстановок: они видны в egui как «квадратики».
    b.set_use_isolating(false);
    let res = match FluentResource::try_new(src.to_owned()) {
        Ok(r) => r,
        Err((r, errs)) => {
            tracing::error!("ошибки в {lang}.ftl: {errs:?}");
            r
        }
    };
    if let Err(errs) = b.add_resource(res) {
        tracing::error!("дубликаты ключей в {lang}.ftl: {errs:?}");
    }
    b
}

impl I18n {
    /// Создаёт набор для языка (`ru`, `en`); неизвестный язык → `ru`.
    pub fn new(lang: &str) -> Self {
        match lang {
            "en" => Self { lang: "en".into(), primary: bundle("en", EN), fallback: Some(bundle("ru", RU)) },
            _ => Self { lang: "ru".into(), primary: bundle("ru", RU), fallback: None },
        }
    }

    pub fn lang(&self) -> &str {
        &self.lang
    }

    fn format(&self, id: &str, args: Option<&FluentArgs<'_>>) -> String {
        for b in std::iter::once(&self.primary).chain(self.fallback.as_ref()) {
            if let Some(pattern) = b.get_message(id).and_then(|m| m.value()) {
                let mut errs = vec![];
                let s: Cow<'_, str> = b.format_pattern(pattern, args, &mut errs);
                if !errs.is_empty() {
                    tracing::warn!("ошибка форматирования «{id}»: {errs:?}");
                }
                return s.into_owned();
            }
        }
        tracing::warn!("нет строки локализации «{id}»");
        id.to_owned()
    }

    /// Строка без параметров.
    pub fn t(&self, id: &str) -> String {
        self.format(id, None)
    }

    /// Строка с параметрами: `t_args("scene-new-name", &[("n", 5.into())])`.
    pub fn t_args(&self, id: &str, args: &[(&str, FluentValue<'_>)]) -> String {
        let mut a = FluentArgs::new();
        for (k, v) in args {
            a.set(*k, v.clone());
        }
        self.format(id, Some(&a))
    }
}

/// Значение-строка для аргументов (числа форматируем сами, чтобы не было разделителей разрядов).
pub fn s(v: impl ToString) -> FluentValue<'static> {
    FluentValue::String(Cow::Owned(v.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn keys(src: &str) -> BTreeSet<String> {
        src.lines()
            .filter(|l| !l.starts_with('#') && !l.starts_with(' ') && l.contains(" = "))
            .filter_map(|l| l.split(" = ").next().map(str::to_owned))
            .collect()
    }

    #[test]
    fn locales_have_same_keys() {
        let ru = keys(RU);
        let en = keys(EN);
        let missing_en: Vec<_> = ru.difference(&en).collect();
        let missing_ru: Vec<_> = en.difference(&ru).collect();
        assert!(missing_en.is_empty(), "нет в en: {missing_en:?}");
        assert!(missing_ru.is_empty(), "нет в ru: {missing_ru:?}");
    }

    #[test]
    fn formatting() {
        let ru = I18n::new("ru");
        assert_eq!(
            ru.t_args("preview-placeholder", &[("w", s(1920)), ("h", s(1080)), ("fps", s(60))]),
            "превью сцены · 1920×1080 · 60 fps"
        );
        assert_eq!(I18n::new("en").t("btn-start-stream"), "Start streaming");
        assert_eq!(I18n::new("de").lang(), "ru");
    }
}
