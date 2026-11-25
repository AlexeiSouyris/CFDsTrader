---
modulo: market-data
ruta: `engine/crates/market-data`
lenguaje: Rust
estado: hecho
resumen: Validación de velas, errores de datos, filtros de spread y de noticias, calendario de mercado y catálogo de instrumentos.
---

# market-data

## Responsabilidad (única)

- Validación de velas (#5), con descarte de velas sospechosas y momentos de error de datos (`check_series`, #89)
- un salto de más de 10 ATR es error solo si es un pico que vuelve, y si se sostiene es un movimiento real que se acepta (`jump_outcome`, `decision_flags`, #271)
- filtro de spread de la spec §7.3 (`spread_flags`, #90)
- filtro de noticias de la spec §7.2 (`news_flags`, #91), con la cobertura del calendario guardado (`NewsFilter`, `calendar_coverage`: qué velas tienen calendario, #243)
- barras de 1 minuto de futuros (`futures`, #248): velas de 15m y 1h cerradas, alineadas en UTC como las de IG, sin rellenar huecos y sin mezclar contratos, con la 1h de la regla de #150 (`futures_candles`, #259)
- qué futuro (dataset y símbolo) corresponde a cada instrumento según `config/databento.toml` (`FuturesFeeds`, #376)
- diarias del futuro con la misma ventana que las de IG (medianoche a medianoche de Madrid), solo días con sesión del CFD y sin mezclar contratos el día del vencimiento (`futures_daily`, #282), con el vencimiento por calendario o, para un continuo por volumen como el oro (`GC.v.0`), donde cambia el contrato de las velas (`contract_changes`, `RollDay::ContractChange`, #449)
- de qué velas sale la liquidez, IG CFD o futuros, con el instrumento derivado bajo el que se guarda (`CandleSources`, `config/candle_sources.toml`, #259) y la tasa de la base de cada futuro para la financiación del backtest (`FuturesBasisConfig`, `[futures.basis]`, #467) y diarias del futuro contra las de IG, con correlación de retornos sin vencimientos y base por contrato (`compare_daily`)
- validación de velas diarias de medianoche a medianoche local (`validate_daily`, #103)
- calendario de sesiones y feriados por bolsa (#9), con sesiones que cruzan la medianoche y el día hábil de cada instante (`Session::trading_day`; oro de CME Globex, ADR 0013, #418)
- horario de entradas y cierre de la semana (#64), con horas de entrada propias por bolsa (`[entries.hours]`, obligatorias para una sesión nocturna; las del oro, pendientes, #418)
- separación entrenamiento/validación/fuera de muestra (#16)
- catálogo de instrumentos y costos del broker con su fuente (#7), moneda de la cuenta y valor de cada tipo de referencia de la financiación (#76)
- trait `MarketDataProvider` y proveedor en memoria (#28). Vela de 1h armada con las cuatro de 15m de la spec §3 y su comparación con la del proveedor (`aggregate_h1`, `compare_h1`, #150; se mide con el ejemplo `h1_check` de `cfds-ingest`, que da por campo la diferencia máxima y el p99 y cuenta el efecto de los picos de bid y ask y el signo contrario como error de datos, `H1Comparison::stats`). Los proveedores reales viven en su propio crate (ej. `ig`) para no sumar dependencias de red acá
- día de sesión de una vela intradía y variación contra el cierre de una sesión anterior (`session_day`: `SessionOffsets` de `[session]` de `config/terminal.toml`, `session_date`, `session_start`, `change`, `change_pct`), único criterio que comparten la terminal `bolsa` y el gateway (#551)

## Estado

- validación (#5), calendario (#9), separación (#16), catálogo (#7), trait de proveedores (#28), horario de entradas (#64), errores de datos (#89), filtro de spread (#90), filtro de noticias (#91), vela de 1h desde 15m (#150)
