//! Velas diarias (#103). Contrato: `contracts/daily_candle.v1.schema.json`.
//!
//! El día va de medianoche a medianoche en la zona horaria de los precios. En un cambio de
//! horario dura 23 o 25 h, así que inicio y fin son explícitos: una vela diaria no es un
//! [`crate::Timeframe`] de duración fija. Se usa recién desde `end` (CLAUDE.md §5b).

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::candle::utc_z;
use crate::instrument::InstrumentId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyCandle {
    pub instrument_id: InstrumentId,
    /// Fecha local del día.
    pub day: NaiveDate,
    /// Medianoche local de `day`, en UTC.
    #[serde(with = "utc_z")]
    pub start: DateTime<Utc>,
    /// Medianoche local del día siguiente, en UTC.
    #[serde(with = "utc_z")]
    pub end: DateTime<Utc>,
    #[serde(with = "crate::decimal_str")]
    pub open: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub high: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub low: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub close: Decimal,
    #[serde(with = "crate::decimal_str::option")]
    pub volume: Option<Decimal>,
}

impl DailyCandle {
    /// `true` si en `now` el día ya terminó y la vela se puede usar.
    pub fn is_closed_at(&self, now: DateTime<Utc>) -> bool {
        now >= self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    #[test]
    fn a_daily_candle_is_usable_only_from_its_real_end() {
        // 25-10-2026, cambio de horario en Europa: el día local dura 25 h.
        let c = DailyCandle {
            instrument_id: "DE40".parse().unwrap(),
            day: "2026-10-25".parse().unwrap(),
            start: "2026-10-24T22:00:00Z".parse().unwrap(),
            end: "2026-10-25T23:00:00Z".parse().unwrap(),
            open: dec!(24000),
            high: dec!(24100),
            low: dec!(23900),
            close: dec!(24050),
            volume: None,
        };
        assert!(
            !c.is_closed_at("2026-10-25T22:00:00Z".parse().unwrap()),
            "a las 24 h todavía no"
        );
        assert!(!c.is_closed_at("2026-10-25T22:59:59Z".parse().unwrap()));
        assert!(c.is_closed_at("2026-10-25T23:00:00Z".parse().unwrap()));
    }
}
