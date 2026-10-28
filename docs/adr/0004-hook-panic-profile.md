# ADR-0004: Стратегия паник для хука

**Контекст.** §18.1: `panic = "abort"` для приложения, но не для `cs-hook` — там паники ловятся
`catch_unwind` на FFI-границах, чтобы не уронить игру. В Cargo стратегия паник задаётся на профиль,
а не на крейт.

**Решение.** Профиль `release` — `abort`; отдельный профиль `release-hook` (`inherits = "release"`,
`panic = "unwind"`) для сборки `cs-hook` и `cs-inject`: `cargo build --profile release-hook -p cs-hook
--target i686-pc-windows-msvc` (и x86_64). В `xtask dist` (M4/M5) эти сборки будут отдельными шагами.

**Альтернативы.** Отдельный workspace для хука — дублирование настроек и lock-файла.

**Последствия.** CI собирает хук обеих разрядностей уже сейчас (проверка, что `cs-hook-proto` совместим).
