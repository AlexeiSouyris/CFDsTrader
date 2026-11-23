//! Estadísticas de liquidez en PostgreSQL (#231, `infra/migrations/0021_liquidity_stats.sql`):
//! eventos de FVG (`fvg_event.v1`), gaps de apertura (`opening_gap.v1`), desequilibrio
//! (`imbalance.v1`) y agregados por corrida (`liquidity_stats.v1`).
//!
//! Todo es de solo agregar y se guarda en **una transacción** por llamada:
//! - una zona o evento ya guardado e idéntico cuenta como "ya estaba";
//! - un evento nuevo de una zona ya guardada (por ejemplo el llenado) se agrega una sola vez;
//! - otro valor para algo ya guardado, o un evento guardado que el cálculo ya no da, es
//!   repintado: [`StorageError::LiquidityStatsConflict`] y no se guarda nada del lote.
//!
//! Las lecturas devuelven lo que se sabía en `as_of` (sin look-ahead) y validan cada fila contra
//! el contrato al armarla.

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use cfds_core::{FvgEvent, Imbalance, InstrumentId, LiquidityStats, OpeningGap, StatsTimeframe};
use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use rust_decimal::Decimal;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};

use crate::{PgStore, StorageError};

/// Resultado de guardar zonas (FVG o gaps) o valores de desequilibrio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatsReport {
    /// Filas nuevas (zonas o valores).
    pub new_rows: usize,
    /// Eventos nuevos (de zonas nuevas o ya guardadas).
    pub new_events: usize,
    /// Ya estaban, idénticos y sin eventos nuevos.
    pub unchanged: usize,
}

/// Resultado de guardar los agregados de una corrida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatsRunReport {
    /// Grupos (instrumento, tipo, timeframe) guardados con la corrida nueva.
    pub new_groups: usize,
    /// Grupos iguales a su última corrida guardada: no se guardan otra vez.
    pub unchanged_groups: usize,
}

fn text<T: serde::Serialize>(v: T) -> Result<String, StorageError> {
    match serde_json::to_value(v)? {
        Value::String(s) => Ok(s),
        other => Err(StorageError::CorruptRow(format!(
            "se esperaba texto: {other}"
        ))),
    }
}

fn utc(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

fn int(n: u32) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

fn conflict(what: &str, instrument: &InstrumentId, at: DateTime<Utc>) -> StorageError {
    StorageError::LiquidityStatsConflict {
        what: what.into(),
        instrument: instrument.to_string(),
        at,
    }
}

/// Un evento de FVG tal como se guarda: (evento, horizonte, hora, velas, retorno).
type FvgEventRow = (
    String,
    Option<i32>,
    DateTime<Utc>,
    Option<i32>,
    Option<Decimal>,
);

fn fvg_event_rows(e: &FvgEvent) -> Result<Vec<FvgEventRow>, StorageError> {
    let mut rows: Vec<FvgEventRow> = [
        ("touch", e.touch),
        ("mid", e.mid),
        ("fill", e.fill),
        ("expiry", e.expiry),
    ]
    .into_iter()
    .filter_map(|(name, p)| {
        p.map(|p| (name.to_string(), None, p.at, Some(int(p.bars.get())), None))
    })
    .collect();
    for r in &e.reactions {
        match (r.return_atr, r.at) {
            (Some(v), Some(at)) => {
                rows.push((
                    "reaction".into(),
                    Some(int(r.bars.get())),
                    at,
                    None,
                    Some(v),
                ));
            }
            (None, _) => {}
            (Some(_), None) => {
                return Err(StorageError::CorruptRow(
                    "una reacción con valor tiene que traer su hora (at)".into(),
                ));
            }
        }
    }
    Ok(rows)
}

/// Inserta la zona si no está; si está, exige que sea idéntica. Devuelve (id, nueva).
async fn zone_id(
    tx: &mut Transaction<'_, Postgres>,
    e: &FvgEvent,
    timeframe: &str,
    direction: &str,
) -> Result<(i64, bool), StorageError> {
    let inserted: Option<i64> = sqlx::query_scalar(
        "INSERT INTO fvg_zones (instrument_id, timeframe, direction, created_at, bottom, top, atr,
             size_atr)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT ON CONSTRAINT fvg_zones_key DO NOTHING
         RETURNING id",
    )
    .bind(e.instrument_id.as_str())
    .bind(timeframe)
    .bind(direction)
    .bind(e.created_at)
    .bind(e.bottom)
    .bind(e.top)
    .bind(e.atr)
    .bind(e.size_atr)
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(id) = inserted {
        return Ok((id, true));
    }
    let (id, bottom, top, atr, size): (i64, Decimal, Decimal, Decimal, Decimal) = sqlx::query_as(
        "SELECT id, bottom, top, atr, size_atr FROM fvg_zones
         WHERE instrument_id = $1 AND timeframe = $2 AND direction = $3 AND created_at = $4",
    )
    .bind(e.instrument_id.as_str())
    .bind(timeframe)
    .bind(direction)
    .bind(e.created_at)
    .fetch_one(&mut **tx)
    .await?;
    if (bottom, top, atr, size) != (e.bottom, e.top, e.atr, e.size_atr) {
        return Err(conflict("FVG", &e.instrument_id, e.created_at));
    }
    Ok((id, false))
}

