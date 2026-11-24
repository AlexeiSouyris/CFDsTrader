//! Mapa de liquidez (spec §5.4.1, #231): precios donde probablemente hay stops. Solo
//! informativo en v1 (no suma al puntaje). Contrato: `contracts/liquidity_level.v1.schema.json`.
//!
//! ```text
//! EQH: ≥ min_touches pivotes altos de 15m confirmados en t, con índice en las últimas
//!      lookback_bars velas, que incluyen el pivote que se confirma en t y cuya diferencia
//!      máxima es ≤ tolerance_atr × ATR[t]. Nivel = el mayor. Existe desde el cierre de t.
//! PDH: máximo de las velas de una sesión de bolsa (config/sessions.toml). Existe desde su cierre.
//! PWH: máximo de las sesiones de una semana (lunes a viernes). Existe desde el cierre de la última.
//! session_high: máximo de las velas dentro de una ventana UTC de [sessions_utc]. Existe desde
//!      el fin de la ventana.
//! Los bajos (EQL, PDL, PWL, session_low) son el espejo.
//!
//! Estado de un alto, vela por vela desde que existe (las primeras max_age_bars velas):
//!   la primera vela con H > nivel: C ≤ nivel → barrido (swept); C > nivel → roto (broken).
//!   Si ninguna lo supera, queda intacto; si llega intacto a max_age_bars velas, vence
//!   (`expired_at` = cierre de la última). El estado no cambia nunca más.
//! ```
//!
//! Reglas:
//! - Solo velas cerradas: un período (sesión, semana o ventana) cuenta cuando la serie llega a
//!   su cierre. Si la serie empieza después de su apertura, el período está incompleto y no
//!   se usa (calentamiento).
//! - Calentamiento: nada existe sin ATR válido en la vela en la que nace; EQH y EQL además
//!   necesitan `lookback_bars` velas.
//! - Velas faltantes: se usan las que hay; las edades se cuentan en velas de la serie.
//! - Un día o una semana fuera de la cobertura del calendario de feriados no da nivel (nunca
//!   se supone si la bolsa abrió).
//! - EQH/EQL repetidos: si ya hay un nivel del mismo tipo y precio activo cuando nace otro,
//!   el nuevo no se agrega (es la misma liquidez).
//!
//! Números redondos (#497, [`round_number_levels`]; informe
//! `docs/analysis/2026-11-patrones-de-manipulacion.md` §4.5):
//!
//! ```text
//! pasos S del instrumento en [round_numbers] (por ejemplo US500: 50 y 100)
//! en cada vela cerrada t con ATR válido y para cada S:
//!   round_high = el múltiplo k × S más chico con k × S > C[t]
//!   round_low  = el múltiplo k × S más grande con k × S < C[t]
//!   (si C[t] es justo un múltiplo, ese número no es ni alto ni bajo: quedan R + S y R − S)
//! nace en el cierre de t, salvo que ya haya uno activo del mismo tipo y precio.
//! ```
//! El estado (intacto, barrido, roto o vencido) es el mismo de los otros niveles. Cuando un
//! round_high se rompe, el cierre queda por encima y ese mismo número nace como round_low.

use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;

use cfds_core::{
    Candle, InstrumentId, LiquidityLevel, LiquidityLevelKind as Kind, LiquidityLevelState as State,
    LiquiditySession,
};
use cfds_market_data::Session;
use cfds_patterns::{Pivot, PivotKind};
use chrono::{DateTime, Datelike, Days, NaiveDate, NaiveTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;

/// Ventana UTC de una sesión (inicio incluido, fin excluido, dentro del mismo día UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionWindow {
    pub session: LiquiditySession,
    pub start: NaiveTime,
    pub end: NaiveTime,
}

