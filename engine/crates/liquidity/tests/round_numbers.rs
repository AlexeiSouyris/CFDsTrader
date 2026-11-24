//! Números redondos y volumen de la barrida (#497) con series construidas a mano: calentamiento,
//! barrido, roto y cambio de lado, precio justo en el nivel, gaps, vencimiento, sin repintado y
//! velas sin volumen.

use std::num::NonZeroUsize;

use cfds_core::{
    Candle, LiquidityLevel, LiquidityLevelKind as Kind, LiquidityLevelState as State, Timeframe,
};
use cfds_liquidity::{
    LiquidityConfig, RoundNumbersConfig, Side, SweepVolumeConfig, is_active_at, relative_volume,
    round_number_levels, sweeps_at, sweeps_with_volume_at,
};
use cfds_patterns::confirmed_pivots;
use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, dec};

const REAL: &str = include_str!("../../../../config/liquidity.toml");

fn t0() -> DateTime<Utc> {
    "2026-11-16T14:30:00Z".parse().unwrap()
}

fn bar_tf(tf: Timeframe, i: usize, h: Decimal, l: Decimal, c: Decimal) -> Candle {
    Candle {
        instrument_id: "US500".parse().unwrap(),
        timeframe: tf,
        start: t0() + tf.duration() * i as i32,
        open: c,
        high: h,
        low: l,
        close: c,
        volume: None,
    }
}

fn bar(i: usize, h: Decimal, l: Decimal, c: Decimal) -> Candle {
    bar_tf(Timeframe::M15, i, h, l, c)
}

/// `n` velas de 15m quietas alrededor de 5020.
fn quiet(n: usize) -> Vec<Candle> {
    (0..n)
        .map(|i| bar(i, dec!(5022), dec!(5018), dec!(5020)))
        .collect()
}

/// ATR válido desde la vela `from` (antes, calentamiento).
fn atr_from(n: usize, from: usize) -> Vec<Option<Decimal>> {
    (0..n).map(|i| (i >= from).then_some(dec!(5))).collect()
}

fn us500_steps() -> Vec<Decimal> {
    RoundNumbersConfig::from_toml(REAL)
        .unwrap()
        .steps_for(&"US500".parse().unwrap())
        .to_vec()
}

fn levels(c: &[Candle], atr_start: usize, max_age: usize) -> Vec<LiquidityLevel> {
    round_number_levels(c, &atr_from(c.len(), atr_start), &us500_steps(), max_age)
}

fn find(all: &[LiquidityLevel], kind: Kind, price: Decimal) -> Vec<&LiquidityLevel> {
    all.iter()
        .filter(|l| l.kind == kind && l.price == price)
        .collect()
}

#[test]
fn the_repository_config_has_the_steps_of_the_report() {
    let cfg = RoundNumbersConfig::from_toml(REAL).unwrap();
    let steps = |id: &str| cfg.steps_for(&id.parse().unwrap()).to_vec();
    assert_eq!(steps("US500"), vec![dec!(50), dec!(100)]);
    assert_eq!(steps("US100"), vec![dec!(100)]);
    assert_eq!(steps("US30"), vec![dec!(500)]);
    assert_eq!(steps("DE40"), vec![dec!(100)]);
    assert_eq!(steps("XAUUSD"), vec![dec!(10)]);
    // Sin paso en la config: sin números redondos (no se inventa uno).
    assert!(steps("US2000").is_empty());
    assert_eq!(
        SweepVolumeConfig::from_toml(REAL)
            .unwrap()
            .median_bars
            .get(),
        20
    );
}

