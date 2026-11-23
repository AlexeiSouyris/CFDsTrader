//! Validación de series de velas (spec §12).
//!
//! Reglas: cada problema se **reporta** con un error tipado y explicable; nada se corrige
//! en silencio. Quien llama decide qué hacer (descartar, alertar, frenar el instrumento).

use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_indicators::wilder_atr;
use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;

use crate::config::ValidationConfig;

/// Calendario de sesiones: qué velas deberían existir. Lo implementa
/// [`crate::calendar::ExchangeCalendar`]; la validación lo usa para detectar huecos.
pub trait SessionCalendar {
    /// Inicios de las velas que deberían existir en `[from, to)` para ese instrumento y timeframe.
    /// `None` si el calendario no cubre ese instrumento o esas fechas: nunca se supone
    /// "mercado cerrado" por no saber.
    fn expected_starts(
        &self,
        instrument: &InstrumentId,
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Option<Vec<DateTime<Utc>>>;
}

/// Un problema encontrado en la vela `index` de la serie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub index: usize,
    pub start: DateTime<Utc>,
    pub kind: IssueKind,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IssueKind {
    #[error(
        "la serie mezcla instrumentos o timeframes: se esperaba {expected_instrument} {expected_timeframe:?}"
    )]
    MixedSeries {
        expected_instrument: InstrumentId,
        expected_timeframe: Timeframe,
    },
    #[error("precio {field} ≤ 0: {value}")]
    NonPositivePrice { field: &'static str, value: Decimal },
    #[error("volumen negativo: {0}")]
    NegativeVolume(Decimal),
    #[error("OHLC incoherente: O {open} H {high} L {low} C {close}")]
    InconsistentOhlc {
        open: Decimal,
        high: Decimal,
        low: Decimal,
        close: Decimal,
    },
    #[error("la vela no empieza en un múltiplo de {grid_minutes} minutos")]
    OffGrid { grid_minutes: u32 },
    #[error("vela duplicada (mismo inicio que la anterior)")]
    Duplicate,
    #[error("orden temporal roto: la vela anterior empieza en {previous_start}")]
    OutOfOrder { previous_start: DateTime<Utc> },
    #[error("se superpone con la vela anterior, que termina en {previous_end}")]
    Overlap { previous_end: DateTime<Utc> },
    #[error("faltan {count} velas dentro de la sesión; la primera empieza en {first_missing}")]
    MissingCandles {
        count: usize,
        first_missing: DateTime<Utc>,
    },
    #[error("el calendario no cubre el hueco desde {gap_from}: no se puede saber si faltan velas")]
    CalendarUnavailable { gap_from: DateTime<Utc> },
    #[error("salto de {jump} = {multiple}× el ATR anterior ({atr}) sin corte de sesión")]
    Outlier {
        jump: Decimal,
        atr: Decimal,
        multiple: Decimal,
    },
}

impl IssueKind {
    /// `true` si la vela misma es sospechosa y se descarta (spec §12). Los huecos
    /// (`MissingCandles`, `CalendarUnavailable`) no descartan la vela que llega después.
    pub fn discards_candle(&self) -> bool {
        !matches!(
            self,
            IssueKind::MissingCandles { .. } | IssueKind::CalendarUnavailable { .. }
        )
    }
}

impl Issue {
    /// Desde cuándo los datos dejan de ser confiables: el inicio del hueco o de la vela
    /// sospechosa.
    pub fn since(&self) -> DateTime<Utc> {
        match self.kind {
            IssueKind::MissingCandles { first_missing, .. } => first_missing,
            IssueKind::CalendarUnavailable { gap_from } => gap_from,
            _ => self.start,
        }
    }
}

/// Una serie ya revisada (spec §12): sin las velas sospechosas, con los problemas encontrados
/// y los momentos desde los que hubo un error de datos (ordenados, sin repetir).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedSeries {
    pub candles: Vec<Candle>,
    pub issues: Vec<Issue>,
    pub data_errors: Vec<DateTime<Utc>>,
    /// Saltos de más de `max_jump_atr_multiple` ATR que se sostuvieron (movimientos reales,
    /// #271): la vela se acepta y queda este registro, sin reiniciar el calentamiento.
    pub volatility: Vec<DateTime<Utc>>,
    /// Tramos `[desde, hasta)` de inicios de vela en los que no se decide nada porque un salto
    /// todavía no se confirmó (#271). `hasta = None`: sin confirmar al final de la serie.
    pub blocked: Vec<(DateTime<Utc>, Option<DateTime<Utc>>)>,
}

