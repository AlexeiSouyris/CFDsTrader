---
modulo: journal
ruta: `engine/crates/journal` (crate `cfds-journal`)
lenguaje: Rust
estado: hecho
resumen: Diario de propuestas y resultados, calibración de probabilidades y aporte de cada señal.
---

# journal

## Responsabilidad (única)

- Diario de propuestas y resultados (simulado y real)
- calibración de probabilidades y aporte de cada señal. Hoy: tipos de `proposal.v1` y `outcome.v1`, su armado desde el backtest y su escritura idempotente en PostgreSQL (`PgJournal`, tablas `proposals` y `outcomes`)
- probabilidad mostrada de la spec §13 (frecuencia de TP1 antes que el stop por gatillo y rango de puntaje, con intervalo de Wilson, solo con resultados anteriores al fuera de muestra; `probability`, `config/probability.toml`)
- registro de cada corrida de backtest (`backtest_run.v1`, tabla `backtest_runs` de solo agregar, idempotente por `run_id`): período, hash de la configuración, costos, intérprete y corte de las noticias y resumen de métricas (`run.rs`, `PgJournal::record_run`)
- reporte de la evaluación de la Fase 5 (`walk_forward_report.v1`, tabla `walk_forward_reports` de solo agregar, `PgJournal::record_walk_forward`, #298)
- variables de ML de cada propuesta (`ml_features.v1`, spec ML §3): ATR14 de 15m, tercio de la volatilidad del último día publicado hasta la señal (leído de `daily_vol_terciles`, el mismo que usan los análogos) y tercio de la sesión, todo al cierre de la vela de la señal, sin fila si falta alguna (`ml_features.rs`, `PgJournal::record_ml_features`)
- etiqueta de ML sin P/L (`ml_label.v1` y la corrida `ml_label_run.v1`, tablas `ml_labels` y `ml_label_runs` de solo agregar, migración 0037, #399): `y` de TP1 antes que el stop a precio medio y sin costos, con la hora en que se resolvió, y la propuesta y las variables de una propuesta contada (`ml_label.rs`, `PgJournal::record_ml_label` y `record_ml_label_run`)
- curva de cada corrida con el drawdown diario y comprar y mantener (`backtest_run_curve.v1`, tabla `backtest_run_curves` de solo agregar, `PgJournal::record_run_curve`, que escribe el ejemplo `backtest_run`) y el detalle de una corrida con sus operaciones (`backtest_run_detail.v1`, `PgJournal::run_detail`, #299)
- informe de aprendizaje continuo (`learning_report.v1`, `learning.rs`, `config/learning.toml`, #327: calibración con Wilson y muestra verificable, aporte por señal y propuestas de cambio pendientes de voto, sin aplicar nada; propuestas y votos guardados de solo agregar, `change_proposal.v1` y `change_vote.v1`, migración 0034, recuento de votos de la spec §1.2 en `votes.rs` con `config/votes.toml`, y el ejemplo `learning_report`)
- comparaciones paper vs backtest (`paper_comparison.v1`, tabla `paper_comparisons` de solo agregar, migración 0032, `PgJournal::record_paper_comparison`, #315; la última de cada instrumento con `latest_paper_comparisons`, #331; el último informe de aprendizaje con `latest_learning_report` y el estado de sus propuestas con `change_statuses`, #331)
- lectura de los experimentos de ML que guarda Python, con el instrumento de su corrida (`ml_experiment_entry.v1`, `PgJournal::ml_experiment_entries`, #426)

## Estado

- diseño (#13), contrato y armado (#60), PostgreSQL (#62), probabilidad §13 (#175), corridas de backtest (#200), variables de ML (#158)
