---
modulo: terminal
ruta: `engine/crates/terminal` (crate `cfds-terminal`, binario `bolsa`)
lenguaje: Rust
estado: en curso (#541)
resumen: Ver la bolsa desde la terminal (binario `bolsa`), solo lectura y solo presentación.
---

# terminal

## Responsabilidad (única)

- Ver la bolsa desde la terminal (#541), **solo lectura y solo presentación**: watchlist (último, variación del día, hora), gráfico de velas en texto, panel del instrumento (OHLC y volumen de la última vela, rango y volumen del día) y barra de estado, con velas cerradas de IBKR paper por la sesión de `cfds-ibkr` (`client_id` propio). No calcula indicadores ni señales ni envía órdenes
- la única cuenta es último menos cierre de la sesión anterior. Pedidos con pausa mínima global (`config/terminal.toml`)
- su `client_id` tiene que ser distinto del de `[gateway]` y del de `[live]` de `config/ibkr.toml`. Pantalla con `crossterm` (=0.29.0)
- el armado es puro y probado sin red
- el día de sesión y la variación los toma de `cfds_market_data::session_day` (compartido con el gateway, #551)

## Estado

- en curso (#541)
