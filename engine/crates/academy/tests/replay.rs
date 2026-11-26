//! Sesiones de replay (#721): misma semilla, misma sesión; nunca el fuera de muestra; el cursor
//! no deja ver el futuro; la fecha real no viaja.

mod common;

use cfds_academy::AcademyConfig;
use cfds_academy::replay::{
    BarStore, MarkKind, MarkRequest, OrderRequest, OrderSide, ProcessRules, ReplayError,
    ReplayKind, ReplaySession, open_session,
};
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_market_data::{SplitConfig, plan};
use chrono::{DateTime, Datelike, Utc, Weekday};
use common::{REAL, assert_valid, validator};
use rust_decimal::Decimal;
use serde_json::json;

const SPLIT: &str = include_str!("../../../../config/data_split.toml");

/// Velas sintéticas de lunes a viernes, de `first` a `last`, generadas al vuelo.
struct Synthetic {
    first: DateTime<Utc>,
    last: DateTime<Utc>,
    /// Sin velas en este tramo (feriado largo).
    hole: Option<(DateTime<Utc>, DateTime<Utc>)>,
}

impl Synthetic {
    fn years(from: i32, to: i32) -> Self {
        Self {
            first: format!("{from}-01-03T00:00:00Z").parse().unwrap(),
            last: format!("{to}-12-30T23:45:00Z").parse().unwrap(),
            hole: None,
        }
    }
}

fn candle(id: &InstrumentId, tf: Timeframe, start: DateTime<Utc>) -> Candle {
    let n = Decimal::from(start.timestamp() / 900 % 97);
    let open = Decimal::from(4000) + n;
    Candle {
        instrument_id: id.clone(),
        timeframe: tf,
        start,
        open,
        high: open + Decimal::from(3),
        low: open - Decimal::from(2),
        close: open + Decimal::ONE,
        volume: None,
    }
}

impl BarStore for Synthetic {
    async fn range(
        &self,
        _: &InstrumentId,
        _: Timeframe,
    ) -> Result<Option<(DateTime<Utc>, DateTime<Utc>)>, String> {
        Ok(Some((self.first, self.last)))
    }

