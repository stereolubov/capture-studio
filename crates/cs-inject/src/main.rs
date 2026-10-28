//! Вспомогательный инжектор хука (§6.3): отдельный процесс той же разрядности, что и игра.
//! Статус: M0 — заглушка; реализация (SetWindowsHookEx / CreateRemoteThread, выбор в ADR) — M4.

fn main() {
    eprintln!("cs-inject: внедрение хука появится на этапе M4 (протокол v{})", cs_hook_proto::PROTOCOL_VERSION);
    std::process::exit(2);
}
