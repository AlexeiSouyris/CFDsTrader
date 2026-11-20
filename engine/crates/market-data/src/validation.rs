//! Validación de series de velas (spec §12).
//!
//! Reglas: cada problema se **reporta** con un error tipado y explicable; nada se corrige
//! en silencio. Quien llama decide qué hacer (descartar, alertar, frenar el instrumento).

use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_indicators::wilder_atr;
use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;

use crate::config::ValidationConfig;

/// Calendario de sesiones: qué velas deberían existir. Lo implementa el Issue #9;
/// la validación solo lo usa para detectar huecos dentro de la sesión.
pub trait SessionCalendar {
    /// Inicios de las velas que deberían existir en `[from, to)` para ese instrumento y timeframe.
    fn expected_starts(
        &self,
        instrument: &InstrumentId,
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Vec<DateTime<Utc>>;
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
    #[error("salto de {jump} = {multiple}× el ATR anterior ({atr}) sin corte de sesión")]
    Outlier {
        jump: Decimal,
        atr: Decimal,
        multiple: Decimal,
    },
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
            let missing =
                calendar.expected_starts(&c.instrument_id, c.timeframe, prev.end(), c.start);
            if let Some(&first_missing) = missing.first() {
                push(IssueKind::MissingCandles {
                    count: missing.len(),
                    first_missing,
                });
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