/// Fila de `opening_gap_events`: (evento, día, sesiones, hora).
type GapEventRow = (String, NaiveDate, i32, DateTime<Utc>);

fn gap_event_row(g: &OpeningGap) -> Result<Option<GapEventRow>, StorageError> {
    let event = match (g.fill, g.expiry) {
        (Some(s), _) => Some(("fill", s)),
        (None, Some(s)) => Some(("expiry", s)),
        (None, None) => None,
    };
    event
        .map(|(name, s)| {
            let at = s.at.ok_or_else(|| {
                StorageError::CorruptRow("el evento de un gap tiene que traer su hora (at)".into())
            })?;
            Ok((name.to_string(), s.day, int(s.sessions), at))
        })
        .transpose()
}

impl PgStore {
    /// Guarda eventos de FVG (`fvg_event.v1`): la zona y cada evento conocido.
    pub async fn insert_fvg_events(
        &self,
        events: &[FvgEvent],
    ) -> Result<StatsReport, StorageError> {
        let mut tx = self.pool().begin().await?;
        let mut report = StatsReport::default();
        for e in events {
            let (timeframe, direction) = (text(e.timeframe)?, text(e.direction)?);
            let (id, new) = zone_id(&mut tx, e, &timeframe, &direction).await?;
            let computed = fvg_event_rows(e)?;
            let stored: Vec<FvgEventRow> = sqlx::query_as(
                "SELECT event, horizon_bars, at, bars, return_atr FROM fvg_zone_events
                 WHERE zone_id = $1",
            )
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
            if stored.iter().any(|s| !computed.contains(s)) {
                return Err(conflict("evento de FVG", &e.instrument_id, e.created_at));
            }
            let mut added = 0;
            for (event, horizon, at, bars, ret) in computed.iter().filter(|c| !stored.contains(c)) {
                // Mismo evento con otros valores: la clave choca y es repintado.
                let key_taken = stored.iter().any(|s| &s.0 == event && &s.1 == horizon);
                if key_taken {
                    return Err(conflict("evento de FVG", &e.instrument_id, e.created_at));
                }
                sqlx::query(
                    "INSERT INTO fvg_zone_events (zone_id, event, horizon_bars, at, bars, return_atr)
                     VALUES ($1, $2, $3, $4, $5, $6)",
                )
                .bind(id)
                .bind(event)
                .bind(horizon)
                .bind(at)
                .bind(bars)
                .bind(ret)
                .execute(&mut *tx)
                .await?;
                added += 1;
            }
            report.new_events += added;
            match (new, added) {
                (true, _) => report.new_rows += 1,
                (false, 0) => report.unchanged += 1,
                (false, _) => {}
            }
        }
        tx.commit().await?;
        Ok(report)
    }

