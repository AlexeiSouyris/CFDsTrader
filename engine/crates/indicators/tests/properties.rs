//! Tests de propiedades de los indicadores (#663, plan B1 de `docs/analysis/2026-11-autoanalisis.md`).
//!
//! Sobre series de velas generadas al azar (con gaps), se verifica para cada indicador:
//! - **Sin repintado:** agregar velas nunca cambia el valor de una vela ya cerrada. Se compara el
//!   indicador sobre un prefijo de la serie con el mismo indicador sobre la serie entera.
//! - **Calentamiento:** no hay valor antes de tener datos suficientes, y sí lo hay después.
//!
//! Pocos casos por propiedad para que el CI no tarde (todo el archivo corre en menos de 1 s).

use cfds_core::{Candle, Timeframe};
use cfds_indicators::{
    crosses, drawdown_from_high, ema, ema_values, realized_volatility, sma, sma_values, true_range,
    wilder_atr,
};
use chrono::{DateTime, TimeDelta, Utc};
use proptest::prelude::*;
use rust_decimal::Decimal;

const CASES: u32 = 48;

/// Una vela en centésimos: (gap contra el cierre anterior, cuerpo, mecha superior, mecha inferior).
type Step = (i64, i64, i64, i64);

fn step() -> impl Strategy<Value = Step> {
    (
        prop_oneof![4 => Just(0i64), 1 => -3_000i64..3_000],
        -2_000i64..2_000,
        0i64..1_500,
        0i64..1_500,
    )
}

/// Velas de 15m consecutivas desde 10 000,00: siempre positivas (como mucho 150 velas de ±50,00).
fn build(steps: &[Step]) -> Vec<Candle> {
    let t0: DateTime<Utc> = "2026-11-19T14:30:00Z".parse().unwrap();
    let mut prev_close = Decimal::new(1_000_000, 2);
    steps
        .iter()
        .enumerate()
        .map(|(i, &(gap, body, up, down))| {
            let open = prev_close + Decimal::new(gap, 2);
            let close = open + Decimal::new(body, 2);
            prev_close = close;
            Candle {
                instrument_id: "US100".parse().unwrap(),
                timeframe: Timeframe::M15,
                start: t0 + TimeDelta::minutes(15 * i as i64),
                open,
                high: open.max(close) + Decimal::new(up, 2),
                low: open.min(close) - Decimal::new(down, 2),
                close,
                volume: None,
            }
        })
        .collect()
}

/// Serie de velas y un corte `k` en `1..=len` (el prefijo `..k` es lo que se sabía antes).
fn series_and_cut() -> impl Strategy<Value = (Vec<Candle>, usize)> {
    prop::collection::vec(step(), 1..150).prop_flat_map(|steps| {
        let n = steps.len();
        (Just(build(&steps)), 1..=n)
    })
}

/// `f(prefijo)` tiene que ser exactamente el comienzo de `f(serie entera)`.
fn assert_no_repaint<T, U>(full: &[T], k: usize, f: impl Fn(&[T]) -> Vec<U>)
where
    U: PartialEq + std::fmt::Debug,
{
    let whole = f(full);
    let prefix = f(&full[..k]);
    assert_eq!(prefix.len(), k);
    assert_eq!(prefix.as_slice(), &whole[..k]);
}

fn closes(candles: &[Candle]) -> Vec<Decimal> {
    candles.iter().map(|c| c.close).collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    #[test]
    fn indicators_never_repaint((candles, k) in series_and_cut(), period in 1usize..30) {
        assert_no_repaint(&candles, k, true_range);
        assert_no_repaint(&candles, k, |c| wilder_atr(c, period));
        assert_no_repaint(&candles, k, |c| sma(c, period));
        assert_no_repaint(&candles, k, |c| ema(c, period));
        let values = closes(&candles);
        assert_no_repaint(&values, k, |v| sma_values(v, period));
        assert_no_repaint(&values, k, |v| ema_values(v, period));
        assert_no_repaint(&values, k, |v| drawdown_from_high(v, period));
        assert_no_repaint(&values, k, |v| realized_volatility(v, period.max(2)));
    }

    #[test]
    fn warm_up_gives_no_value_before_enough_data(
        (candles, _) in series_and_cut(),
        period in 1usize..30,
    ) {
        let values = closes(&candles);
        // ATR, SMA, EMA y caída desde el máximo: primer valor en la vela `period − 1`.
        for series in [
            wilder_atr(&candles, period),
            sma(&candles, period),
            ema(&candles, period),
            drawdown_from_high(&values, period),
        ] {
            for (i, v) in series.iter().enumerate() {
                prop_assert_eq!(v.is_some(), i + 1 >= period, "vela {} período {}", i, period);
            }
        }
        // Volatilidad realizada: necesita `window` retornos, o sea `window + 1` cierres.
        let window = period.max(2);
        for (i, v) in realized_volatility(&values, window).iter().enumerate() {
            prop_assert_eq!(v.is_some(), i >= window, "vela {} ventana {}", i, window);
        }
    }

    #[test]
    fn atr_and_ema_stay_within_the_data(
        (candles, _) in series_and_cut(),
        period in 1usize..30,
    ) {
        let tr = true_range(&candles);
        for (i, atr) in wilder_atr(&candles, period).iter().enumerate() {
            if let Some(atr) = atr {
                let seen = &tr[..=i];
                prop_assert!(*atr >= *seen.iter().min().unwrap());
                prop_assert!(*atr <= *seen.iter().max().unwrap());
            }
        }
        let values = closes(&candles);
        for (i, e) in ema_values(&values, period).iter().enumerate() {
            if let Some(e) = e {
                let seen = &values[..=i];
                prop_assert!(*e >= *seen.iter().min().unwrap());
                prop_assert!(*e <= *seen.iter().max().unwrap());
            }
        }
    }

    #[test]
    fn crosses_never_repaint(
        (candles, k) in series_and_cut(),
        fast in 1usize..10,
        slow in 10usize..30,
    ) {
        let values = closes(&candles);
        let at = |v: &[Decimal]| crosses(&sma_values(v, fast), &sma_values(v, slow));
        let whole = at(&values);
        let prefix = at(&values[..k]);
        let known: Vec<_> = whole.into_iter().filter(|c| c.index < k).collect();
        prop_assert_eq!(prefix, known);
    }
}
