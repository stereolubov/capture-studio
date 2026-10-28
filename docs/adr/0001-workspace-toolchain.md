# ADR-0001: Workspace, toolchain и редакция языка

**Контекст.** ТЗ §3 требует зафиксированный stable Rust и edition 2021 или новее; §5.3 — структуру workspace.

**Решение.** Rust 1.99.1 в `rust-toolchain.toml` (цели x86_64 и i686 MSVC), edition 2024, resolver 3.
Общие версии зависимостей — `[workspace.dependencies]`, общие линты — `[workspace.lints]`
(`unsafe_op_in_unsafe_fn = deny`, `clippy::unwrap_used = warn`). Release: `lto = "fat"`,
`codegen-units = 1`, `strip`, `panic = "abort"`, `opt-level = 3`.

**Альтернативы.** Edition 2021 — нет let-chains, которые заметно упрощают код с FFI-проверками.

**Последствия.** Минимальная версия Rust 1.85+; фиксация toolchain делает сборки воспроизводимыми.
