//! Alineación multi-timeframe (spec §4: la tendencia sale de la última vela de 1h **cerrada**).

use cfds_core::Candle;

/// Para cada vela de `lower` (15m), el índice de la última vela de `higher` (1h) que ya cerró
/// cuando cierra la de 15m: `higher[j].end() ≤ lower[t].end()`. `None` si todavía no cerró
/// ninguna. Las dos series tienen que estar ordenadas por tiempo (lo garantiza la validación).
pub fn align_closed(higher: &[Candle], lower: &[Candle]) -> Vec<Option<usize>> {
    let mut j = 0;
    lower
        .iter()
        .map(|c| {
            while j < higher.len() && higher[j].end() <= c.end() {
                j += 1;
            }
            j.checked_sub(1)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cfds_core::Timeframe;
    use chrono::{DateTime, Utc};
    use rust_decimal::dec;

    fn at(tf: Timeframe, start: &str) -> Candle {
        let p = dec!(100);
        Candle {
            instrument_id: "US100".parse().unwrap(),
            timeframe: tf,
            start: start.parse::<DateTime<Utc>>().unwrap(),
            open: p,
            high: p,
            low: p,
            close: p,
            volume: None,
        }
    }

    #[test]
    fn a_1h_candle_counts_from_the_15m_candle_that_closes_with_it() {
        let h1 = [
            at(Timeframe::H1, "2026-11-19T14:00:00Z"),
            at(Timeframe::H1, "2026-11-19T15:00:00Z"),
        ];
        let m15: Vec<Candle> = ["13:45", "14:00", "14:30", "14:45", "15:00", "15:45"]
            .iter()
            .map(|t| at(Timeframe::M15, &format!("2026-11-19T{t}:00Z")))
            .collect();
        // 13:45 cierra a las 14:00: todavía no cerró ninguna de 1h.
        // 14:45 cierra a las 15:00, junto con la de 14:00 de 1h: ya cuenta.
        // 15:45 cierra a las 16:00, junto con la de 15:00 de 1h.
        assert_eq!(
            align_closed(&h1, &m15),
            vec![None, None, None, Some(0), Some(0), Some(1)]
        );
    }

    #[test]
    fn empty_series() {
        assert!(align_closed(&[], &[]).is_empty());
        let m15 = [at(Timeframe::M15, "2026-11-19T14:00:00Z")];
        assert_eq!(align_closed(&[], &m15), vec![None]);
    }
}
