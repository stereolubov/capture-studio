//! DLL захвата игры (§6.3), внедряется в процесс игры. Собирается под x86 и x64
//! с профилем `release-hook` (unwind: паники ловятся `catch_unwind` на FFI-границах).
//!
//! Статус: M0 — пустая библиотека, проверяющая сборку под обе разрядности. Перехват
//! Present / SwapBuffers / vkQueuePresentKHR — этап M4.

pub use cs_hook_proto::PROTOCOL_VERSION;
