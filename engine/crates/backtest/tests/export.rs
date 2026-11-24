//! Exportación para la verificación (#81): contrato, tabla por operación y sin repintado.

use std::path::PathBuf;

use cfds_backtest::{
    AnalysisConfig, BacktestConfig, ExportAnalysis, analysis_export, analysis_view, chart_export,
    run, run_checked, setup_from_catalog,
};
use cfds_core::{Candle, Timeframe};
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::Catalog;
use cfds_patterns::PatternsConfig;
use cfds_risk::RiskConfig;
use cfds_strategy::{Configs, StrategyConfig};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};
use serde_json::Value;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

fn setup() -> cfds_backtest::Setup {
    setup_from_catalog(
        &Catalog::from_toml(cfg!("instruments.toml")).unwrap(),
        &"DE40".parse().unwrap(),
        Configs {
            patterns: PatternsConfig::from_toml(cfg!("patterns.toml")).unwrap(),
            liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml")).unwrap(),
            trend: TrendConfig::from_toml(cfg!("indicators.toml")).unwrap(),
            strategy: StrategyConfig::from_toml(cfg!("strategy.toml")).unwrap(),
        },
        RiskConfig::from_toml(cfg!("risk.toml")).unwrap(),
        BacktestConfig::from_toml(cfg!("backtest.toml")).unwrap(),
        dec!(10000),
    )
    .unwrap()
}

/// Caminata aleatoria determinística de velas de 15m del DE40 (sintéticas: no son de IG).
fn random_walk(n: usize, seed: u64) -> Vec<Candle> {
    let mut state = seed;
    let mut next = move |m: i64| -> i64 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) % m as u64) as i64
    };
    let t0: DateTime<Utc> = "2026-01-05T00:00:00Z".parse().unwrap();
    let mut close = dec!(20000);
    (0..n)
        .map(|i| {
            let drift = if (i / 150) % 2 == 0 { 1 } else { -1 };
            let open = close;
            close = open + Decimal::new(next(81) - 40 + drift * 4, 1);
            Candle {
                instrument_id: "DE40".parse().unwrap(),
                timeframe: Timeframe::M15,
                start: t0 + TimeDelta::minutes(15 * i as i64),
                open,
                high: open.max(close) + Decimal::new(next(50), 1),
                low: open.min(close) - Decimal::new(next(50), 1),
                close,
                volume: None,
            }
        })
        .collect()
}

fn resample_h1(m15: &[Candle]) -> Vec<Candle> {
    m15.chunks_exact(4)
        .map(|g| Candle {
            instrument_id: g[0].instrument_id.clone(),
            timeframe: Timeframe::H1,
            start: g[0].start,
            open: g[0].open,
            high: g.iter().map(|c| c.high).max().unwrap(),
            low: g.iter().map(|c| c.low).min().unwrap(),
            close: g[3].close,
            volume: None,
        })
        .collect()
}

fn export(m15: &[Candle]) -> cfds_backtest::ChartExport {
    let h1 = resample_h1(m15);
    let s = setup();
    let report = run(m15, &h1, &[], &[], &s).unwrap();
    chart_export(m15, &h1, &[], &s, &report).unwrap()
}