    /// FVG de un instrumento y timeframe creados en `[from, as_of]`, **tal como se conocían en
    /// `as_of`** (un evento posterior no se ve). `reaction_bars` son los horizontes de
    /// `[fvg_stats]`: un horizonte sin reacción conocida va con `return_atr = null`.
    pub async fn fvg_events(
        &self,
        instrument: &InstrumentId,
        timeframe: StatsTimeframe,
        from: DateTime<Utc>,
        as_of: DateTime<Utc>,
        reaction_bars: &[NonZeroU32],
    ) -> Result<Vec<FvgEvent>, StorageError> {
        type Zone = (
            i64,
            String,
            DateTime<Utc>,
            Decimal,
            Decimal,
            Decimal,
            Decimal,
        );
        let tf = text(timeframe)?;
        let zones: Vec<Zone> = sqlx::query_as(
            "SELECT id, direction, created_at, bottom, top, atr, size_atr FROM fvg_zones
             WHERE instrument_id = $1 AND timeframe = $2 AND created_at >= $3 AND created_at <= $4
             ORDER BY created_at, direction",
        )
        .bind(instrument.as_str())
        .bind(&tf)
        .bind(from)
        .bind(as_of)
        .fetch_all(self.pool())
        .await?;
        let mut out = Vec::with_capacity(zones.len());
        for (id, direction, created_at, bottom, top, atr, size) in zones {
            let events: Vec<FvgEventRow> = sqlx::query_as(
                "SELECT event, horizon_bars, at, bars, return_atr FROM fvg_zone_events
                 WHERE zone_id = $1 AND at <= $2",
            )
            .bind(id)
            .bind(as_of)
            .fetch_all(self.pool())
            .await?;
            let point = |name: &str| {
                events
                    .iter()
                    .find(|r| r.0 == name)
                    .map(|r| json!({ "at": utc(r.2), "bars": r.3 }))
                    .unwrap_or(Value::Null)
            };
            let reactions: Vec<Value> = reaction_bars
                .iter()
                .map(|k| {
                    let r = events
                        .iter()
                        .find(|r| r.0 == "reaction" && r.1 == Some(int(k.get())));
                    json!({
                        "bars": k.get(),
                        "return_atr": r.and_then(|r| r.4).map(|v| v.to_string()),
                        "at": r.map(|r| utc(r.2)),
                    })
                })
                .collect();
            let (fill, expiry) = (point("fill"), point("expiry"));
            let status = match (fill.is_null(), expiry.is_null()) {
                (false, _) => "filled",
                (true, false) => "expired",
                (true, true) => "open",
            };
            let doc = json!({
                "instrument_id": instrument.as_str(),
                "timeframe": tf,
                "direction": direction,
                "created_at": utc(created_at),
                "bottom": bottom.to_string(),
                "top": top.to_string(),
                "atr": atr.to_string(),
                "size_atr": size.to_string(),
                "touch": point("touch"),
                "mid": point("mid"),
                "fill": fill,
                "expiry": expiry,
                "reactions": reactions,
                "status": status,
                "as_of": utc(as_of),
            });
            out.push(serde_json::from_value(doc)?);
        }
        Ok(out)
    }