    async fn bars(
        &self,
        id: &InstrumentId,
        tf: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Candle>, String> {
        let mut out = Vec::new();
        let mut t = from.max(self.first);
        while t < to && t <= self.last {
            let weekday = !matches!(t.weekday(), Weekday::Sat | Weekday::Sun);
            let in_hole = self.hole.is_some_and(|(a, b)| t >= a && t < b);
            if weekday && !in_hole {
                out.push(candle(id, tf, t));
            }
            t += tf.duration();
        }
        Ok(out)
    }
}

fn cfg() -> AcademyConfig {
    AcademyConfig::from_toml(REAL).unwrap()
}

fn split() -> SplitConfig {
    SplitConfig::from_toml(SPLIT).unwrap()
}

fn rules() -> ProcessRules {
    ProcessRules {
        stop_min_atr: "0.5".parse().unwrap(),
        stop_max_atr: "3".parse().unwrap(),
        max_risk_pct: Decimal::ONE,
        provisional: true,
    }
}

async fn open(seed: u64, kind: ReplayKind) -> ReplaySession {
    let store = Synthetic::years(2011, 2016);
    open_session(&store, &cfg().replay, &split(), kind, None, seed, 7)
        .await
        .unwrap()
}

#[tokio::test]
async fn the_same_seed_gives_the_same_session() {
    let a = open(42, ReplayKind::Trades).await;
    let b = open(42, ReplayKind::Trades).await;
    assert_eq!(a, b);
    assert_eq!(a.cursor(), 299);
    assert_eq!(a.last_index(), 300 + 400 - 1);
    let other: Vec<_> = futures_like(&[1, 2, 3, 4, 5, 6, 7, 8]).await;
    assert!(
        other.iter().any(|s| s.draw != a.draw),
        "8 semillas, todas iguales"
    );
    assert_eq!(
        open(42, ReplayKind::Practice).await.last_index(),
        300 + 200 - 1
    );
}

async fn futures_like(seeds: &[u64]) -> Vec<ReplaySession> {
    let mut out = Vec::new();
    for s in seeds {
        out.push(open(*s, ReplayKind::Trades).await);
    }
    out
}

#[tokio::test]
async fn no_session_touches_the_out_of_sample_months() {
    let store = Synthetic::years(2011, 2016);
    let holdout = plan(store.first, store.last, &split())
        .unwrap()
        .out_of_sample
        .start;
    // Los 12 meses completos antes del mes de la última vela (diciembre de 2016).
    assert_eq!(
        holdout,
        "2015-12-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap()
    );
    for seed in 0..60 {
        let mut s = open(seed, ReplayKind::Levels).await;
        s.finish().unwrap();
        let bars = s.all_bars().unwrap();
        assert!(bars.last().unwrap().end() <= holdout, "semilla {seed}");
        assert!(bars[0].start >= store.first);
        assert_eq!(s.draw.research.end, holdout);
    }
    // Con menos datos que un tramo fuera de muestra más una ventana, no hay sesión.
    let short = Synthetic::years(2015, 2016);
    let r = open_session(
        &short,
        &cfg().replay,
        &split(),
        ReplayKind::Trades,
        None,
        1,
        7,
    )
    .await;
    assert!(matches!(r, Err(ReplayError::NoData(_))), "{r:?}");
}

#[tokio::test]
async fn days_without_bars_are_skipped_and_the_draws_have_a_limit() {
    let mut store = Synthetic::years(2011, 2016);
    store.hole = Some((store.first, "2015-11-30T00:00:00Z".parse().unwrap()));
    let r = open_session(
        &store,
        &cfg().replay,
        &split(),
        ReplayKind::Trades,
        None,
        3,
        7,
    )
    .await;
    assert_eq!(r, Err(ReplayError::NoWindow(20)));
}

#[tokio::test]
async fn the_cursor_never_lets_a_later_bar_out() {
    let mut s = open(9, ReplayKind::Trades).await;
    assert_eq!(s.visible().len(), 300);
    assert!(s.bars_until(299).is_ok());
    assert_eq!(
        s.bars_until(300),
        Err(ReplayError::AfterCursor {
            asked: 300,
            cursor: 299
        })
    );
    assert!(s.all_bars().is_err(), "todas las velas solo al terminar");
    let step = s.step(5, 100).unwrap();
    assert_eq!((step.cursor, step.bars.len()), (304, 5));
    assert_eq!(step.bars[0].index, 300);
    assert!(s.bars_until(305).is_err());
    // El paso se corta en el máximo y en el final.
    assert_eq!(s.step(1000, 100).unwrap().bars.len(), 100);
    let mut last = s.step(1000, 1000).unwrap();
    assert!(last.done);
    assert_eq!(last.cursor, s.last_index());
    last = s.step(1, 100).unwrap();
    assert!(last.done && last.bars.is_empty());
    assert!(s.step(0, 100).is_err());
}

#[tokio::test]
async fn the_real_date_does_not_travel_and_the_views_follow_the_contracts() {
    let mut s = open(11, ReplayKind::Trades).await;
    let view = serde_json::to_value(s.view(&cfg().replay, rules())).unwrap();
    assert_valid("replay_session.v1.schema.json", &view);
    let text = view.to_string();
    for year in 2011..=2016 {
        assert!(
            !text.contains(&year.to_string()),
            "la vista deja ver el año {year}"
        );
    }
    assert_eq!(view["bars"][0]["day"], json!(1));
    assert_eq!(view["seed"], json!("11"));
    let step = serde_json::to_value(s.step(3, 100).unwrap()).unwrap();
    assert_valid("replay_step.v1.schema.json", &step);
    // Un campo de fecha agregado: lo rechaza el contrato.
    let mut leaked = step.clone();
    leaked["bars"][0]["start"] = json!("2012-03-12T09:30:00Z");
    assert!(!validator("replay_step.v1.schema.json").is_valid(&leaked));
}

#[tokio::test]
async fn marks_and_orders_carry_the_cursor_and_respect_it() {
    let mut s = open(5, ReplayKind::Trades).await;
    let level = |kind, bar_index| MarkRequest {
        kind,
        price: Decimal::from(4010),
        bar_index,
    };
    let m = s.mark(level(MarkKind::Level, None)).unwrap();
    assert_eq!((m.mark_id, m.cursor), (1, 299));
    assert_valid(
        "replay_mark.v1.schema.json",
        &serde_json::to_value(&m).unwrap(),
    );
    assert!(s.mark(level(MarkKind::PivotHigh, Some(280))).is_ok());
    assert!(
        s.mark(level(MarkKind::PivotHigh, Some(300))).is_err(),
        "pivote en una vela que no se ve"
    );
    assert!(s.mark(level(MarkKind::Level, Some(10))).is_err());
    assert!(s.mark(level(MarkKind::PivotLow, None)).is_err());
    let order = |stop: Option<i64>| OrderRequest {
        side: OrderSide::Long,
        stop: stop.map(Decimal::from),
        target: None,
        risk_pct: Decimal::ONE,
        plan: "Compro si cierra arriba del nivel".into(),
    };
    s.step(10, 100).unwrap();
    let o = s.order(order(None)).unwrap();
    assert_eq!((o.order_id, o.cursor, o.stop), (1, 309, None));
    assert_valid(
        "replay_order.v1.schema.json",
        &serde_json::to_value(&o).unwrap(),
    );
    // En la última vela no hay apertura siguiente para entrar.
    s.step(10_000, 10_000).unwrap();
    assert!(s.order(order(Some(3990))).is_err());
    s.finish().unwrap();
    assert!(s.all_bars().is_ok());
    assert_eq!(s.finish(), Err(ReplayError::Finished));
    assert_eq!(s.step(1, 1).err(), Some(ReplayError::Finished));
    assert!(s.mark(level(MarkKind::Level, None)).is_err());
}

#[test]
fn the_replay_config_is_validated() {
    let c = cfg().replay;
    assert_eq!(
        (c.warmup_bars, c.exam_bars, c.trades_per_exam),
        (300, 400, 10)
    );
    assert_eq!(c.level_tolerance_atr.to_string(), "0.25");
    assert!(!c.shift_prices);
    for (from, to) in [
        ("shift_prices = false", "shift_prices = true"),
        ("source = \"futures\"", "source = \"ig\""),
        ("warmup_bars = 300", "warmup_bars = 0"),
        (
            "level_tolerance_atr = \"0.25\"",
            "level_tolerance_atr = \"0\"",
        ),
        ("timeframes = [\"15m\", \"1h\"]", "timeframes = []"),
        ("timeframes = [\"15m\", \"1h\"]", "timeframes = [\"4h\"]"),
        ("max_draws = 20", "max_draws = 20\nextra = 1"),
    ] {
        let text = REAL.replace(from, to);
        assert_ne!(text, REAL, "{to}");
        assert!(AcademyConfig::from_toml(&text).is_err(), "{to}");
    }
}
