//! Puntos de control del broker simulado (#463).
//!
//! Para reconstruir igual, el broker guarda las velas desde la primera orden del registro (#438)
//! y vuelve a valuar cada operación desde su entrada: el costo de cada paso crece sin límite.
//! Un punto de control guarda el estado del broker después de las primeras `after_orders`
//! órdenes, y la reconstrucción arranca desde ahí:
//!
//! ```text
//! punto de control (después de una reconciliación limpia):
//!   D = 00:00 (spec §10: Madrid) del día de la vela cerrada más vieja entre las últimas de
//!       cada instrumento: toda valuación futura es en D o después
//!   operaciones cerradas con todo su movimiento antes de D → selladas: su realizado en EUR,
//!       su resultado (`trade_outcomes`) y sus instrumentos; no se vuelven a valuar
//!   por instrumento: se sigue desde la vela cerrada en D (o la entrada de una abierta, si es
//!       anterior), con el cierre y el ATR de Wilder de la vela anterior (mismo ATR, #463)
//!   + máximo del capital (valuado en cada vela cerrada hasta D, #691), kill switch,
//!     operaciones abiertas (índices desde el corte)
//!   + SHA-256 del registro de órdenes cubierto y de la config del broker
//! guardado: texto JSON exacto + hash encadenado al anterior (solo agregar, migración 0043)
//! reconstrucción: el último punto que verifica (cadena, registro, config, velas) + las órdenes
//!   y cambios del kill switch posteriores; si ninguno verifica, desde la primera orden.
//! ```
//!
//! Da exactamente lo mismo que la reconstrucción completa (tests de equivalencia). Si algo pide
//! valuar antes de lo sellado (no puede pasar con velas que solo se agregan), el broker activa
//! el kill switch: nunca adivina.

use std::collections::BTreeMap;

use cfds_core::{InstrumentId, Order, OrderReport};
use cfds_indicators::AtrSeed;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::paper::{BrokerConfig, MarketSetup, OpenTrade, PaperBroker, Sealed};

/// Versión del formato de `BrokerCheckpoint`. Otro valor no se usa (se reconstruye completo).
/// 2 (#691): el máximo del capital incluye el capital valuado en cada vela cerrada hasta
/// `safe_from`; los de la versión 1 lo tenían solo de las órdenes evaluadas.
pub const CHECKPOINT_VERSION: u32 = 2;

/// Desde qué vela sigue un instrumento y el estado del ATR antes de ella.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketCut {
    /// Primera vela que sigue. `None`: el instrumento no tenía velas cerradas; no se corta y se
    /// cargan sus velas como en la reconstrucción completa.
    pub first_start: Option<DateTime<Utc>>,
    /// `None`: el ATR arranca en `first_start` (no se cortó nunca).
    pub atr_seed: Option<AtrSeed>,
}

/// El estado del broker después de las primeras `after_orders` órdenes del registro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrokerCheckpoint {
    pub version: u32,
    /// Hora del broker al tomarlo.
    pub taken_at: DateTime<Utc>,
    pub after_orders: usize,
    /// Cuántos cambios guardados del kill switch (#441) ya están aplicados.
    pub marks_applied: usize,
    /// SHA-256 de las primeras `after_orders` órdenes del registro con sus informes.
    pub journal_hash: String,
    /// SHA-256 de la config del broker y de lo fijo de cada mercado (límites, costos, corte).
    pub setup_hash: String,
    /// Desde cuándo vale: lo sellado terminó antes.
    pub safe_from: DateTime<Utc>,
    pub cuts: BTreeMap<InstrumentId, MarketCut>,
    pub(crate) peak_equity: Decimal,
    pub(crate) kill_switch: Option<(String, DateTime<Utc>)>,
    pub(crate) sealed: Sealed,
    pub(crate) trades: Vec<OpenTrade>,
}

/// Un punto de control listo para guardar: el texto exacto y su hash encadenado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedCheckpoint {
    pub after_orders: usize,
    pub state: String,
    pub prev_hash: Option<String>,
    pub hash: String,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Cada parte con su largo: ninguna concatenación se confunde con otra.
fn sha256_parts<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update((p.len() as u64).to_be_bytes());
        h.update(p.as_bytes());
    }
    hex(&h.finalize())
}

/// SHA-256 de un tramo del registro (orden e informe en JSON, en orden).
pub fn journal_hash(journal: &[(Order, OrderReport)]) -> String {
    let docs: Vec<String> = journal
        .iter()
        .flat_map(|(o, r)| {
            [
                serde_json::to_string(o).unwrap_or_default(),
                serde_json::to_string(r).unwrap_or_default(),
            ]
        })
        .collect();
    sha256_parts(docs.iter().map(String::as_str))
}

/// SHA-256 de la config del broker y de lo fijo de cada mercado. Las tasas y los tipos de
/// cambio no entran: se vuelven a leer en marcha (#439) y lo sellado ya los usó.
pub fn setup_hash(cfg: &BrokerConfig, markets: &BTreeMap<InstrumentId, MarketSetup>) -> String {
    let mut parts = vec![format!("{cfg:?}")];
    for (id, m) in markets {
        parts.push(format!(
            "{id}|{:?}|{:?}|{:?}|{:?}",
            m.limits, m.costs, m.funding.cutoff, m.funding.tz
        ));
    }
    sha256_parts(parts.iter().map(String::as_str))
}