    /// Guarda gaps de apertura (`opening_gap.v1`): el gap y su llenado o vencimiento.
    pub async fn insert_opening_gaps(
        &self,
        gaps: &[OpeningGap],
    ) -> Result<StatsReport, StorageError> {
        let mut tx = self.pool().begin().await?;
        let mut report = StatsReport::default();
        for g in gaps {
            let direction = text(g.direction)?;
            let at_gap = g.as_of;
            let inserted: Option<i64> = sqlx::query_scalar(
                "INSERT INTO opening_gaps (instrument_id, day, direction, prev_close, open, atr,
                     size_atr)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT ON CONSTRAINT opening_gaps_key DO NOTHING
                 RETURNING id",
            )
            .bind(g.instrument_id.as_str())
            .bind(g.day)
            .bind(&direction)
            .bind(g.prev_close)
            .bind(g.open)
            .bind(g.atr)
            .bind(g.size_atr)
            .fetch_optional(&mut *tx)
            .await?;
            let id = match inserted {
                Some(id) => id,
                None => {
                    let (id, d, prev, open, atr, size): (
                        i64,
                        String,
                        Decimal,
                        Decimal,
                        Decimal,
                        Decimal,
                    ) = sqlx::query_as(
                        "SELECT id, direction, prev_close, open, atr, size_atr FROM opening_gaps
                             WHERE instrument_id = $1 AND day = $2",
                    )
                    .bind(g.instrument_id.as_str())
                    .bind(g.day)
                    .fetch_one(&mut *tx)
                    .await?;
                    if (d, prev, open, atr, size)
                        != (direction.clone(), g.prev_close, g.open, g.atr, g.size_atr)
                    {
                        return Err(conflict("gap de apertura", &g.instrument_id, at_gap));
                    }
                    id
                }
            };
            let computed = gap_event_row(g)?;
            let stored: Option<GapEventRow> = sqlx::query_as(
                "SELECT event, day, sessions, at FROM opening_gap_events WHERE gap_id = $1",
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            let added = match (stored, computed) {
                (None, Some((event, day, sessions, at))) => {
                    sqlx::query(
                        "INSERT INTO opening_gap_events (gap_id, event, day, sessions, at)
                         VALUES ($1, $2, $3, $4, $5)",
                    )
                    .bind(id)
                    .bind(event)
                    .bind(day)
                    .bind(sessions)
                    .bind(at)
                    .execute(&mut *tx)
                    .await?;
                    true
                }
                (None, None) => false,
                (Some(s), Some(c)) if s == c => false,
                (Some(_), _) => return Err(conflict("gap de apertura", &g.instrument_id, at_gap)),
            };
            report.new_events += usize::from(added);
            if inserted.is_some() {
                report.new_rows += 1;
            } else if !added {
                report.unchanged += 1;
            }
        }
        tx.commit().await?;
        Ok(report)
    }