/// Parámetros del mapa de liquidez (`config/liquidity.toml`: `[equal]`, `[levels]` y
/// `[sessions_utc]`), validados al cargar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelsConfig {
    pub equal_min_touches: usize,
    pub equal_lookback_bars: usize,
    pub equal_tolerance_atr: Decimal,
    pub max_age_bars: usize,
    pub sessions: [SessionWindow; 3],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawEqual {
    min_touches: usize,
    lookback_bars: usize,
    tolerance_atr: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawLevels {
    max_age_bars: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawSessionsUtc {
    asia: RawWindow,
    london: RawWindow,
    ny: RawWindow,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWindow {
    start: String,
    end: String,
}

impl LevelsConfig {
    pub(crate) fn from_raw(e: RawEqual, l: RawLevels, s: RawSessionsUtc) -> Result<Self, String> {
        if e.min_touches < 2 {
            return Err("equal.min_touches tiene que ser ≥ 2".into());
        }
        if e.lookback_bars == 0 || l.max_age_bars == 0 {
            return Err("equal.lookback_bars y levels.max_age_bars tienen que ser > 0".into());
        }
        let tolerance = Decimal::from_str(&e.tolerance_atr)
            .map_err(|err| format!("equal.tolerance_atr: {err}"))?;
        if tolerance <= Decimal::ZERO {
            return Err("equal.tolerance_atr tiene que ser mayor que 0".into());
        }
        let window = |session: LiquiditySession, w: RawWindow| {
            let time = |t: &str| {
                NaiveTime::parse_from_str(t, "%H:%M")
                    .map_err(|_| format!("sessions_utc: {t:?} no es HH:MM"))
            };
            let (start, end) = (time(&w.start)?, time(&w.end)?);
            if start >= end {
                return Err(format!(
                    "sessions_utc: {session:?} empieza después de terminar"
                ));
            }
            Ok::<_, String>(SessionWindow {
                session,
                start,
                end,
            })
        };
        Ok(Self {
            equal_min_touches: e.min_touches,
            equal_lookback_bars: e.lookback_bars,
            equal_tolerance_atr: tolerance,
            max_age_bars: l.max_age_bars,
            sessions: [
                window(LiquiditySession::Asia, s.asia)?,
                window(LiquiditySession::London, s.london)?,
                window(LiquiditySession::Ny, s.ny)?,
            ],
        })
    }
}

/// Un nivel antes de evaluar su estado.
struct Candidate {
    kind: Kind,
    session: Option<LiquiditySession>,
    price: Decimal,
    exists_from: DateTime<Utc>,
}

/// Mapa de liquidez de una serie de velas **cerradas** de 15m de un instrumento, ordenada.
///
/// - `atr`: ATR de 15m alineado con `candles` (`order_blocks.atr_period`).
/// - `pivots`: `cfds_patterns::confirmed_pivots(candles, pivots.strength)`.
/// - `session`: sesión de bolsa del instrumento (`config/sessions.toml`).
///
/// Ordenado por `exists_from`. Función pura: con más velas, los niveles ya emitidos son los
/// mismos (solo puede cambiar el estado de uno intacto, con una vela posterior).
pub fn liquidity_levels(
    candles: &[Candle],
    atr: &[Option<Decimal>],
    pivots: &[Pivot],
    session: &Session,
    cfg: &LevelsConfig,
) -> Vec<LiquidityLevel> {
    let (Some(first), Some(last)) = (candles.first(), candles.last()) else {
        return Vec::new();
    };
    let mut candidates = equal_candidates(candles, atr, pivots, cfg);
    exchange_candidates(candles, session, &mut candidates);
    window_candidates(candles, cfg, &mut candidates);
    candidates.sort_by_key(|c| (c.exists_from, c.kind, c.session, c.price));
    debug_assert!(candidates.iter().all(|c| c.exists_from <= last.end()));

    let mut out: Vec<LiquidityLevel> = Vec::new();
    for c in candidates {
        // Vela en la que nace: la última que cerró hasta `exists_from`.
        let Some(born) = candles
            .partition_point(|k| k.end() <= c.exists_from)
            .checked_sub(1)
        else {
            continue;
        };
        if atr.get(born).copied().flatten().is_none() {
            continue; // calentamiento
        }
        let repeated = matches!(c.kind, Kind::Eqh | Kind::Eql)
            && out.iter().any(|o| {
                o.kind == c.kind && o.price == c.price && is_active_at(o, candles[born].end())
            });
        if repeated {
            continue;
        }
        let from = candles.partition_point(|k| k.start < c.exists_from);
        let (state, state_changed_at, expired_at) = evaluate(
            c.kind.is_high(),
            c.price,
            &candles[from..],
            cfg.max_age_bars,
        );
        out.push(LiquidityLevel {
            instrument_id: first.instrument_id.clone(),
            kind: c.kind,
            session: c.session,
            price: c.price,
            exists_from: c.exists_from,
            state,
            state_changed_at,
            expired_at,
        });
    }
    out
}

/// Sección `[round_numbers]` de `config/liquidity.toml` (#497): pasos de los números redondos
/// por instrumento, validados al cargar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundNumbersConfig {
    steps: BTreeMap<InstrumentId, Vec<Decimal>>,
}

#[derive(Deserialize)]
struct RawRoundFile {
    round_numbers: RawRoundNumbers,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRoundNumbers {
    steps: BTreeMap<String, Vec<String>>,
}

impl RoundNumbersConfig {
    /// Lee y valida `[round_numbers]`; ignora las demás secciones de `liquidity.toml`.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let raw = toml::from_str::<RawRoundFile>(text)
            .map_err(|e| e.to_string())?
            .round_numbers;
        let mut steps = BTreeMap::new();
        for (id, list) in raw.steps {
            let instrument: InstrumentId = id
                .parse()
                .map_err(|_| format!("round_numbers.steps: {id:?} no es un instrumento válido"))?;
            if list.is_empty() {
                return Err(format!(
                    "round_numbers.steps.{id}: hace falta al menos un paso"
                ));
            }
            let mut parsed = Vec::with_capacity(list.len());
            for text in &list {
                let step = Decimal::from_str(text)
                    .map_err(|e| format!("round_numbers.steps.{id}: {text:?}: {e}"))?;
                if step <= Decimal::ZERO {
                    return Err(format!(
                        "round_numbers.steps.{id}: {text:?} tiene que ser > 0"
                    ));
                }
                parsed.push(step);
            }
            parsed.sort();
            if parsed.windows(2).any(|w| w[0] == w[1]) {
                return Err(format!("round_numbers.steps.{id}: hay pasos repetidos"));
            }
            steps.insert(instrument, parsed);
        }
        Ok(Self { steps })
    }

    /// Pasos del instrumento, de menor a mayor; vacío si no tiene números redondos.
    pub fn steps_for(&self, instrument: &InstrumentId) -> &[Decimal] {
        self.steps.get(instrument).map_or(&[], Vec::as_slice)
    }
}

/// Niveles de número redondo (#497) de una serie de velas **cerradas** de 15m de un
/// instrumento, ordenada. Solo informativos, como el resto del mapa.
///
/// - `atr`: el mismo ATR de [`liquidity_levels`] (solo marca el calentamiento).
/// - `steps`: [`RoundNumbersConfig::steps_for`] del instrumento.
/// - `max_age_bars`: `[levels] max_age_bars`.
///
/// Ordenado por `exists_from`. Función pura y sin repintado: el nivel que nace en `t` depende
/// solo de las velas `0..=t`; su estado, de las posteriores, y cuando cambia queda fijo.
pub fn round_number_levels(
    candles: &[Candle],
    atr: &[Option<Decimal>],
    steps: &[Decimal],
    max_age_bars: usize,
) -> Vec<LiquidityLevel> {
    let mut out: Vec<LiquidityLevel> = Vec::new();
    // Último nivel emitido de cada tipo y precio: es el único que puede seguir activo.
    let mut latest: HashMap<(Kind, Decimal), usize> = HashMap::new();
    for (t, c) in candles.iter().enumerate() {
        if atr.get(t).copied().flatten().is_none() {
            continue; // calentamiento
        }
        let now = c.end();
        let mut born: Vec<(Kind, Decimal)> = steps
            .iter()
            .filter(|s| **s > Decimal::ZERO)
            .flat_map(|&s| {
                let (below, above) = round_neighbors(c.close, s);
                [(Kind::RoundHigh, above), (Kind::RoundLow, below)]
            })
            .collect();
        born.sort();
        born.dedup();
        for (kind, price) in born {
            if latest
                .get(&(kind, price))
                .is_some_and(|&i| is_active_at(&out[i], now))
            {
                continue; // la misma liquidez sigue activa
            }
            let (state, state_changed_at, expired_at) =
                evaluate(kind.is_high(), price, &candles[t + 1..], max_age_bars);
            latest.insert((kind, price), out.len());
            out.push(LiquidityLevel {
                instrument_id: c.instrument_id.clone(),
                kind,
                session: None,
                price,
                exists_from: now,
                state,
                state_changed_at,
                expired_at,
            });
        }
    }
    out
}

/// Múltiplos de `step` más cercanos estrictamente por debajo y por encima de `close`.
fn round_neighbors(close: Decimal, step: Decimal) -> (Decimal, Decimal) {
    let mut k = (close / step).floor();
    // La división puede redondear en el último dígito: se corrige para que k × S ≤ C < (k + 1) × S.
    while k * step > close {
        k -= Decimal::ONE;
    }
    while (k + Decimal::ONE) * step <= close {
        k += Decimal::ONE;
    }
    let below = if k * step == close {
        k - Decimal::ONE
    } else {
        k
    };
    (
        (below * step).normalize(),
        ((k + Decimal::ONE) * step).normalize(),
    )
}

/// `true` si en el instante `now` (el cierre de una vela) el nivel ya existe, no se barrió ni se
/// rompió y todavía no venció.
pub fn is_active_at(level: &LiquidityLevel, now: DateTime<Utc>) -> bool {
    level.exists_from <= now
        && level.state_changed_at.is_none_or(|t| t > now)
        && level.expired_at.is_none_or(|t| t > now)
}

/// Estado de un nivel evaluado sobre las velas posteriores a su nacimiento: el estado, la vela
/// que lo cambió y, si llegó intacto a `max_age_bars` velas, el cierre de la última (vencido).
fn evaluate(
    high: bool,
    price: Decimal,
    after: &[Candle],
    max_age_bars: usize,
) -> (State, Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
    for c in after.iter().take(max_age_bars) {
        let (crossed, back_inside) = if high {
            (c.high > price, c.close <= price)
        } else {
            (c.low < price, c.close >= price)
        };
        if crossed {
            let state = if back_inside {
                State::Swept
            } else {
                State::Broken
            };
            return (state, Some(c.end()), None);
        }
    }
    let expired_at = max_age_bars
        .checked_sub(1)
        .and_then(|i| after.get(i))
        .map(Candle::end);
    (State::Intact, None, expired_at)
}

fn equal_candidates(
    candles: &[Candle],
    atr: &[Option<Decimal>],
    pivots: &[Pivot],
    cfg: &LevelsConfig,
) -> Vec<Candidate> {
    let mut out = Vec::new();
    for (kind, pivot_kind) in [(Kind::Eqh, PivotKind::High), (Kind::Eql, PivotKind::Low)] {
        let same: Vec<&Pivot> = pivots.iter().filter(|p| p.kind == pivot_kind).collect();
        for p in &same {
            let t = p.confirmed_at;
            if t >= candles.len() || t + 1 < cfg.equal_lookback_bars {
                continue; // calentamiento: todavía no hay lookback_bars velas
            }
            let Some(atr_t) = atr.get(t).copied().flatten() else {
                continue;
            };
            let oldest = t + 1 - cfg.equal_lookback_bars;
            // `same` está ordenado por índice (y por confirmación, que es índice + N).
            let from = same.partition_point(|q| q.index < oldest);
            let mut prices: Vec<Decimal> = same[from..]
                .iter()
                .take_while(|q| q.confirmed_at <= t)
                .map(|q| q.price)
                .collect();
            prices.sort();
            let group = best_group(&prices, p.price, cfg.equal_tolerance_atr * atr_t);
            if group.len() >= cfg.equal_min_touches {
                let price = if kind.is_high() {
                    group[group.len() - 1]
                } else {
                    group[0]
                };
                out.push(Candidate {
                    kind,
                    session: None,
                    price,
                    exists_from: candles[t].end(),
                });
            }
        }
    }
    out
}

/// El grupo más grande de precios (ordenados) que contiene `anchor` y cuya diferencia máxima
/// es ≤ `width`. Empate: el de menor diferencia; después, el de precios más bajos.
fn best_group(sorted: &[Decimal], anchor: Decimal, width: Decimal) -> &[Decimal] {
    let mut best: &[Decimal] = &[];
    for (i, &low) in sorted.iter().enumerate() {
        if low > anchor || anchor - low > width {
            continue;
        }
        let end = i + sorted[i..].partition_point(|&x| x - low <= width);
        let group = &sorted[i..end];
        let spread = |g: &[Decimal]| g[g.len() - 1] - g[0];
        if group.len() > best.len() || (group.len() == best.len() && spread(group) < spread(best)) {
            best = group;
        }
    }
    best
}

/// Máximo y mínimo de las velas que empiezan en `[from, to)` y cierran hasta `to`.
fn extremes(
    candles: &[Candle],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Option<(Decimal, Decimal)> {
    let start = candles.partition_point(|c| c.start < from);
    candles[start..]
        .iter()
        .take_while(|c| c.start < to)
        .filter(|c| c.end() <= to)
        .fold(None, |acc, c| match acc {
            None => Some((c.high, c.low)),
            Some((h, l)) => Some((h.max(c.high), l.min(c.low))),
        })
}

fn push_pair(
    out: &mut Vec<Candidate>,
    (high, low): (Kind, Kind),
    session: Option<LiquiditySession>,
    (h, l): (Decimal, Decimal),
    exists_from: DateTime<Utc>,
) {
    for (kind, price) in [(high, h), (low, l)] {
        out.push(Candidate {
            kind,
            session,
            price,
            exists_from,
        });
    }
}

/// PDH/PDL de cada sesión de bolsa y PWH/PWL de cada semana, ya cerradas y completas.
fn exchange_candidates(candles: &[Candle], session: &Session, out: &mut Vec<Candidate>) {
    let (first_start, last_end) = (candles[0].start, candles[candles.len() - 1].end());
    let local = |t: DateTime<Utc>| t.with_timezone(&session.tz).date_naive();
    let (first_day, last_day) = (local(first_start), local(last_end));
    let complete =
        |open: DateTime<Utc>, close: DateTime<Utc>| open >= first_start && close <= last_end;
    let mut monday: NaiveDate =
        first_day - Days::new(first_day.weekday().num_days_from_monday().into());
    while monday <= last_day {
        let mut week: Vec<(DateTime<Utc>, DateTime<Utc>)> = Vec::new();
        let mut covered = true;
        for d in (0..5).filter_map(|k| monday.checked_add_days(Days::new(k))) {
            match session.hours(d) {
                None => covered = false,
                Some(None) => {}
                Some(Some((open, close))) => {
                    week.push((open, close));
                    if complete(open, close)
                        && let Some(hl) = extremes(candles, open, close)
                    {
                        push_pair(out, (Kind::Pdh, Kind::Pdl), None, hl, close);
                    }
                }
            }
        }
        if covered
            && let (Some(&(open, _)), Some(&(_, close))) = (week.first(), week.last())
            && complete(open, close)
        {
            let hl = week
                .iter()
                .filter_map(|&(o, c)| extremes(candles, o, c))
                .reduce(|(h1, l1), (h2, l2)| (h1.max(h2), l1.min(l2)));
            if let Some(hl) = hl {
                push_pair(out, (Kind::Pwh, Kind::Pwl), None, hl, close);
            }
        }
        let Some(next) = monday.checked_add_days(Days::new(7)) else {
            break;
        };
        monday = next;
    }
}

/// Máximo y mínimo de cada ventana UTC de `[sessions_utc]`, ya cerrada y completa.
fn window_candidates(candles: &[Candle], cfg: &LevelsConfig, out: &mut Vec<Candidate>) {
    let (first_start, last_end) = (candles[0].start, candles[candles.len() - 1].end());
    let mut day = first_start.date_naive();
    while day <= last_end.date_naive() {
        for w in cfg.sessions {
            let (from, to) = (
                day.and_time(w.start).and_utc(),
                day.and_time(w.end).and_utc(),
            );
            if from >= first_start
                && to <= last_end
                && let Some(hl) = extremes(candles, from, to)
            {
                push_pair(
                    out,
                    (Kind::SessionHigh, Kind::SessionLow),
                    Some(w.session),
                    hl,
                    to,
                );
            }
        }
        let Some(next) = day.succ_opt() else { break };
        day = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LiquidityConfig;
    use rust_decimal::dec;

    const REAL: &str = include_str!("../../../../config/liquidity.toml");

    fn hm(s: &str) -> NaiveTime {
        NaiveTime::parse_from_str(s, "%H:%M").unwrap()
    }

    #[test]
    fn the_repository_config_matches_the_spec() {
        let c = LiquidityConfig::from_toml(REAL).unwrap().levels;
        assert_eq!(
            c,
            LevelsConfig {
                equal_min_touches: 2,
                equal_lookback_bars: 200,
                equal_tolerance_atr: dec!(0.10),
                max_age_bars: 500,
                sessions: [
                    SessionWindow {
                        session: LiquiditySession::Asia,
                        start: hm("00:00"),
                        end: hm("07:00"),
                    },
                    SessionWindow {
                        session: LiquiditySession::London,
                        start: hm("07:00"),
                        end: hm("13:30"),
                    },
                    SessionWindow {
                        session: LiquiditySession::Ny,
                        start: hm("13:30"),
                        end: hm("20:00"),
                    },
                ],
            }
        );
    }

    #[test]
    fn rejects_invalid_level_values() {
        for bad in [
            REAL.replace("min_touches = 2", "min_touches = 1"),
            REAL.replace("lookback_bars = 200", "lookback_bars = 0"),
            REAL.replace(r#""0.10""#, r#""0""#),
            REAL.replace(r#""0.10""#, r#""diez""#),
            REAL.replace("max_age_bars = 500", "max_age_bars = 0"),
            REAL.replace(r#"end = "07:00""#, r#"end = "00:00""#),
            REAL.replace(r#"start = "13:30""#, r#"start = "1:30pm""#),
            REAL.replace("ny = ", "nyc = "),
            REAL.replace("[sessions_utc]", "[sesiones_utc]"),
        ] {
            assert!(LiquidityConfig::from_toml(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn best_group_takes_the_largest_tightest_cluster_with_the_anchor() {
        let p = [dec!(99), dec!(100), dec!(100.4), dec!(101), dec!(103)];
        assert_eq!(best_group(&p, dec!(100.4), dec!(1)), &p[1..4]);
        assert_eq!(best_group(&p, dec!(103), dec!(1)), &p[4..]);
        // Con poca tolerancia, solo entra el vecino cercano.
        assert_eq!(best_group(&p, dec!(100), dec!(0.5)), &p[1..3]);
    }
}