fn validator() -> jsonschema::Validator {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts/chart_export.v1.schema.json");
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

#[test]
fn the_export_follows_its_contract_and_roundtrips() {
    let e = export(&random_walk(2000, 5));
    assert!(e.trades.len() > 3, "solo {} operaciones", e.trades.len());
    assert!(!e.pivots.is_empty());
    let json = serde_json::to_value(&e).unwrap();
    let errors: Vec<String> = validator()
        .iter_errors(&json)
        .map(|x| x.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        serde_json::from_value::<cfds_backtest::ChartExport>(json).unwrap(),
        e
    );
}

#[test]
fn the_contract_rejects_floats_and_unknown_names() {
    let json = serde_json::to_value(export(&random_walk(2000, 5))).unwrap();
    let v = validator();
    let mut bad = json.clone();
    bad["candles"][100]["close"] = serde_json::json!(20000.5);
    assert!(!v.is_valid(&bad), "precio como número JSON");
    let mut bad = json.clone();
    bad["trades"][0]["exits"][0]["reason"] = serde_json::json!("Stop");
    assert!(!v.is_valid(&bad), "motivo fuera del contrato");
    let mut bad = json;
    bad["candles"][100]["trend_1h"] = serde_json::json!("Up");
    assert!(!v.is_valid(&bad), "tendencia fuera del contrato");
}

#[test]
fn each_trade_carries_the_indicator_values_of_its_signal_candle() {
    let e = export(&random_walk(2000, 5));
    for t in &e.trades {
        let c = &e.candles[t.signal_index];
        assert_eq!(t.at_signal.close, c.close);
        assert_eq!(t.at_signal.sma, c.sma);
        assert_eq!(t.at_signal.atr, c.atr);
        assert_eq!(t.at_signal.trend_1h, c.trend_1h);
        assert!(t.entry_index == t.signal_index + 1, "entra en t+1");
        let side_score = if t.side == "long" {
            c.long_score
        } else {
            c.short_score
        };
        assert_eq!(
            side_score,
            Some(t.score),
            "el puntaje exportado es el de la operación"
        );
        let total: i32 = t.breakdown.iter().map(|b| b.points).sum();
        assert_eq!(total, t.score);
    }
}

#[test]
fn no_exported_value_changes_when_more_candles_arrive() {
    // ROADMAP 3b: "ningún valor histórico cambia al agregar datos".
    let full = random_walk(2000, 9);
    let all = export(&full);
    for n in [400, 1000, 1600] {
        let part = export(&full[..n]);
        assert_eq!(part.candles[..], all.candles[..n], "velas con {n}");
        let confirmed: Vec<_> = all
            .pivots
            .iter()
            .filter(|p| p.confirmed_index < n)
            .cloned()
            .collect();
        assert_eq!(part.pivots, confirmed, "pivotes con {n}");
    }
}

#[test]
fn the_export_shows_no_scores_while_a_data_error_blocks_the_instrument() {
    // Spec §7.8 y §12: la exportación usa los mismos errores de datos que el recorrido.
    let m15 = random_walk(1200, 3);
    let h1 = resample_h1(&m15);
    let s = setup();
    let errors = [m15[600].start];
    let report = run_checked(&m15, &h1, &[], &[], &errors, &s).unwrap();
    let e = chart_export(&m15, &h1, &errors, &s, &report).unwrap();
    let scored = |c: &cfds_backtest::ChartExport, i: usize| {
        c.candles[i].long_score.is_some() || c.candles[i].short_score.is_some()
    };
    assert!(scored(&e, 599), "antes del error hay puntajes");
    assert!(
        (600..800).all(|i| !scored(&e, i)),
        "200 velas sin puntaje desde el error"
    );
    assert!(
        e.trades
            .iter()
            .all(|t| t.signal_index < 600 || t.signal_index >= 800)
    );
}

// --- Herramientas de análisis (#491) ---

const ANALYSIS: &str = cfg!("analysis.toml");

/// Diarias UTC armadas con las velas de 15m (96 por día), solo días completos.
fn daily_of(m15: &[Candle]) -> Vec<cfds_core::DailyCandle> {
    m15.chunks_exact(96)
        .map(|g| cfds_core::DailyCandle {
            instrument_id: g[0].instrument_id.clone(),
            day: g[0].start.date_naive(),
            start: g[0].start,
            end: g[95].end(),
            open: g[0].open,
            high: g.iter().map(|c| c.high).max().unwrap(),
            low: g.iter().map(|c| c.low).min().unwrap(),
            close: g[95].close,
            volume: None,
        })
        .collect()
}

/// VIX sintético publicado cada día a las 14:30 UTC (no es de Cboe).
fn vix_of(m15: &[Candle]) -> Vec<(DateTime<Utc>, Decimal)> {
    let first = m15.first().map(|c| c.start).unwrap_or_default();
    (0..m15.len() / 96 + 1)
        .map(|d| {
            (
                first + TimeDelta::days(d as i64) + TimeDelta::minutes(14 * 60 + 30),
                Decimal::from(12 + 4 * (d as i64 % 6)),
            )
        })
        .collect()
}

fn analysis(m15: &[Candle], daily: &[cfds_core::DailyCandle], period: usize) -> ExportAnalysis {
    analysis_export(
        m15,
        daily,
        &vix_of(m15),
        setup().strategy.patterns.pivots.strength,
        &test_cfg(period),
    )
}

fn test_cfg(period: usize) -> AnalysisConfig {
    let mut cfg = AnalysisConfig::from_toml(ANALYSIS).unwrap();
    cfg.ema_daily_period = period;
    // Medias cortas y fondos fáciles: las series sintéticas son de pocos días.
    cfg.ma_cross.fast = 2;
    cfg.ma_cross.slow = 4;
    cfg.wyckoff.levels.min_touches = 2;
    cfg.wyckoff.levels.tolerance = dec!(0.001);
    cfg.wyckoff.max_bars_to_reclaim = 20;
    cfg.wyckoff.max_penetration = dec!(0.05);
    cfg.bottom.min_decline = dec!(0.001);
    cfg
}

#[test]
fn the_analysis_block_follows_the_contract_and_roundtrips() {
    let m15 = random_walk(2000, 5);
    let mut e = export(&m15);
    e.analysis = Some(analysis(&m15, &daily_of(&m15), 5));
    let a = e.analysis.as_ref().unwrap();
    assert_eq!(a.ema_daily.len(), m15.len());
    assert!(!a.fibonacci.is_empty() && !a.golden_pocket.is_empty());
    assert!(!a.wyckoff.is_empty() && !a.bottoms.is_empty());
    assert!(!a.ma_cross.crosses.is_empty() && !a.floor_pivots.is_empty());
    assert!(a.vix.iter().any(Option::is_some));
    let json = serde_json::to_value(&e).unwrap();
    let errors: Vec<String> = validator()
        .iter_errors(&json)
        .map(|x| x.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        serde_json::from_value::<cfds_backtest::ChartExport>(json.clone()).unwrap(),
        e
    );
    let mut bad = json;
    bad["analysis"]["fibonacci"][0]["direction"] = serde_json::json!("Up");
    assert!(!validator().is_valid(&bad), "dirección fuera del contrato");
}

#[test]
fn a_15m_candle_only_sees_the_daily_ema_of_finished_days() {
    let m15 = random_walk(96 * 8, 2);
    let daily = daily_of(&m15);
    let a = analysis(&m15, &daily, 3);
    let ema = cfds_indicators::ema_values(&daily.iter().map(|d| d.close).collect::<Vec<_>>(), 3);
    // Días 0..=2 hacen falta para la EMA(3): el primer valor aparece al cerrar el día 2 (vela 287).
    assert!(a.ema_daily[..287].iter().all(Option::is_none));
    assert_eq!(a.ema_daily[287], ema[2]);
    // Durante el día 3 se sigue viendo el valor del día 2, nunca el del día en curso.
    assert!(a.ema_daily[288..383].iter().all(|v| *v == ema[2]));
    assert_eq!(a.ema_daily[383], ema[3]);
}

#[test]
fn without_daily_candles_or_with_few_candles_nothing_breaks() {
    let m15 = random_walk(5, 1);
    let a = analysis(&m15, &[], 365);
    assert!(a.ema_daily.iter().all(Option::is_none));
    assert!(a.fibonacci.is_empty() && a.golden_pocket.is_empty());
    assert!(a.wyckoff.is_empty() && a.bottoms.is_empty());
    assert!(a.ma_cross.crosses.is_empty() && a.floor_pivots.is_empty());
    assert!(a.ma_cross.fast.iter().all(Option::is_none));
    let a = analysis(&[], &[], 365);
    assert!(a.ema_daily.is_empty());
}

#[test]
fn no_analysis_value_changes_when_more_candles_arrive() {
    let full = random_walk(2000, 9);
    let daily = daily_of(&full);
    let all = analysis(&full, &daily, 5);
    for n in [400, 1000, 1600] {
        // Las diarias que existen con n velas: solo los días ya terminados.
        let part_daily: Vec<_> = daily
            .iter()
            .filter(|d| d.end <= full[n - 1].end())
            .cloned()
            .collect();
        let part = analysis(&full[..n], &part_daily, 5);
        assert_eq!(part.ema_daily[..], all.ema_daily[..n], "EMA diaria con {n}");
        let legs: Vec<_> = all
            .fibonacci
            .iter()
            .filter(|l| l.confirmed_index < n)
            .cloned()
            .collect();
        assert_eq!(part.fibonacci, legs, "tramos con {n}");
        let events: Vec<_> = all
            .golden_pocket
            .iter()
            .filter(|x| x.index < n)
            .cloned()
            .collect();
        assert_eq!(part.golden_pocket, events, "golden pocket con {n}");
        let w: Vec<_> = all
            .wyckoff
            .iter()
            .filter(|x| x.confirmed_index < n)
            .cloned()
            .collect();
        assert_eq!(part.wyckoff, w, "wyckoff con {n}");
        let b: Vec<_> = all
            .bottoms
            .iter()
            .filter(|x| x.confirmed_index < n)
            .cloned()
            .collect();
        assert_eq!(part.bottoms, b, "fondos con {n}");
        let c: Vec<_> = all
            .ma_cross
            .crosses
            .iter()
            .filter(|x| x.known_from_index < n)
            .cloned()
            .collect();
        assert_eq!(part.ma_cross.crosses, c, "cruces con {n}");
        assert_eq!(part.ma_cross.slow[..], all.ma_cross.slow[..n]);
        let f: Vec<_> = all
            .floor_pivots
            .iter()
            .filter(|x| x.from_index < n)
            .cloned()
            .collect();
        assert_eq!(part.floor_pivots, f, "pivotes diarios con {n}");
        assert_eq!(part.vix[..], all.vix[..n], "VIX con {n}");
    }
}

#[test]
fn the_vix_and_the_floor_pivots_come_from_what_was_known() {
    let m15 = random_walk(96 * 4, 4);
    let daily = daily_of(&m15);
    let a = analysis(&m15, &daily, 3);
    // El VIX del día 0 se publica a las 14:30: la vela de 14:15 (índice 57, cierra 14:30) ya lo ve.
    assert!(a.vix[..57].iter().all(Option::is_none));
    let v = a.vix[57].as_ref().unwrap();
    assert_eq!((v.close, v.regime.as_str()), (dec!(12), "calm"));
    assert_eq!(a.vix[96 + 57].as_ref().unwrap().regime, "normal");
    // Pivotes diarios: desde la última vela del día 0 (su cierre es el fin del día), con sus datos.
    assert_eq!(a.floor_pivots[0].from_index, 95);
    let pp = (daily[0].high + daily[0].low + daily[0].close) / Decimal::from(3);
    assert_eq!(a.floor_pivots[0].pp, pp);
    assert_eq!(a.floor_pivots.len(), 4);
}

/// Lo de una vista con los índices de la serie entera (`base` = dónde empieza la vista) y cada
/// toque del golden pocket con su tramo, para comparar vistas que empiezan en lugares distintos.
#[derive(Debug, PartialEq)]
struct Absolute {
    ema: Vec<(usize, Option<Decimal>)>,
    legs: Vec<(usize, usize, usize, String)>,
    pocket: Vec<(usize, (usize, usize), bool)>,
    wyckoff: Vec<(usize, usize, String, Decimal)>,
    bottoms: Vec<(usize, usize, usize, Decimal)>,
    crosses: Vec<(usize, String)>,
    pivots: Vec<(usize, String)>,
    vix: Vec<(usize, Option<String>)>,
}

impl Absolute {
    /// Solo lo que se refiere a velas en `[from, to)`.
    fn of(a: &ExportAnalysis, base: usize, from: usize, to: usize) -> Self {
        let inside = |i: usize| (from..to).contains(&(i + base));
        let leg_of = |k: usize| {
            let l = &a.fibonacci[k];
            (l.start_index + base, l.end_index + base)
        };
        Self {
            ema: (0..a.ema_daily.len())
                .filter(|&i| inside(i))
                .map(|i| (i + base, a.ema_daily[i]))
                .collect(),
            legs: a
                .fibonacci
                .iter()
                .filter(|l| inside(l.start_index) && inside(l.confirmed_index))
                .map(|l| {
                    (
                        l.start_index + base,
                        l.end_index + base,
                        l.confirmed_index + base,
                        l.direction.clone(),
                    )
                })
                .collect(),
            pocket: a
                .golden_pocket
                .iter()
                .filter(|e| inside(e.index) && inside(a.fibonacci[e.leg].start_index))
                .map(|e| (e.index + base, leg_of(e.leg), e.closes_inside))
                .collect(),
            wyckoff: a
                .wyckoff
                .iter()
                .filter(|w| inside(w.break_index) && inside(w.confirmed_index))
                .map(|w| {
                    (
                        w.break_index + base,
                        w.confirmed_index + base,
                        w.kind.clone(),
                        w.level,
                    )
                })
                .collect(),
            bottoms: a
                .bottoms
                .iter()
                .filter(|b| inside(b.index) && inside(b.confirmed_index))
                .map(|b| {
                    (
                        b.index + base,
                        b.confirmed_index + base,
                        b.rebound_index + base,
                        b.price,
                    )
                })
                .collect(),
            crosses: a
                .ma_cross
                .crosses
                .iter()
                .filter(|c| inside(c.known_from_index))
                .map(|c| (c.known_from_index + base, c.day.clone()))
                .collect(),
            pivots: a
                .floor_pivots
                .iter()
                .filter(|p| inside(p.from_index) && p.from_index + base > from)
                .map(|p| (p.from_index + base, p.day.clone()))
                .collect(),
            vix: (0..a.vix.len())
                .filter(|&i| inside(i))
                .map(|i| (i + base, a.vix[i].as_ref().map(|v| v.close.to_string())))
                .collect(),
        }
    }
}

/// #511: el gateway calcula sobre `warmup` velas antes de la primera visible y devuelve solo la
/// vista. Una vela cerrada da lo mismo aunque la vista empiece en otro lado o termine antes (sin
/// repintado por el borde), y la vista trae las señales cerca de su borde izquierdo.
#[test]
fn a_view_gives_the_same_analysis_wherever_it_starts() {
    let full = random_walk(3200, 11);
    let daily = daily_of(&full);
    let vix = vix_of(&full);
    let cfg = test_cfg(5);
    let strength = setup().strategy.patterns.pivots.strength;
    let warmup = cfg.warmup.m15_bars;
    let view = |first: usize, end: usize| {
        let known: Vec<_> = daily
            .iter()
            .filter(|d| d.end <= full[end - 1].end())
            .cloned()
            .collect();
        let base = first - warmup;
        let a = analysis_view(&full[base..end], warmup, &known, &vix, strength, &cfg);
        assert_eq!(a.ema_daily.len(), end - first, "un valor por vela visible");
        a
    };
    let (fa, ea) = (1100, 2600);
    let (fb, eb) = (1700, 3200);
    let (a, b) = (view(fa, ea), view(fb, eb));
    // En lo que comparten (y que la vista A ya conocía), lo mismo.
    let (sa, sb) = (Absolute::of(&a, fa, fb, ea), Absolute::of(&b, fb, fb, ea));
    assert_eq!(sa, sb);
    assert!(!sa.legs.is_empty() && !sa.pocket.is_empty(), "hay tramos");
    assert!(
        !sa.wyckoff.is_empty() && !sa.bottoms.is_empty(),
        "hay señales"
    );
    assert!(!sa.pivots.is_empty() && sa.vix.iter().any(|v| v.1.is_some()));
    // Y es lo mismo que sale de la serie entera.
    let whole = analysis(&full[..ea], &daily_of(&full[..ea]), 5);
    assert_eq!(Absolute::of(&whole, 0, fb, ea), sa);
    // Cerca del borde izquierdo de la vista también hay señales: sin calentamiento, una vista
    // que empieza ahí no tendría pivotes en sus primeras velas ni niveles de Wyckoff.
    let edge = |x: &Absolute| {
        x.wyckoff.iter().filter(|w| w.0 < fb + 200).count()
            + x.bottoms.iter().filter(|b| b.0 < fb + 200).count()
            + x.legs.iter().filter(|l| l.0 < fb + 200).count()
    };
    let cold = analysis_export(&full[fb..ea], &daily, &vix, strength, &cfg);
    assert!(edge(&sb) > edge(&Absolute::of(&cold, fb, fb, ea)));
    // Y la vista tiene las velas de la vista, nada del calentamiento.
    assert!(b.fibonacci.iter().all(|l| l.end_index < eb - fb));
    assert!(b.floor_pivots.first().is_some_and(|p| p.from_index == 0));
}

#[test]
fn a_view_without_warmup_is_the_same_as_the_export() {
    let m15 = random_walk(800, 4);
    let daily = daily_of(&m15);
    let cfg = test_cfg(5);
    let strength = setup().strategy.patterns.pivots.strength;
    let vix = vix_of(&m15);
    assert_eq!(
        analysis_view(&m15, 0, &daily, &vix, strength, &cfg),
        analysis_export(&m15, &daily, &vix, strength, &cfg)
    );
}