fn chain_hash(prev: Option<&str>, state: &str) -> String {
    sha256_parts([prev.unwrap_or(""), state])
}

/// Serializa el punto de control y lo encadena al anterior (`prev_hash`).
pub fn seal(ck: &BrokerCheckpoint, prev_hash: Option<&str>) -> Result<SealedCheckpoint, String> {
    let state = serde_json::to_string(ck).map_err(|e| e.to_string())?;
    Ok(SealedCheckpoint {
        after_orders: ck.after_orders,
        hash: chain_hash(prev_hash, &state),
        prev_hash: prev_hash.map(str::to_owned),
        state,
    })
}

/// Los puntos de control guardados cuya cadena verifica, en orden (con su hash). Se corta en
/// la primera fila que no verifica: lo que viene después no se usa.
pub fn verified_chain(rows: &[SealedCheckpoint]) -> Vec<(BrokerCheckpoint, String)> {
    let mut out = Vec::new();
    let mut prev: Option<&str> = None;
    for r in rows {
        if r.prev_hash.as_deref() != prev || chain_hash(prev, &r.state) != r.hash {
            break;
        }
        let Ok(ck) = serde_json::from_str::<BrokerCheckpoint>(&r.state) else {
            break;
        };
        if ck.version != CHECKPOINT_VERSION || ck.after_orders != r.after_orders {
            break;
        }
        out.push((ck, r.hash.clone()));
        prev = Some(&r.hash);
    }
    out
}

impl PaperBroker {
    /// El punto de control del broker en `now`, o `None` si no se puede sellar nada nuevo
    /// (sin órdenes todavía, sin velas cerradas, o nada cerrado antes de D ni velas para
    /// soltar). `marks_applied`: cuántos cambios guardados del kill switch ya están aplicados.
    pub fn plan_checkpoint(
        &self,
        now: DateTime<Utc>,
        marks_applied: usize,
    ) -> Option<BrokerCheckpoint> {
        // Sin órdenes no hay nada que sellar, y las velas todavía se anclan a la primera orden
        // (#438, `start_candles_at`): un corte antes cambiaría ese ancla.
        if self.markets().is_empty() || self.journal().is_empty() {
            return None;
        }
        // Un instrumento sin velas cerradas no se puede operar (sin precio): no cuenta.
        let oldest_last = self
            .markets()
            .values()
            .filter_map(|m| m.candles.iter().rev().find(|c| c.end() <= now))
            .map(cfds_core::Candle::end)
            .min()?;
        let d = self.day_start(oldest_last);

        // Lo que se puede sellar: cerrado del todo antes de D y valuado sin datos faltantes.
        let pending = self.take_data_flags();
        let mut sealed = self.sealed.clone();
        let mut keep = Vec::new();
        for t in &self.trades {
            let mut ok = t.opened_at < d;
            let mut realized = Decimal::ZERO;
            let mut last_event = t.opened_at;
            let mut outcome = None;
            if ok {
                let (open, r, events) = self.valuate_open_fraction_and_events(t, now);
                realized = r;
                last_event = events
                    .iter()
                    .map(|(at, _)| *at)
                    .max()
                    .unwrap_or(t.opened_at);
                outcome = Some(self.outcome_of(t, now));
                let flags = self.take_data_flags();
                ok = open.is_zero() && last_event < d && flags == (None, None);
            }
            match outcome {
                Some(o) if ok => {
                    sealed.realized.push((t.seq, realized));
                    sealed.until = Some(sealed.until.map_or(last_event, |u| u.max(last_event)));
                    sealed.instruments.insert(t.instrument().clone());
                    sealed.outcomes.push((t.seq, o));
                }
                _ => keep.push(t.clone()),
            }
        }
        // El máximo del capital al día hasta D (#691): desde ahí sigue quien arranque del punto
        // de control, porque antes de D ya no puede valuar. Sin marcar datos faltantes.
        let (peak_equity, _) = self.peak_until(d);
        self.restore_data_flags(pending);
        sealed.outcomes.sort_by_key(|(seq, _)| *seq);
        sealed.realized.sort_by_key(|(seq, _)| *seq);

        // Por instrumento: desde la vela cerrada en D (o la entrada de una abierta anterior).
        let mut cuts = BTreeMap::new();
        let mut shift = BTreeMap::new();
        for (id, m) in self.markets() {
            if !m.candles.iter().any(|c| c.end() <= now) {
                cuts.insert(
                    id.clone(),
                    MarketCut {
                        first_start: None,
                        atr_seed: None,
                    },
                );
                shift.insert(id.clone(), 0);
                continue;
            }
            let atr = self.atr(id, &m.candles);
            let mut k = m.candles.iter().rposition(|c| c.end() <= d).unwrap_or(0);
            for t in keep.iter().filter(|t| t.instrument() == id) {
                k = k.min(t.entry_index);
            }
            let seed = match k.checked_sub(1).map(|p| (p, atr[p])) {
                Some((p, Some(prev_atr))) => Some(AtrSeed {
                    prev_close: m.candles[p].close,
                    prev_atr,
                }),
                // Todavía calentando: no se corta.
                Some((_, None)) => {
                    k = 0;
                    self.atr_seeds.get(id).copied()
                }
                None => self.atr_seeds.get(id).copied(),
            };
            cuts.insert(
                id.clone(),
                MarketCut {
                    first_start: Some(m.candles[k].start),
                    atr_seed: seed,
                },
            );
            shift.insert(id.clone(), k);
        }
        let newly_sealed = sealed.outcomes.len() > self.sealed.outcomes.len();
        if !newly_sealed && shift.values().all(|k| *k == 0) {
            return None;
        }
        for t in &mut keep {
            t.entry_index -= shift[t.instrument()];
        }
        Some(BrokerCheckpoint {
            version: CHECKPOINT_VERSION,
            taken_at: now,
            after_orders: self.journal().len(),
            marks_applied,
            journal_hash: journal_hash(self.journal()),
            setup_hash: setup_hash(self.config(), self.markets()),
            safe_from: d,
            cuts,
            peak_equity,
            kill_switch: self.kill_switch.clone(),
            sealed,
            trades: keep,
        })
    }