#[test]
fn rejects_invalid_round_number_and_volume_values() {
    for bad in [
        REAL.replace(r#"US100 = ["100"]"#, r#"US100 = []"#),
        REAL.replace(r#"US100 = ["100"]"#, r#"US100 = ["0"]"#),
        REAL.replace(r#"US100 = ["100"]"#, r#"US100 = ["-100"]"#),
        REAL.replace(r#"US100 = ["100"]"#, r#"US100 = ["cien"]"#),
        REAL.replace(r#"US100 = ["100"]"#, r#"US100 = ["100", "100.0"]"#),
        REAL.replace(r#"US100 = ["100"]"#, r#"us100 = ["100"]"#),
        REAL.replace("median_bars = 20", "median_bars = 0"),
        REAL.replace("median_bars = 20", "median_velas = 20"),
        REAL.replace("[round_numbers]", "[numeros_redondos]"),
    ] {
        assert!(LiquidityConfig::from_toml(&bad).is_err(), "{bad}");
    }
}

#[test]
fn nearest_round_numbers_above_and_below_the_close() {
    let c = quiet(5);
    let all = levels(&c, 0, 500);
    // Nacen en la primera vela: 5050 y 5100 arriba; 5000 abajo (múltiplo de 50 y de 100, uno solo).
    let born_first: Vec<(Kind, Decimal)> = all
        .iter()
        .filter(|l| l.exists_from == c[0].end())
        .map(|l| (l.kind, l.price))
        .collect();
    assert_eq!(
        born_first,
        vec![
            (Kind::RoundHigh, dec!(5050)),
            (Kind::RoundHigh, dec!(5100)),
            (Kind::RoundLow, dec!(5000)),
        ]
    );
    // Mientras siguen activos, no se repiten.
    assert_eq!(all.len(), 3);
    assert!(
        all.iter()
            .all(|l| l.state == State::Intact && l.session.is_none())
    );
}

#[test]
fn nothing_exists_during_warmup() {
    let c = quiet(10);
    assert!(round_number_levels(&c, &[None; 10], &us500_steps(), 500).is_empty());
    let all = levels(&c, 4, 500);
    assert!(all.iter().all(|l| l.exists_from == c[4].end()));
    assert!(round_number_levels(&c, &atr_from(10, 0), &[], 500).is_empty());
    assert!(round_number_levels(&[], &[], &us500_steps(), 500).is_empty());
}

#[test]
fn a_swept_round_high_is_born_again_and_a_broken_one_flips_side() {
    let mut c = quiet(5);
    // Vela 2: pincha 5050 y cierra adentro → barrido.
    c[2] = bar(2, dec!(5055), dec!(5030), dec!(5040));
    // Vela 4: cierra por encima de 5050 → roto.
    c[4] = bar(4, dec!(5065), dec!(5045), dec!(5060));
    let all = levels(&c, 0, 500);
    let highs = find(&all, Kind::RoundHigh, dec!(5050));
    assert_eq!(highs.len(), 2);
    assert_eq!(
        (highs[0].state, highs[0].state_changed_at),
        (State::Swept, Some(c[2].end()))
    );
    // Vuelve a nacer en la misma vela que lo barrió (el anterior ya no está activo).
    assert_eq!(highs[1].exists_from, c[2].end());
    assert_eq!(
        (highs[1].state, highs[1].state_changed_at),
        (State::Broken, Some(c[4].end()))
    );
    // Roto hacia arriba: 5050 pasa a ser un número redondo por debajo del cierre.
    let lows = find(&all, Kind::RoundLow, dec!(5050));
    assert_eq!(lows.len(), 1);
    assert_eq!(lows[0].exists_from, c[4].end());
    // El de 5000 de abajo sigue intacto.
    assert_eq!(
        find(&all, Kind::RoundLow, dec!(5000))[0].state,
        State::Intact
    );
}

#[test]
fn a_close_exactly_on_a_round_number_is_neither_above_nor_below() {
    let mut c = quiet(4);
    // Vela 1: H toca 5050 sin pasarlo → el alto sigue intacto.
    c[1] = bar(1, dec!(5050), dec!(5020), dec!(5045));
    // Vela 2: cierra justo en 5050 con H = 5050 (no lo supera).
    c[2] = bar(2, dec!(5050), dec!(5040), dec!(5050));
    let all = levels(&c, 0, 500);
    let high = find(&all, Kind::RoundHigh, dec!(5050));
    assert_eq!(high.len(), 1);
    assert_eq!(high[0].state, State::Intact);
    // En la vela 2 no nace un round_low en 5050: los vecinos son 5000 (abajo) y 5100 (arriba).
    assert!(find(&all, Kind::RoundLow, dec!(5050)).is_empty());
    // Vela 3 vuelve a 5020 con L = 5018: 5000 no se toca.
    assert_eq!(
        find(&all, Kind::RoundLow, dec!(5000))[0].state,
        State::Intact
    );
}

#[test]
fn a_gap_breaks_every_level_it_jumps_and_only_creates_the_new_neighbors() {
    let mut c = quiet(4);
    // Vela 2: gap hasta 5235 (sin velas en el medio).
    c[2] = bar(2, dec!(5240), dec!(5225), dec!(5235));
    c[3] = bar(3, dec!(5240), dec!(5230), dec!(5235));
    let all = levels(&c, 0, 500);
    for price in [dec!(5050), dec!(5100)] {
        let h = find(&all, Kind::RoundHigh, price);
        assert_eq!(
            (h[0].state, h[0].state_changed_at),
            (State::Broken, Some(c[2].end()))
        );
    }
    // Nacen solo los vecinos del cierre nuevo: 5200 abajo, 5250 y 5300 arriba.
    let born_gap: Vec<(Kind, Decimal)> = all
        .iter()
        .filter(|l| l.exists_from == c[2].end())
        .map(|l| (l.kind, l.price))
        .collect();
    assert_eq!(
        born_gap,
        vec![
            (Kind::RoundHigh, dec!(5250)),
            (Kind::RoundHigh, dec!(5300)),
            (Kind::RoundLow, dec!(5200)),
        ]
    );
    assert!(find(&all, Kind::RoundLow, dec!(5150)).is_empty());
}

#[test]
fn an_intact_level_expires_and_is_born_again() {
    let c = quiet(5);
    let all = levels(&c, 0, 2);
    let high = find(&all, Kind::RoundHigh, dec!(5050));
    // Nace en 0; las velas 1 y 2 no lo tocan → vence al cierre de 2 y renace ahí.
    assert_eq!(high[0].expired_at, Some(c[2].end()));
    assert_eq!(high[1].exists_from, c[2].end());
    assert!(!is_active_at(high[0], c[2].end()));
}

/// Serie que sube y baja cruzando varios números redondos.
fn wave(n: usize) -> Vec<Candle> {
    (0..n)
        .map(|i| {
            let x = Decimal::from((i * 37) % 23) * dec!(7) - dec!(70);
            let close = dec!(5000) + x;
            bar(i, close + dec!(12), close - dec!(9), close)
        })
        .collect()
}

#[test]
fn no_repaint_levels_from_a_prefix_are_the_same_with_more_candles() {
    let c = wave(80);
    let full = levels(&c, 3, 20);
    for cut in 1..c.len() {
        let part = levels(&c[..cut], 3, 20);
        let known = c[cut - 1].end();
        let expected: Vec<&LiquidityLevel> =
            full.iter().filter(|l| l.exists_from <= known).collect();
        assert_eq!(part.len(), expected.len(), "cut {cut}");
        for (p, f) in part.iter().zip(expected) {
            assert_eq!(
                (p.kind, p.price, p.exists_from),
                (f.kind, f.price, f.exists_from)
            );
            if p.state != State::Intact {
                // Un estado ya cambiado no cambia nunca más.
                assert_eq!((p.state, p.state_changed_at), (f.state, f.state_changed_at));
            } else {
                // Intacto con lo que se sabía: el cambio (si hubo) es posterior.
                assert!(f.state_changed_at.is_none_or(|t| t > known));
            }
        }
    }
}

// ---------- Volumen de la barrida ----------

fn with_volume(mut c: Candle, v: Option<Decimal>) -> Candle {
    c.volume = v;
    c
}

fn n(k: usize) -> NonZeroUsize {
    NonZeroUsize::new(k).unwrap()
}

#[test]
fn relative_volume_against_the_median_of_the_previous_bars() {
    let vols = [dec!(10), dec!(30), dec!(20), dec!(1000), dec!(60)];
    let c: Vec<Candle> = vols
        .iter()
        .enumerate()
        .map(|(i, v)| with_volume(bar(i, dec!(1), dec!(1), dec!(1)), Some(*v)))
        .collect();
    // Impar: mediana de 10, 30, 20 = 20 → 1000 / 20.
    assert_eq!(relative_volume(&c, 3, n(3)), Some(dec!(50)));
    // Par: mediana de 30, 20, 1000, 10 ordenados (10, 20, 30, 1000) = 25 → 60 / 25. Sin la propia.
    assert_eq!(relative_volume(&c, 4, n(4)), Some(dec!(2.4)));
    // Calentamiento: no hay 4 velas antes de la 3.
    assert_eq!(relative_volume(&c, 3, n(4)), None);
    assert_eq!(relative_volume(&c, 9, n(1)), None);
}

#[test]
fn without_volume_the_field_stays_empty() {
    let mut c: Vec<Candle> = (0..4)
        .map(|i| with_volume(bar(i, dec!(1), dec!(1), dec!(1)), Some(dec!(10))))
        .collect();
    assert_eq!(relative_volume(&c, 3, n(3)), Some(dec!(1)));
    // Una vela de la ventana sin volumen (IG): vacío.
    c[1].volume = None;
    assert_eq!(relative_volume(&c, 3, n(3)), None);
    // La propia vela sin volumen: vacío.
    c[1].volume = Some(dec!(10));
    c[3].volume = None;
    assert_eq!(relative_volume(&c, 3, n(3)), None);
    // Mediana 0: vacío (no se divide por cero).
    let zero: Vec<Candle> = (0..4)
        .map(|i| with_volume(bar(i, dec!(1), dec!(1), dec!(1)), Some(Decimal::ZERO)))
        .collect();
    assert_eq!(relative_volume(&zero, 3, n(3)), None);
}

/// 10 velas de 1h con un pivote bajo (90) en la 3; con N = 3 se confirma al cierre de la 6.
fn h1_with_low_pivot() -> Vec<Candle> {
    (0..10)
        .map(|i| {
            let l = if i == 3 { dec!(90) } else { dec!(95) };
            bar_tf(Timeframe::H1, i, dec!(97), l, dec!(96))
        })
        .collect()
}

#[test]
fn a_sweep_carries_the_relative_volume_of_its_candle() {
    let h1 = h1_with_low_pivot();
    let piv = confirmed_pivots(&h1, 3);
    let cfg = SweepVolumeConfig { median_bars: n(4) };
    // 29 velas de 15m quietas con volumen 100; la 28 barre 90 con volumen 350.
    let mut m15: Vec<Candle> = (0..29)
        .map(|i| with_volume(bar(i, dec!(97), dec!(95), dec!(96)), Some(dec!(100))))
        .collect();
    m15[28] = with_volume(bar(28, dec!(93), dec!(89), dec!(91)), Some(dec!(350)));
    let s = sweeps_with_volume_at(&m15, 28, &h1, &piv, &cfg);
    assert_eq!(s.len(), 1);
    // Es el mismo sweep de `sweeps_at`: no se reimplementa.
    assert_eq!(s[0].sweep, sweeps_at(&m15[28], &h1, &piv)[0]);
    assert_eq!((s[0].sweep.side, s[0].sweep.level), (Side::Long, dec!(90)));
    assert_eq!(s[0].relative_volume, Some(dec!(3.5)));
    // Sin volumen (velas de IG): el sweep sigue, el volumen queda vacío.
    let no_vol: Vec<Candle> = m15.iter().cloned().map(|k| with_volume(k, None)).collect();
    let s = sweeps_with_volume_at(&no_vol, 28, &h1, &piv, &cfg);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].relative_volume, None);
    // Sin sweep, nada; fuera de rango, nada.
    assert!(sweeps_with_volume_at(&m15, 27, &h1, &piv, &cfg).is_empty());
    assert!(sweeps_with_volume_at(&m15, 99, &h1, &piv, &cfg).is_empty());
    // Sin repintado: con velas posteriores, el valor de la 28 no cambia.
    let mut more = m15.clone();
    more.push(with_volume(
        bar(29, dec!(97), dec!(95), dec!(96)),
        Some(dec!(9999)),
    ));
    assert_eq!(
        sweeps_with_volume_at(&more, 28, &h1, &piv, &cfg)[0].relative_volume,
        Some(dec!(3.5))
    );
}
