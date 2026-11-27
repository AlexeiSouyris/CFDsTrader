---
modulo: ibkr
ruta: `engine/crates/ibkr` (crate `cfds-ibkr`)
lenguaje: Rust
estado: en curso (#534)
resumen: IBKR como segundo broker, solo cuenta paper y solo lectura (ADR 0017): conexión, cuenta, velas e histórico.
---

# ibkr

## Responsabilidad (única)

- IBKR como segundo broker, **solo cuenta paper y solo lectura** (ADR 0017, etapa 1): conexión a IB Gateway en esta máquina con el crate `ibapi` (=5.7.0, sin reconexión automática), cuentas de la sesión, resumen de la cuenta paper y velas **cerradas** de futuros micro (MNQ, MES) con su fuente
- como fuente de datos (#536), también futuro continuo, acciones o ETF (QQQ) y divisas de IDEALPRO, en velas de 1m, 15m, 1h y 1d, de un contrato vencido por mes (`ContractChoice`) y la primera vela disponible (`head_timestamp`), precios a `Decimal` al entrar, detrás de `MarketDataProvider`. Guardas con tests: puertos de sesión real rechazados antes del socket (7496, 4001 y `config/ibkr.toml`), sin `IBKR_PAPER_ACCOUNT_ID` no conecta, una cuenta que no es la paper desconecta y activa el kill switch (`cfds_core::KillSwitch`), desconexión o error de datos → kill switch con alerta (`cfds-obs`)
- sin código de órdenes. Ejemplos `ibkr_read` e `ibkr_probe` (medición de historia, `docs/analysis/2026-11-ibkr-medicion-historia.md`; con `--details SÍMBOLO BOLSA MONEDA`, la ficha de los vencimientos de cualquier futuro por `reqContractDetails`: multiplicador, tick, moneda, horario y vencimientos, sin precios ni cuentas, #672). Carga incremental de velas cerradas (`load`: ventanas, pausa y tope por corrida de `[load]`, sin continuo ni diarias) con el ejemplo `ibkr_load`, que guarda en `ibkr_bars` solo velas completas (#563: la última de cada respuesta espera `[load].data_delay_secs` por el atraso de los datos de IBKR, y una de 15m o 1h que no coincide con sus velas de 1m guardadas no se guarda; funciones puras `complete_prefix` y `check_against_minutes`)
- diagnóstico de solo lectura `ibkr_check_bars`
- `complete_prefix_by`, el mismo criterio para velas de cualquier tipo (lo usa el gateway, #575)
- aviso de carga trabada (#607): `health` cuenta, con una función pura, las corridas de `ibkr_load` en que IBKR no responde (tiempo agotado, conexión; una respuesta sin velas no cuenta) en un archivo de racha (`[load_health]`) y avisa una vez al empezar un corte y otra al volver, con un evento en `ops_events` (`ibkr_stalled` / `ibkr_recovered`) y una notificación de macOS
- ejemplo `ibkr_correct_bars` que agrega a `ibkr_bar_corrections` el OHLC de las velas de 15m y 1h mal guardadas con el agregado de sus 1m (solo los instrumentos pedidos, nunca acciones; sin `--run` solo muestra, #575). Vela en formación (#558, **solo visual**): `live` arma con funciones puras la vela en curso de 1m, 5m, 15m y 1h a partir de los precios por streaming (alineada en UTC, cierre por reloj a la hora de los datos, precios tardíos ignorados, cambio de contrato, el último operado previo a la suscripción no cuenta) y `live_feed` mantiene la suscripción de `[live]` (`client_id` 21, en vivo y sin permiso demorados, tipo y atraso informados; reabre el socket con espera creciente pasando otra vez por todas las guardas, sin credenciales; una guarda o un dato inválido lo detienen)
- ejemplo `ibkr_live` para medir
- ningún crate que decide depende de esto (test `live_isolation`)

## Estado

- en curso (#534, #536, #558)