    /// Corta el broker en marcha en `ck` (el mismo punto que daría [`Self::plan_checkpoint`]
    /// sobre él, salvo el kill switch, que sigue siendo el suyo). Si no coincide, no cambia nada.
    pub fn apply_checkpoint(&mut self, ck: &BrokerCheckpoint) -> Result<(), String> {
        let mut mine = self
            .plan_checkpoint(ck.taken_at, ck.marks_applied)
            .ok_or("el broker no tiene ese punto de control")?;
        mine.kill_switch.clone_from(&ck.kill_switch);
        if &mine != ck {
            return Err("el punto de control no coincide con el broker en marcha".into());
        }
        self.cut_candles(ck)?;
        self.peak_equity = ck.peak_equity;
        self.peak_through = self.peak_through.max(Some(ck.safe_from));
        self.trades.clone_from(&ck.trades);
        self.sealed = ck.sealed.clone();
        Ok(())
    }

    /// Suelta las velas anteriores a cada corte y guarda el estado del ATR.
    fn cut_candles(&mut self, ck: &BrokerCheckpoint) -> Result<(), String> {
        let ids: Vec<InstrumentId> = self.markets().keys().cloned().collect();
        if ids.len() != ck.cuts.len() || ids.iter().any(|id| !ck.cuts.contains_key(id)) {
            return Err("los instrumentos no son los del punto de control".into());
        }
        for (id, cut) in &ck.cuts {
            let Some(first) = cut.first_start else {
                if cut.atr_seed.is_some() {
                    return Err(format!("{id}: estado del ATR sin vela"));
                }
                self.atr_seeds.remove(id);
                continue;
            };
            let candles = &self.markets()[id].candles;
            let k = candles
                .iter()
                .position(|c| c.start == first)
                .ok_or_else(|| format!("{id}: falta la vela de {first}"))?;
            self.markets_mut()
                .get_mut(id)
                .expect("instrumento ya visto")
                .candles
                .drain(..k);
            match cut.atr_seed {
                Some(seed) => self.atr_seeds.insert(id.clone(), seed),
                None => self.atr_seeds.remove(id),
            };
        }
        Ok(())
    }

    /// El broker tal como quedó en `ck`, con `journal` (el registro guardado completo o al menos
    /// las órdenes que cubre) y los mercados (sus velas tienen que incluir la primera de cada
    /// corte; las anteriores se sueltan). Error si algo no verifica: se reconstruye completo.
    pub fn from_checkpoint(
        cfg: BrokerConfig,
        markets: BTreeMap<InstrumentId, MarketSetup>,
        ck: &BrokerCheckpoint,
        journal: &[(Order, OrderReport)],
    ) -> Result<Self, String> {
        if ck.version != CHECKPOINT_VERSION {
            return Err(format!("versión {} desconocida", ck.version));
        }
        let covered = journal
            .get(..ck.after_orders)
            .ok_or("el registro tiene menos órdenes que el punto de control")?;
        if journal_hash(covered) != ck.journal_hash {
            return Err("el registro guardado no es el que cubre el punto de control".into());
        }
        if setup_hash(&cfg, &markets) != ck.setup_hash {
            return Err("la config del broker cambió desde el punto de control".into());
        }
        let mut paper = PaperBroker::new(cfg, markets);
        paper.cut_candles(ck)?;
        paper.set_journal(covered);
        paper.peak_equity = ck.peak_equity;
        // El máximo incluye todo hasta D (`safe_from`): sigue desde ahí.
        paper.peak_through = Some(ck.safe_from);
        paper.kill_switch.clone_from(&ck.kill_switch);
        paper.sealed = ck.sealed.clone();
        paper.trades.clone_from(&ck.trades);
        Ok(paper)
    }
}
