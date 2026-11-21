# Fase 4 — Matriz de trazabilidad: reglas de la spec → tests

Criterio de aprobación de la Fase 4 (`docs/ROADMAP.md`): **cada regla de la spec tiene al menos un test.**

Hay una fila por regla de `docs/spec/fase-1-especificacion.md` (§2, §3, §4, §5–§8 y §12). Cada test se nombra como `` `ruta/al/archivo.rs::nombre_del_test` ``, con la ruta relativa a `engine/`. El test `engine/crates/backtest/tests/spec_matrix.rs` lee este archivo y falla si un test nombrado no existe, así la matriz no queda vieja si se renombra algo. También falla si una fila no nombra ningún test ni ningún Issue.

Estados:
- ✅ cubierta;
- ⏳ con Issue abierto en la Fase 4 (#86);
- ➡️ de otra fase, sin test todavía a propósito.

Las reglas de cortos son el **espejo exacto** de las de largos (spec §5). Cada familia tiene al menos un test del espejo, que se nombra en su fila.

## §2 Mercado y horario de entradas

| Regla | Tests | Estado |
|---|---|---|
| Sesión en la hora local de la bolsa (el horario de verano se resuelve solo) | `crates/market-data/tests/calendar.rs::new_york_follows_us_daylight_saving_time`, `crates/market-data/tests/calendar.rs::the_weeks_where_us_and_europe_change_time_on_different_dates` | ✅ |
| Sin entradas en los primeros 15 min de la sesión | `crates/market-data/tests/trading_window.rs::no_entries_in_the_first_15_minutes` | ✅ |
| Sin entradas nuevas en los últimos 30 min de la sesión del viernes | `crates/market-data/tests/trading_window.rs::no_new_entries_in_the_last_30_minutes_of_the_week` | ✅ |
| El backtest no decide fuera de la ventana de entradas | `crates/backtest/tests/runner.rs::no_decision_outside_the_entry_window` | ✅ |

## §3 Datos y velas

| Regla | Tests | Estado |
|---|---|---|
| Solo velas cerradas (la de 15m se usa desde su cierre) | `crates/core/src/candle.rs::candle_is_not_usable_until_it_closes`, `crates/core/src/candle.rs::a_15m_candle_closes_exactly_15_minutes_after_its_start` | ✅ |
| La vela de 1h existe recién al cerrar la hora | `crates/core/src/candle.rs::a_1h_candle_is_only_usable_after_the_full_hour`, `crates/strategy/src/mtf.rs::a_1h_candle_counts_from_the_15m_candle_that_closes_with_it` | ✅ |
| La señal se calcula al cierre de `t` con datos hasta `t` | `crates/strategy/tests/evidence.rs::the_signal_at_t_only_uses_data_available_at_the_close_of_t` | ✅ |
| La entrada simulada es la apertura de `t+1` más slippage | `crates/backtest/tests/runner.rs::one_trade_matches_the_hand_calculation_in_every_field` | ✅ |
| Calentamiento: 200 velas de 15m y 60 de 1h **cerradas** antes de emitir señales | `crates/strategy/tests/evidence.rs::the_spec_warmup_needs_200_candles_of_15m`, `crates/strategy/tests/evidence.rs::the_spec_warmup_needs_60_closed_candles_of_1h`, `crates/strategy/tests/evidence.rs::no_evidence_during_warmup` | ✅ |

## §4 Indicadores

| Regla | Tests | Estado |
|---|---|---|
| SMA50: media aritmética de 50 cierres | `crates/indicators/src/moving_average.rs::sma_matches_hand_calculation` | ✅ |
| ATR14: True Range con el cierre anterior y suavizado de Wilder | `crates/indicators/src/atr.rs::true_range_uses_previous_close_on_gaps`, `crates/indicators/src/atr.rs::wilder_atr_matches_hand_calculation` | ✅ |
| Calentamiento y sin repintado de los indicadores | `crates/indicators/src/atr.rs::warmup_returns_none_until_enough_candles`, `crates/indicators/src/atr.rs::values_never_change_when_new_candles_arrive`, `crates/indicators/src/moving_average.rs::values_never_change_when_new_candles_arrive` | ✅ |
| Pivote: estrictamente mayor que 3 velas a cada lado | `crates/patterns/src/pivots.rs::finds_a_high_pivot_and_confirms_it_n_candles_later`, `crates/patterns/src/pivots.rs::equal_highs_are_not_a_pivot`, `crates/patterns/src/pivots.rs::finds_low_pivots_too` | ✅ |
| Pivote: se confirma en `i+3` y antes no existe | `crates/patterns/src/pivots.rs::a_pivot_does_not_exist_before_its_confirmation_candle`, `crates/patterns/src/pivots.rs::pivots_never_change_when_new_candles_arrive` | ✅ |
| Tendencia de 1h: alcista, bajista o neutral | `crates/indicators/src/trend.rs::rising_prices_are_an_uptrend_after_warmup`, `crates/indicators/src/trend.rs::falling_prices_are_a_downtrend`, `crates/indicators/src/trend.rs::price_and_slope_disagreeing_is_neutral` | ✅ |

## §5.1 Velas (gatillo)

| Regla | Tests | Estado |
|---|---|---|
| Martillo: caída previa estricta `C[t−1] < C[t−6]` | `crates/patterns/src/candles.rs::hammer_prior_fall_is_strict`, `crates/patterns/src/candles.rs::the_same_shape_after_a_rise_is_not_a_hammer` | ✅ |
| Martillo: `mecha_inf ≥ 2 × cuerpo` | `crates/patterns/src/candles.rs::hammer_lower_wick_of_exactly_twice_the_body` | ✅ |
| Martillo: `mecha_sup ≤ 0,3 × rango` | `crates/patterns/src/candles.rs::hammer_upper_wick_of_at_most_30_percent_of_the_range` | ✅ |
| Martillo: `cuerpo ≥ 0,05 × rango` | `crates/patterns/src/candles.rs::hammer_body_of_at_least_5_percent_of_the_range`, `crates/patterns/src/candles.rs::a_flat_candle_is_never_a_pattern` | ✅ |
| Martillo: `rango ≥ 0,5 × ATR` (y sin ATR no hay martillo) | `crates/patterns/src/candles.rs::hammer_range_of_at_least_half_the_atr`, `crates/patterns/src/candles.rs::without_atr_yet_there_is_no_hammer` | ✅ |
| Estrella fugaz (espejo) | `crates/patterns/src/candles.rs::detects_a_shooting_star_after_a_rise` | ✅ |
| Envolvente: vela anterior bajista y actual alcista | `crates/patterns/src/candles.rs::engulfing_needs_a_bearish_candle_before`, `crates/patterns/src/candles.rs::detects_engulfing_both_ways` | ✅ |
| Envolvente: `O[t] ≤ C[t−1]` y `C[t] ≥ O[t−1]` | `crates/patterns/src/candles.rs::engulfing_open_and_close_edges_are_inclusive`, `crates/patterns/src/candles.rs::a_candle_that_does_not_cover_the_previous_body_is_not_engulfing` | ✅ |
| Envolvente: `cuerpo[t] > cuerpo[t−1]` | `crates/patterns/src/candles.rs::engulfing_needs_a_strictly_bigger_body` | ✅ |
| Umbrales: un umbral más laxo nunca detecta menos | `crates/patterns/tests/sensitivity.rs::candle_thresholds_are_monotone` | ✅ |

## §5.2 Doble suelo / doble techo (gatillo)

| Regla | Tests | Estado |
|---|---|---|
| Dos pivotes bajos **confirmados** de 15m | `crates/patterns/tests/chart_patterns.rs::never_uses_a_pivot_before_its_confirmation`, `crates/patterns/tests/chart_patterns.rs::passing_future_pivots_does_not_leak_into_the_past` | ✅ |
| `10 ≤ velas entre S1 y S2 ≤ 60` | `crates/patterns/tests/chart_patterns.rs::double_bottom_takes_10_to_60_candles_between_the_bottoms_inclusive`, `crates/patterns/tests/chart_patterns.rs::rejects_bottoms_too_close_together` | ✅ |
| `\|S1 − S2\| / S1 ≤ 0,3 %` | `crates/patterns/tests/sensitivity.rs::double_difference_tolerance_is_inclusive_at_the_edge`, `crates/patterns/tests/chart_patterns.rs::rejects_pairs_too_different_too_shallow_or_too_old` | ✅ |
| `neckline − min(S1,S2) ≥ 1 × ATR` | `crates/patterns/tests/sensitivity.rs::double_min_height_is_inclusive_at_the_edge` | ✅ |
| Gatillo: `C[t] > neckline` y `C[t−1] ≤ neckline` | `crates/patterns/tests/chart_patterns.rs::detects_a_double_bottom_on_the_neckline_break`, `crates/patterns/tests/chart_patterns.rs::only_the_breaking_candle_triggers` | ✅ |
| Gatillo hasta 20 velas después de confirmar S2 | `crates/patterns/tests/chart_patterns.rs::double_bottom_trigger_lasts_20_candles_after_confirming_the_second_bottom` | ✅ |
| Doble techo (espejo) | `crates/patterns/tests/chart_patterns.rs::the_mirror_is_a_double_top` | ✅ |

## §5.3 Soporte / resistencia (gatillo)

| Regla | Tests | Estado |
|---|---|---|
| Nivel: ≥ 3 pivotes de las últimas 200 velas, dentro de ±0,15 % del promedio | `crates/patterns/tests/chart_patterns.rs::groups_three_close_lows_into_a_support`, `crates/patterns/tests/chart_patterns.rs::two_touches_or_old_or_unconfirmed_pivots_are_not_a_level`, `crates/patterns/tests/sensitivity.rs::level_tolerance_is_inclusive_at_the_edge` | ✅ |
| Rechazo: `L[t] ≤ nivel × (1 + 0,15 %)` y `C[t] > nivel` | `crates/patterns/tests/chart_patterns.rs::support_rejection_edges` | ✅ |
| Rechazo de resistencia (espejo) | `crates/patterns/tests/chart_patterns.rs::detects_rejections_of_support_and_resistance` | ✅ |

## §5.4 Liquidez ICT/SMC

| Regla | Tests | Estado |
|---|---|---|
| Sweep: pincha el último pivote bajo **confirmado** de 1h y cierra adentro | `crates/liquidity/tests/liquidity.rs::sweep_of_a_confirmed_1h_low`, `crates/liquidity/tests/liquidity.rs::a_1h_pivot_not_yet_confirmed_is_invisible`, `crates/liquidity/tests/liquidity.rs::piercing_without_closing_back_inside_is_not_a_sweep`, `crates/liquidity/tests/liquidity.rs::an_unconfirmed_newer_pivot_does_not_hide_the_confirmed_one` | ✅ |
| Sweep de un máximo (espejo) | `crates/liquidity/tests/liquidity.rs::sweep_of_a_1h_high_is_the_mirror` | ✅ |
| FVG: zona `[H[i−2], L[i]]`, toque sin llenar de las últimas 50 velas | `crates/liquidity/tests/liquidity.rs::touch_of_an_open_bullish_fvg`, `crates/liquidity/tests/liquidity.rs::a_filled_fvg_no_longer_counts`, `crates/liquidity/tests/liquidity.rs::an_old_fvg_falls_out_of_the_lookback`, `crates/liquidity/tests/liquidity.rs::the_fvg_result_at_t_does_not_depend_on_later_candles`, `crates/liquidity/tests/liquidity.rs::bearish_fvg_is_the_mirror` | ✅ |
| Order block: última vela contraria antes de un impulso de cuerpo ≥ 1,5 × ATR que hace BOS, sin invalidar | `crates/liquidity/tests/liquidity.rs::order_block_is_the_last_opposite_candle_before_the_impulse`, `crates/liquidity/tests/liquidity.rs::a_weak_impulse_does_not_make_an_order_block`, `crates/liquidity/tests/liquidity.rs::a_close_beyond_the_order_block_invalidates_it`, `crates/liquidity/tests/liquidity.rs::bearish_order_block_is_the_mirror` | ✅ |
| BOS (con la tendencia de 1h) y CHoCH (en contra), CHoCH reciente durante 12 velas | `crates/liquidity/tests/liquidity.rs::break_with_the_1h_trend_is_a_bos_and_against_it_a_choch`, `crates/liquidity/tests/liquidity.rs::a_break_before_the_pivot_is_confirmed_does_not_count`, `crates/liquidity/tests/liquidity.rs::choch_is_recent_for_12_candles`, `crates/liquidity/tests/liquidity.rs::bearish_break_is_the_mirror` | ✅ |
| Sin repintado de rupturas y order blocks | `crates/liquidity/tests/liquidity.rs::breaks_and_order_blocks_do_not_repaint` | ✅ |

## §6 Puntaje de confluencia y entrada

| Regla | Tests | Estado |
|---|---|---|
| Sin ningún gatillo en `t` no hay señal | `crates/strategy/tests/strategy.rs::without_a_trigger_there_is_no_signal` | ✅ |
| Tendencia 1h: +2 a favor, −3 en contra, 0 neutral | `crates/strategy/tests/strategy.rs::trend_with_adds_2_against_subtracts_3_neutral_adds_0` | ✅ |
| Lado correcto de la SMA50 de 15m: +1 | `crates/strategy/tests/strategy.rs::right_side_of_the_sma_adds_1`, `crates/strategy/tests/evidence.rs::sma_side_and_fvg_context_match_their_definitions` | ✅ |
| Pesos de los gatillos: vela +1, doble +2, rechazo +1, sweep +2; cada tipo cuenta una vez | `crates/strategy/tests/strategy.rs::each_trigger_has_its_weight`, `crates/strategy/tests/strategy.rs::a_trigger_kind_counts_once` | ✅ |
| FVG u order block +1, CHoCH reciente +1 | `crates/strategy/tests/strategy.rs::fvg_or_order_block_and_recent_choch_add_1_each` | ✅ |
| Ajuste por noticias de −2 a +2 (el cálculo es de la 4b) | `crates/strategy/tests/strategy.rs::news_adjust_between_minus_2_and_plus_2`, `crates/strategy/tests/evidence.rs::news_out_of_range_is_an_error_not_a_silent_clamp` | ✅ |
| Propuesta con puntaje ≥ 5 | `crates/strategy/tests/strategy.rs::a_score_of_5_is_a_candidate_and_4_is_not` | ✅ |
| El desglose explica el total | `crates/strategy/tests/strategy.rs::the_breakdown_explains_the_total` | ✅ |
| Largo y corto a la vez: gana el mayor puntaje y el empate no opera | `crates/backtest/tests/runner.rs::the_higher_score_wins_and_a_tie_means_no_trade` | ✅ |
| Cada gatillo y contexto sale de su definición | `crates/strategy/tests/evidence.rs::candle_triggers_follow_the_patterns_of_their_side`, `crates/strategy/tests/evidence.rs::double_and_sweep_triggers_use_their_structure`, `crates/strategy/tests/evidence.rs::level_sweep_and_choch_evidence_match_their_definitions` | ✅ |
| La propuesta se registra con su desglose (contrato `proposal.v1`) | `crates/journal/tests/journal.rs::a_backtest_trade_becomes_its_proposal_and_outcome` | ✅ |
| Vence a los 10 min o al alejarse 0,25R (registro real) | Fase 8: no hay registro real todavía | ➡️ |
| Probabilidad estimada y análogos en la propuesta | Fase 4b (§13) | ➡️ |

## §7 Filtros: no se abre si...

| Regla | Tests | Estado |
|---|---|---|
| 1. Fuera de horario | `crates/backtest/tests/runner.rs::no_decision_outside_the_entry_window`, `crates/market-data/tests/trading_window.rs::weekends_and_holidays_have_no_entries` | ✅ |
| 2. Noticia de alto impacto del instrumento o de su país a ±15 min de la entrada, solo desde su publicación | `crates/market-data/tests/news_filter.rs::a_high_impact_event_within_15_minutes_blocks_both_edges_included`, `crates/market-data/tests/news_filter.rs::only_high_impact_blocks`, `crates/market-data/tests/news_filter.rs::only_events_of_the_instrument_or_its_countries_count`, `crates/market-data/tests/news_filter.rs::an_event_counts_only_once_published` | ✅ |
| 2b. Fuente del calendario económico para aplicar el filtro 2 en el backtest y en vivo | Fase 4b, con su ADR (spec §9). Mientras tanto, `backtest_run` avisa que el filtro no tiene datos | ➡️ |
| 3. Spread > 2 × su promedio de 20 días a esa hora (hora local de la bolsa); sin dato o sin historia suficiente no se abre | `crates/market-data/tests/spread_filter.rs::more_than_twice_the_average_blocks_and_exactly_twice_does_not`, `crates/market-data/tests/spread_filter.rs::only_the_last_20_days_count`, `crates/market-data/tests/spread_filter.rs::other_hours_do_not_count`, `crates/market-data/tests/spread_filter.rs::the_hour_is_local_to_the_exchange_across_daylight_saving`, `crates/market-data/tests/spread_filter.rs::without_enough_history_or_current_spread_it_does_not_open`, `crates/market-data/tests/spread_filter.rs::later_spreads_never_change_an_earlier_decision` | ✅ |
| 4. Ya hay una posición en el instrumento | `crates/risk/tests/risk.rs::one_position_per_instrument`, `crates/backtest/tests/runner.rs::with_one_position_at_most_trades_never_overlap` | ✅ |
| 5. Ya hay 5 posiciones abiertas | `crates/risk/tests/risk.rs::at_most_5_open_positions` | ✅ |
| 6. Pérdida máxima del día o kill switch activo | `crates/risk/tests/risk.rs::daily_loss_of_3_percent_stops_new_entries`, `crates/risk/tests/risk.rs::kill_switch_blocks_everything`, `crates/backtest/tests/runner.rs::the_daily_loss_limit_blocks_the_rest_of_the_day` | ✅ |
| 7. Distancia al stop menor a 0,5 × ATR o mayor a 3 × ATR | `crates/risk/tests/risk.rs::stop_distance_between_half_and_three_atr` | ✅ |
| 8. Error de datos: no se abre hasta volver a tener el calentamiento completo después del error | `crates/strategy/tests/evidence.rs::a_data_error_restarts_the_warmup`, `crates/backtest/tests/runner.rs::after_a_data_error_nothing_opens_until_the_warmup_is_complete`, `crates/backtest/tests/export.rs::the_export_shows_no_scores_while_a_data_error_blocks_the_instrument` | ✅ |

## §8 Stop, tamaño, apalancamiento y salidas

| Regla | Tests | Estado |
|---|---|---|
| Stop = estructura − 0,5 × ATR (largo) | `crates/strategy/tests/strategy.rs::long_stop_is_the_lowest_structure_minus_half_atr` | ✅ |
| Stop del corto (espejo) | `crates/strategy/tests/strategy.rs::short_stop_is_the_highest_structure_plus_half_atr` | ✅ |
| Estructura por gatillo: vela (y la anterior si es envolvente), `min(S1,S2)`, mecha del sweep, nivel | `crates/strategy/tests/evidence.rs::candle_triggers_follow_the_patterns_of_their_side`, `crates/strategy/tests/evidence.rs::double_and_sweep_triggers_use_their_structure`, `crates/strategy/tests/evidence.rs::level_sweep_and_choch_evidence_match_their_definitions` | ✅ |
| Sin gatillo, ATR no positivo o stop del lado equivocado: sin riesgo definido | `crates/strategy/tests/strategy.rs::no_stop_without_triggers_or_with_a_non_positive_atr`, `crates/strategy/tests/strategy.rs::a_stop_on_the_wrong_side_has_no_risk_defined` | ✅ |
| Riesgo del 1 % del capital, tamaño = riesgo / R, en la moneda del instrumento | `crates/risk/tests/risk.rs::risks_one_percent_of_equity`, `crates/risk/tests/risk.rs::short_is_the_mirror`, `crates/risk/tests/risk.rs::sizes_in_the_instrument_currency` | ✅ |
| Tope de apalancamiento: se reduce el tamaño | `crates/risk/tests/risk.rs::the_leverage_cap_reduces_the_size` | ✅ |
| Margen requerido mayor que el libre: no se abre | `crates/risk/tests/risk.rs::margin_must_fit_in_free_margin`, `crates/backtest/tests/runner.rs::margin_is_shared_between_positions` | ✅ |
| Tamaño menor al mínimo del broker: no se abre | `crates/risk/tests/risk.rs::below_the_broker_minimum_does_not_open`, `crates/risk/tests/risk.rs::an_unconfirmed_broker_minimum_does_not_open` | ✅ |
| TP1 en +2R: cierra el 50 % y el stop pasa a la entrada | `crates/backtest/tests/exits.rs::tp1_closes_half_and_moves_the_stop_to_the_entry`, `crates/backtest/tests/exits.rs::back_to_the_entry_in_the_tp1_candle_closes_the_rest_there` | ✅ |
| Trailing: `max(stop, máximo desde TP1 − 2 × ATR)` | `crates/backtest/tests/exits.rs::the_trailing_stop_follows_the_high_minus_two_atr`, `crates/backtest/tests/exits.rs::the_trailing_stop_never_moves_back`, `crates/backtest/tests/exits.rs::the_trailing_uses_the_highest_high_since_tp1_not_the_last_one` | ✅ |
| Salida por tiempo: 32 velas sin tocar +1R | `crates/backtest/tests/exits.rs::without_1r_after_32_candles_it_closes_at_the_close`, `crates/backtest/tests/exits.rs::touching_1r_avoids_the_time_exit` | ✅ |
| Viernes: se cierra todo en la última vela antes del cierre de la sesión | `crates/backtest/tests/exits.rs::forced_close_on_friday_or_session_end`, `crates/backtest/tests/exits.rs::the_stop_comes_before_a_forced_close`, `crates/market-data/tests/trading_window.rs::flags_for_a_friday_session` | ✅ |
| Viernes feriado: aplica el último día hábil | `crates/market-data/tests/trading_window.rs::before_a_friday_holiday_the_thursday_closes_the_week` | ✅ |
| Stop y TP en la misma vela: primero el stop | `crates/backtest/tests/exits.rs::stop_and_tp_in_the_same_candle_assume_the_stop` | ✅ |
| Cortos (espejo de las salidas) | `crates/backtest/tests/runner.rs::short_trades_mirror_the_long_ones` | ✅ |

## §12 Casos borde

| Regla | Tests | Estado |
|---|---|---|
| Faltan datos: el hueco marca un error de datos y no hay entradas nuevas en el instrumento | `crates/market-data/tests/corrupted_data.rs::detects_missing_candles_inside_the_session`, `crates/market-data/tests/corrupted_data.rs::a_gap_marks_the_first_missing_candle_and_keeps_the_next_one`, `crates/strategy/tests/evidence.rs::a_data_error_restarts_the_warmup` | ✅ |
| Datos tardíos o error de conexión en vivo: kill switch | `crates/ig/src/live.rs::overdue_only_after_give_up`. Alertar y bloquear el instrumento en vivo es de la Fase 8 | ➡️ |
| Gap que salta el stop o el TP: se ejecuta a la apertura | `crates/backtest/tests/exits.rs::a_gap_through_the_stop_fills_at_the_open`, `crates/backtest/tests/exits.rs::a_gap_through_tp1_fills_at_the_open`, `crates/backtest/tests/runner.rs::a_gap_through_the_stop_at_the_entry_exits_at_that_open` | ✅ |
| Feriado: el instrumento no opera | `crates/market-data/tests/calendar.rs::holidays_and_weekends_have_no_session`, `crates/market-data/tests/trading_window.rs::weekends_and_holidays_have_no_entries` | ✅ |
| Halt: no hay entradas nuevas | En el backtest, un halt sin velas dentro de la sesión es un hueco y aplica la regla de datos faltantes. Detectarlo en vivo con el estado del mercado del broker es de la Fase 8 | ➡️ |
| Ejecución parcial (registro real) | Fase 8 | ➡️ |
| Datos sospechosos: se detectan | `crates/market-data/tests/corrupted_data.rs::detects_zero_or_negative_prices`, `crates/market-data/tests/corrupted_data.rs::detects_a_huge_jump_between_contiguous_candles`, `crates/market-data/tests/corrupted_data.rs::detects_duplicates`, `crates/market-data/tests/corrupted_data.rs::detects_broken_time_order`, `crates/market-data/tests/corrupted_data.rs::a_big_jump_after_a_session_break_is_not_an_outlier` | ✅ |
| Datos sospechosos: se descarta la vela y no hay entradas nuevas en el instrumento | `crates/market-data/tests/corrupted_data.rs::suspicious_candles_are_discarded_and_mark_a_data_error`, `crates/market-data/tests/corrupted_data.rs::a_candle_with_several_problems_is_one_data_error`, `crates/backtest/tests/runner.rs::after_a_data_error_nothing_opens_until_the_warmup_is_complete` | ✅ |

## Pendientes

Ninguna regla de la Fase 4 queda sin test.

Fuera de la Fase 4, a propósito:
- el vencimiento de la propuesta, la ejecución parcial y la detección en vivo de halts y datos tardíos (fase 8);
- la probabilidad y los análogos (4b).
