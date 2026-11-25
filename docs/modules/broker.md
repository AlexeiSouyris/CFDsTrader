---
modulo: broker
ruta: `engine/crates/broker` (crate `cfds-broker`)
lenguaje: Rust
estado: hecho
resumen: Interfaz de broker con envío idempotente; hoy el broker simulado (`PaperBroker`) con reconciliación y swap.
---

# broker

## Responsabilidad (única)

- Interfaz de broker (`Broker`: vista previa, envío idempotente por `order_id`, posiciones y cuenta) con dos implementaciones: `PaperBroker`, simulado contra las velas guardadas (un contrato en otra moneda que la cuenta, como el oro en USD, se convierte con el EUR/USD vigente en cada momento, `AccountFx`; sin tipo a la hora de la señal, rechazo `missing_fx_rate`, ADR 0013, #418) (fill en la apertura de t+1 con spread, slippage y comisión de `cfds-backtest`, tamaño solo de `cfds-risk::check`, salidas con `cfds-backtest::simulate` desde el precio ejecutado y salida a la apertura si la entrada salta el stop, igual que el backtest (#319), financiación nocturna noche por noche con `funding_cutoffs` y `funding_over` y la tasa vigente en cada corte, kill switch; la €STR y el tipo de cambio de `fx_rates` el gateway los vuelve a leer en cada operación para usar lo publicado después del arranque, cada valor desde su publicación, `update_funding_rates` y `update_account_fx`, #439), y `LiveBroker`, **imposible de construir** (dinero real bloqueado, CLAUDE.md §5 y fase 11). Mensajes `order.v1`, `order_report.v1`, `ticket_verdict.v1`, `position.v1` y `paper_account.v1`
- reconciliación al arrancar (`restore_and_reconcile`: reenvía el registro de órdenes, compara informes y posiciones, y ante cualquier diferencia activa el kill switch; las velas del broker empiezan `[paper] candles_days` antes de la **primera** orden del registro, `start_candles_at`, para que la reconstrucción tenga las velas de todas las órdenes y el mismo ATR, #438; con el registro vacío, el gateway completa hacia atrás las velas que le faltan a una primera orden anterior al arranque, `prepend_candles` y `cover_first_order`, #481)
- kill switch guardado (`paper_kill_switch`, migración 0042, #441): cada activación queda con su punto del registro y vuelve al reconstruir (`restore_with_kill_switch`), así sobrevive a un reinicio
- solo lo apaga una liberación que agrega una persona después de la votación de la spec §1.2
- registro de órdenes en PostgreSQL (`paper_orders` y `paper_order_reports`, solo agregar, migración 0028: `submit_and_record`, arranque con `restore_from_store` y reconciliación periódica cada `[reconcile] every_minutes` con `reconcile_periodically`)
- órdenes pendientes (#471): una orden cuya vela del fill todavía no llegó (la manual, con `created_at` de ahora) no se rechaza con `no_price` ni entra al registro: si el riesgo la aceptaría queda pendiente (`order_report.v1` con `pending`) y se decide cuando llega esa vela (`settle_pending`, `settle_and_record` en cada operación del gateway), igual que la decide la reconstrucción
- las velas se recortan antes de la primera orden que **entra** al registro (`anchor_candles_before_first_order`)
- pendientes guardadas (#479, `paper_pending_orders` y `paper_pending_outcomes`, solo agregar, migración 0044): la pendiente se guarda al llegar (`submit_and_record`; si no se puede guardar, no queda pendiente y se puede reenviar), entra al registro y queda `settled` en la misma transacción, y al arrancar se retoman sobre el broker reconstruido (con su kill switch) sin evaluarlas otra vez (`restore_pending_from_store`, `PaperBroker::restore_pending`), así se deciden igual que sin reinicio
- la de un instrumento que ya no se opera queda `dropped` con el motivo
- eventos para logs y alertas (`BrokerEvent`: orden recibida, pendiente, decisión del riesgo, informe y kill switch; `take_events`, el broker no escribe logs) y parada segura (`flush_journal`: guarda lo que falte del registro y verifica que no queden órdenes huérfanas, #297)
- operaciones valuadas para compararlas con el backtest (`trade_outcomes`, #315)
- puntos de control (`checkpoint`, `paper_broker_checkpoints`, migración 0043, #463): después de cada reconciliación periódica limpia se sella lo que ya no cambia (operaciones cerradas antes del día de la vela cerrada más vieja, con su resultado), se sueltan las velas anteriores con el estado del ATR de Wilder, y la reconstrucción arranca del último punto que verifica (hash encadenado, registro y config) con exactamente el mismo resultado
- solo agregar
- config en `config/broker.toml`

## Estado

- broker simulado, reconciliación, swap y registro en PostgreSQL (#278)