impl CheckedSeries {
    /// `true` si se puede decidir al cierre de la vela que empieza en `start` (no está dentro de
    /// un salto sin confirmar).
    pub fn decision_allowed(&self, start: DateTime<Utc>) -> bool {
        !self
            .blocked
            .iter()
            .any(|(from, to)| start >= *from && to.is_none_or(|t| start < t))
    }
}

/// Para cada vela de `candles`, `true` si se puede decidir a su cierre: no está dentro de un
/// salto sin confirmar de ninguna de las series revisadas (por ejemplo, la de 15m y la de 1h).
pub fn decision_flags(candles: &[Candle], checked: &[&CheckedSeries]) -> Vec<bool> {
    candles
        .iter()
        .map(|c| checked.iter().all(|s| s.decision_allowed(c.start)))
        .collect()
}

/// Qué fue un salto de más de `max_jump_atr_multiple` ATR (#271).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JumpOutcome {
    /// Algún cierre volvió cerca del nivel anterior: pico aislado, dato malo.
    Spike,
    /// El precio se sostuvo `confirm_bars` velas: movimiento real.
    Sustained,
    /// Todavía no hay `confirm_bars` velas después del salto.
    Pending,
}

/// Decide si el salto de la vela `i` fue un pico o un movimiento real, mirando solo hasta la
/// vela `i + confirm_bars` (la decisión se usa recién al cierre de esa vela).
///
/// ```text
/// base = cierre de la vela anterior; extremo = el máximo o el mínimo de la vela i, el que esté
/// más lejos de la base; salto = |extremo − base|
/// pico      ⇔ algún cierre de las velas i..=i+confirm_bars cumple |cierre − base| ≤ fracción × salto
/// sostenido ⇔ no es pico y la vela i+confirm_bars existe
/// ```
pub fn jump_outcome(candles: &[Candle], i: usize, cfg: &ValidationConfig) -> JumpOutcome {
    let (Some(prev), Some(c)) = (i.checked_sub(1).map(|p| &candles[p]), candles.get(i)) else {
        return JumpOutcome::Pending;
    };
    let base = prev.close;
    let size = (c.high - base).abs().max((base - c.low).abs());
    let end = (i + cfg.confirm_bars).min(candles.len() - 1);
    let back = candles[i..=end]
        .iter()
        .any(|k| (k.close - base).abs() <= cfg.return_fraction * size);
    if back {
        JumpOutcome::Spike
    } else if i + cfg.confirm_bars < candles.len() {
        JumpOutcome::Sustained
    } else {
        JumpOutcome::Pending
    }
}

/// Valida la serie, **descarta** las velas sospechosas y devuelve los momentos de error, para
/// que quien decide no abra operaciones nuevas hasta que vuelva a haber datos válidos (spec
/// §7.8 y §12). Nada se corrige en silencio: los problemas quedan en `issues` para alertar.
pub fn check_series(
    candles: &[Candle],
    calendar: &impl SessionCalendar,
    cfg: &ValidationConfig,
) -> CheckedSeries {
    let issues = validate_series(candles, calendar, cfg);
    // Un salto grande es un error solo si fue un pico (#271): si se sostuvo, o todavía no se
    // sabe, la vela queda y no hay error de datos. En los dos casos no se decide hasta confirmar.
    let mut volatility = Vec::new();
    let mut blocked = Vec::new();
    let mut real_jump = std::collections::BTreeSet::new();
    for i in issues
        .iter()
        .filter(|i| matches!(i.kind, IssueKind::Outlier { .. }))
    {
        let confirm = candles.get(i.index + cfg.confirm_bars).map(|c| c.start);
        blocked.push((i.start, confirm));
        match jump_outcome(candles, i.index, cfg) {
            JumpOutcome::Spike => {}
            JumpOutcome::Sustained => {
                volatility.push(i.start);
                real_jump.insert(i.index);
            }
            JumpOutcome::Pending => {
                real_jump.insert(i.index);
            }
        }
    }
    let counts =
        |i: &&Issue| !(matches!(i.kind, IssueKind::Outlier { .. }) && real_jump.contains(&i.index));
    let discarded: std::collections::BTreeSet<usize> = issues
        .iter()
        .filter(counts)
        .filter(|i| i.kind.discards_candle())
        .map(|i| i.index)
        .collect();
    let mut data_errors: Vec<DateTime<Utc>> =
        issues.iter().filter(counts).map(Issue::since).collect();
    data_errors.sort();
    data_errors.dedup();
    CheckedSeries {
        candles: candles
            .iter()
            .enumerate()
            .filter(|(i, _)| !discarded.contains(i))
            .map(|(_, c)| c.clone())
            .collect(),
        issues,
        data_errors,
        volatility,
        blocked,
    }
}

