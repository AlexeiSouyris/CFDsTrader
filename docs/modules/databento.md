---
modulo: databento
ruta: `intelligence/cfds_intel/databento`
lenguaje: Python
estado: hecho
resumen: Importador del histórico de 1 minuto de futuros desde Databento (ADR 0011).
---

# databento

## Responsabilidad (única)

- Importador del histórico de 1 minuto de futuros (ES, NQ, YM de CME y FDAX de Eurex) desde Databento con su SDK oficial (ADR 0011, en propuesta): `--estimate` (solo metadata, gratis; frena si pasa `max_usd` de `config/databento.toml`), `--sample` (un día por símbolo, validado) y `--run` (solo en la máquina de datos, mes por mes y sin volver a pagar lo ya bajado; baja lo que falta antes y después de lo guardado, desde `start` = 2011-01-01, #369)
- `--check-month` cuenta los duplicados de un mes sin guardar, y `--only` limita a un símbolo. Guarda solo el publisher del libro central (en Eurex descarta las barras fuera de libro, `XEUR.EOBI.XOFF`, #254). Guarda las barras crudas en `futures_bars_1m` (`futures_bar.v1`) y los tramos en `futures_bar_loads`
- no toca `candles`. La agrupación a 15m, 1h y diarias es de Rust (`cfds_market_data::futures`, #248). Con `--daily` (#522) baja velas diarias `ohlcv-1d` de los futuros de investigación de `config/databento_daily.toml` (NKD, ZN, ZB, 6E, 6J, 6B, 6A, SI, CL y HG, `v.0` de CME, con la raíz como id interno y fuera de `config/instruments.toml`: no son operables) a `futures_bars_1d` (`futures_bar_1d.v1`), de a un año por pedido
- **la vela diaria de Databento va de medianoche a medianoche UTC, no de la hora de liquidación de la bolsa** (el borrador del programa 3, #513, pide la de liquidación: estas no la cumplen). Tope duro por corrida (`max_usd`, USD 1 en las diarias): se aplica a la estimación y, durante `--run`, a la suma del `get_cost` de cada pedido antes de hacerlo (`check_spend`). `futures_bar_loads` registra el esquema de cada tramo

## Estado

- hecho (#247)
- tramo anterior a lo guardado (#369)
- diarias de investigación (#522)
