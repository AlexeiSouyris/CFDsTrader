---
modulo: obs
ruta: `engine/crates/obs` (crate `cfds-obs`)
lenguaje: Rust
estado: hecho
resumen: Observabilidad: logs estructurados en JSON sin secretos y alertas (fase 9).
---

# obs

## Responsabilidad (única)

- Observabilidad (fase 9, #297): logs estructurados en líneas JSON con hora UTC (`Logger`), que **nunca** escriben secretos (campos con nombre de secreto, contraseña de URLs y valores registrados se tapan), y alertas (`Alerter`: destino de log y de Telegram por la Bot API oficial `sendMessage`, apagado sin `TELEGRAM_BOT_TOKEN` y `TELEGRAM_CHAT_ID`; disparadores kill switch, error de datos, desconexión y racha de rechazos; umbrales en `config/alerts.toml`)

## Estado

- logs y alertas (#297)