/// Valida una serie de velas de **un** instrumento y **un** timeframe, en orden.
/// Devuelve todos los problemas encontrados (vacío = la serie es válida).
pub fn validate_series(
    candles: &[Candle],
    calendar: &impl SessionCalendar,
    cfg: &ValidationConfig,
) -> Vec<Issue> {
    let Some(first) = candles.first() else {
        return Vec::new();
    };
    let atr = wilder_atr(candles, cfg.atr_period);
    let mut issues = Vec::new();

    for (i, c) in candles.iter().enumerate() {
        let mut push = |kind| {
            issues.push(Issue {
                index: i,
                start: c.start,
                kind,
            })
        };

        if c.instrument_id != first.instrument_id || c.timeframe != first.timeframe {
            push(IssueKind::MixedSeries {
                expected_instrument: first.instrument_id.clone(),
                expected_timeframe: first.timeframe,
            });
            continue;
        }
        for (field, value) in [
            ("open", c.open),
            ("high", c.high),
            ("low", c.low),
            ("close", c.close),
        ] {
            if value <= Decimal::ZERO {
                push(IssueKind::NonPositivePrice { field, value });
            }
        }
        if let Some(v) = c.volume.filter(|v| *v < Decimal::ZERO) {
            push(IssueKind::NegativeVolume(v));
        }
        if c.high < c.low || c.high < c.open.max(c.close) || c.low > c.open.min(c.close) {
            push(IssueKind::InconsistentOhlc {
                open: c.open,
                high: c.high,
                low: c.low,
                close: c.close,
            });
        }
        let grid_seconds = i64::from(cfg.grid_minutes) * 60;
        if c.start.timestamp() % grid_seconds != 0 || c.start.nanosecond() != 0 {
            push(IssueKind::OffGrid {
                grid_minutes: cfg.grid_minutes,
            });
        }

        let Some(prev) = i.checked_sub(1).map(|p| &candles[p]) else {
            continue;
        };
        if prev.instrument_id != c.instrument_id || prev.timeframe != c.timeframe {
            continue; // ya reportado como MixedSeries en la vela anterior
        }
        if c.start == prev.start {
            push(IssueKind::Duplicate);
        } else if c.start < prev.start {
            push(IssueKind::OutOfOrder {
                previous_start: prev.start,
            });
        } else if c.start < prev.end() {
            push(IssueKind::Overlap {
                previous_end: prev.end(),
            });
        } else if c.start > prev.end() {
            match calendar.expected_starts(&c.instrument_id, c.timeframe, prev.end(), c.start) {
                None => push(IssueKind::CalendarUnavailable {
                    gap_from: prev.end(),
                }),
                Some(missing) => {
                    if let Some(&first_missing) = missing.first() {
                        push(IssueKind::MissingCandles {
                            count: missing.len(),
                            first_missing,
                        });
                    }
                }
            }
        } else if let Some(prev_atr) = atr[i - 1].filter(|a| *a > Decimal::ZERO) {
            // Velas contiguas: un salto enorme sin corte de sesión es sospechoso.
            // Se usa el ATR de la vela ANTERIOR: la vela evaluada no se juzga con su propio dato.
            let jump = (c.open - prev.close).abs().max(c.high - c.low);
            if jump > cfg.max_jump_atr_multiple * prev_atr {
                push(IssueKind::Outlier {
                    jump,
                    atr: prev_atr,
                    multiple: (jump / prev_atr).round_dp(2),
                });
            }
        }
    }
    issues
}