    /// Gaps de un instrumento con sesión desde `from`, **tal como se conocían en `as_of`**: solo
    /// los de sesiones cuya vela diaria ya terminó en `as_of` (según `daily_candles`) y sus
    /// eventos con `at <= as_of`.
    pub async fn opening_gaps(
        &self,
        instrument: &InstrumentId,
        from: NaiveDate,
        as_of: DateTime<Utc>,
    ) -> Result<Vec<OpeningGap>, StorageError> {
        type Row = (
            NaiveDate,
            String,
            Decimal,
            Decimal,
            Decimal,
            Decimal,
            Option<String>,
            Option<NaiveDate>,
            Option<i32>,
            Option<DateTime<Utc>>,
        );
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT g.day, g.direction, g.prev_close, g.open, g.atr, g.size_atr,
                    e.event, e.day, e.sessions, e.at
             FROM opening_gaps g
             JOIN daily_candles d ON d.instrument_id = g.instrument_id AND d.day = g.day
             LEFT JOIN opening_gap_events e ON e.gap_id = g.id AND e.at <= $3
             WHERE g.instrument_id = $1 AND g.day >= $2 AND d.end_at <= $3
             ORDER BY g.day",
        )
        .bind(instrument.as_str())
        .bind(from)
        .bind(as_of)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter()
            .map(
                |(day, direction, prev, open, atr, size, event, eday, sessions, at)| {
                    let session = json!({
                        "day": eday.map(|d| d.to_string()),
                        "sessions": sessions,
                        "at": at.map(utc),
                    });
                    let (fill, expiry, status) = match event.as_deref() {
                        Some("fill") => (session, Value::Null, "filled"),
                        Some("expiry") => (Value::Null, session, "expired"),
                        _ => (Value::Null, Value::Null, "open"),
                    };
                    let doc = json!({
                        "instrument_id": instrument.as_str(),
                        "day": day.to_string(),
                        "direction": direction,
                        "prev_close": prev.to_string(),
                        "open": open.to_string(),
                        "atr": atr.to_string(),
                        "size_atr": size.to_string(),
                        "same_session": fill.get("sessions") == Some(&json!(0)),
                        "fill": fill,
                        "expiry": expiry,
                        "status": status,
                        "as_of": utc(as_of),
                    });
                    serde_json::from_value(doc).map_err(StorageError::from)
                },
            )
            .collect()
    }

    /// Guarda valores del desequilibrio (`imbalance.v1`). Uno idéntico ya guardado cuenta como
    /// `unchanged`; otro valor para la misma vela es repintado y no se guarda nada.
    pub async fn insert_imbalances(&self, rows: &[Imbalance]) -> Result<StatsReport, StorageError> {
        type Row = (Option<String>, Decimal, Decimal, Decimal, Decimal);
        let mut tx = self.pool().begin().await?;
        let mut report = StatsReport::default();
        for r in rows {
            let tf = text(r.timeframe)?;
            let displacement = r.displacement.map(text).transpose()?;
            let window = int(r.window_bars.get());
            let inserted = sqlx::query(
                "INSERT INTO imbalances (instrument_id, timeframe, at, window_bars, displacement,
                     bullish_body_sum, bearish_body_sum, range_sum, imbalance_index)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                 ON CONFLICT ON CONSTRAINT imbalances_key DO NOTHING",
            )
            .bind(r.instrument_id.as_str())
            .bind(&tf)
            .bind(r.at)
            .bind(window)
            .bind(&displacement)
            .bind(r.bullish_body_sum)
            .bind(r.bearish_body_sum)
            .bind(r.range_sum)
            .bind(r.index)
            .execute(&mut *tx)
            .await?
            .rows_affected()
                == 1;
            if inserted {
                report.new_rows += 1;
                continue;
            }
            let stored: Row = sqlx::query_as(
                "SELECT displacement, bullish_body_sum, bearish_body_sum, range_sum, imbalance_index
                 FROM imbalances
                 WHERE instrument_id = $1 AND timeframe = $2 AND at = $3 AND window_bars = $4",
            )
            .bind(r.instrument_id.as_str())
            .bind(&tf)
            .bind(r.at)
            .bind(window)
            .fetch_one(&mut *tx)
            .await?;
            let computed = (
                displacement,
                r.bullish_body_sum,
                r.bearish_body_sum,
                r.range_sum,
                r.index,
            );
            if stored != computed {
                return Err(conflict("desequilibrio", &r.instrument_id, r.at));
            }
            report.unchanged += 1;
        }
        tx.commit().await?;
        Ok(report)
    }

    /// Desequilibrio de un instrumento y timeframe con `at` en `[from, to]`, en orden.
    pub async fn imbalances(
        &self,
        instrument: &InstrumentId,
        timeframe: StatsTimeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Imbalance>, StorageError> {
        type Row = (
            DateTime<Utc>,
            i32,
            Option<String>,
            Decimal,
            Decimal,
            Decimal,
            Decimal,
        );
        let tf = text(timeframe)?;
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT at, window_bars, displacement, bullish_body_sum, bearish_body_sum, range_sum,
                    imbalance_index
             FROM imbalances
             WHERE instrument_id = $1 AND timeframe = $2 AND at >= $3 AND at <= $4
             ORDER BY at, window_bars",
        )
        .bind(instrument.as_str())
        .bind(&tf)
        .bind(from)
        .bind(to)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter()
            .map(|(at, window, displacement, bull, bear, range, index)| {
                let doc = json!({
                    "instrument_id": instrument.as_str(),
                    "timeframe": tf,
                    "at": utc(at),
                    "window_bars": window,
                    "displacement": displacement,
                    "bullish_body_sum": bull.to_string(),
                    "bearish_body_sum": bear.to_string(),
                    "range_sum": range.to_string(),
                    "index": index.to_string(),
                    "label": "aproximación desde el precio",
                });
                serde_json::from_value(doc).map_err(StorageError::from)
            })
            .collect()
    }

    /// Guarda los agregados (`liquidity_stats.v1`) de una corrida hecha en `computed_at`. Por
    /// grupo (instrumento, tipo y timeframe): si es igual a su última corrida guardada no se
    /// guarda otra vez (así el comando es idempotente); si cambió, se agrega con `computed_at` y
    /// las corridas anteriores quedan como estaban.
    pub async fn insert_liquidity_stats(
        &self,
        computed_at: DateTime<Utc>,
        stats: &[LiquidityStats],
    ) -> Result<StatsRunReport, StorageError> {
        let mut groups: BTreeMap<(String, String, String), Vec<&LiquidityStats>> = BTreeMap::new();
        for s in stats {
            let key = (
                s.instrument_id.to_string(),
                text(s.subject)?,
                text(s.timeframe)?,
            );
            groups.entry(key).or_default().push(s);
        }
        let mut tx = self.pool().begin().await?;
        let mut report = StatsRunReport::default();
        for ((instrument, subject, timeframe), group) in groups {
            let last: Vec<Value> = sqlx::query_scalar(
                "SELECT document FROM liquidity_stats
                 WHERE instrument_id = $1 AND subject = $2 AND timeframe = $3
                   AND computed_at = (SELECT max(computed_at) FROM liquidity_stats
                                      WHERE instrument_id = $1 AND subject = $2 AND timeframe = $3)",
            )
            .bind(&instrument)
            .bind(&subject)
            .bind(&timeframe)
            .fetch_all(&mut *tx)
            .await?;
            let last: Vec<LiquidityStats> = last
                .into_iter()
                .map(serde_json::from_value)
                .collect::<Result<_, _>>()?;
            if last.len() == group.len() && group.iter().all(|s| last.contains(s)) {
                report.unchanged_groups += 1;
                continue;
            }
            for s in group {
                sqlx::query(
                    "INSERT INTO liquidity_stats (computed_at, instrument_id, subject, timeframe,
                         direction, size_tercile, as_of, n, document)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                )
                .bind(computed_at)
                .bind(&instrument)
                .bind(&subject)
                .bind(&timeframe)
                .bind(text(s.direction)?)
                .bind(text(s.size_tercile)?)
                .bind(s.as_of)
                .bind(int(s.n))
                .bind(serde_json::to_value(s)?)
                .execute(&mut *tx)
                .await?;
            }
            report.new_groups += 1;
        }
        tx.commit().await?;
        Ok(report)
    }

    /// Los agregados de la última corrida, hecha hasta `as_of` (inclusive), de cada tipo y
    /// timeframe del instrumento.
    pub async fn latest_liquidity_stats(
        &self,
        instrument: &InstrumentId,
        as_of: DateTime<Utc>,
    ) -> Result<Vec<LiquidityStats>, StorageError> {
        let docs: Vec<Value> = sqlx::query_scalar(
            "SELECT s.document FROM liquidity_stats s
             WHERE s.instrument_id = $1
               AND s.computed_at = (SELECT max(computed_at) FROM liquidity_stats
                                    WHERE instrument_id = s.instrument_id AND subject = s.subject
                                      AND timeframe = s.timeframe AND computed_at <= $2)
             ORDER BY s.subject, s.timeframe, s.direction, s.size_tercile",
        )
        .bind(instrument.as_str())
        .bind(as_of)
        .fetch_all(self.pool())
        .await?;
        docs.into_iter()
            .map(|d| serde_json::from_value(d).map_err(StorageError::from))
            .collect()
    }
}
