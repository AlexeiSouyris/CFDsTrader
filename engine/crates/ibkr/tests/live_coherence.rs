//! Coherencia de la vela en formación con las velas históricas de IBKR (#558), con datos dados (sin
//! red ni base): velas de 1 minuto reales de MNQZ6 guardadas en `ibkr_bars` (25-11-2026, 02:00 a
//! 02:14 UTC). Con un precio en cada apertura, máximo, mínimo y cierre de esas velas, la vela en
//! formación al cerrar es exactamente la vela histórica (1m) y su agregado (5m y 15m). Con precios
//! muestreados (como llegan de IBKR: medido cada ~5 s en demorados), queda dentro del rango de la
//! histórica.

use cfds_ibkr::{BarUpdate, LiveBar, LiveDataType, LiveTimeframe, QuoteEvent, QuoteState};
use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use rust_decimal::Decimal;
use rust_decimal::dec;

/// (minuto, apertura, máximo, mínimo, cierre) de las velas de 1m de MNQZ6, 02:00 a 02:14 UTC.
const M1: [(u32, Decimal, Decimal, Decimal, Decimal); 15] = [
    (0, dec!(26068.5), dec!(26083), dec!(26060), dec!(26065.5)),
    (
        1,
        dec!(26066.5),
        dec!(26077.75),
        dec!(26059.25),
        dec!(26060.5),
    ),
    (2, dec!(26060.75), dec!(26070), dec!(26054.25), dec!(26066)),
    (3, dec!(26066), dec!(26093.75), dec!(26064), dec!(26074)),
    (
        4,
        dec!(26073.25),
        dec!(26080.25),
        dec!(26063.75),
        dec!(26068.25),
    ),
    (5, dec!(26068.25), dec!(26071), dec!(26050), dec!(26061.5)),
    (
        6,
        dec!(26061),
        dec!(26073.25),
        dec!(26053.25),
        dec!(26056.5),
    ),
    (
        7,
        dec!(26056.25),
        dec!(26058.5),
        dec!(26035.75),
        dec!(26039.5),
    ),
    (
        8,
        dec!(26040.25),
        dec!(26047.75),
        dec!(26036),
        dec!(26040.25),
    ),
    (9, dec!(26041), dec!(26043.5), dec!(26029.75), dec!(26036)),
    (
        10,
        dec!(26036.75),
        dec!(26048.25),
        dec!(26028),
        dec!(26035.25),
    ),
    (
        11,
        dec!(26035),
        dec!(26037.5),
        dec!(26022.75),
        dec!(26029.5),
    ),
    (
        12,
        dec!(26030.75),
        dec!(26049.75),
        dec!(26028.25),
        dec!(26034.75),
    ),
    (13, dec!(26032.75), dec!(26042), dec!(26030), dec!(26033)),
    (14, dec!(26035.25), dec!(26051.5), dec!(26033), dec!(26050)),
];

fn at(h: u32, m: u32, s: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 25, h, m, s).unwrap()
}

/// Un operado: su precio y su hora, como los manda IBKR (precio y después la hora).
fn trade(s: &mut QuoteState, price: Decimal, t: DateTime<Utc>, delay: TimeDelta) -> Vec<BarUpdate> {
    let received = t + delay;
    let mut out = s.on_clock(received);
    out.extend(s.on_event(QuoteEvent::Last { price, received }));
    out.extend(s.on_event(QuoteEvent::LastTime { at: t, received }));
    out
}

fn state(delay: TimeDelta, data_type: LiveDataType) -> QuoteState {
    let mut s = QuoteState::new(&LiveTimeframe::ALL, TimeDelta::seconds(1));
    s.on_event(QuoteEvent::Contract("MNQZ6".into()));
    s.on_event(QuoteEvent::DataType(data_type));
    // El último operado previo a la suscripción (01:59:59): no arma velas.
    trade(&mut s, dec!(26070), at(1, 59, 59), delay);
    s
}

/// Las velas cerradas de cada timeframe, con un precio en (segundo, campo) de cada vela de 1m.
fn closed_with(points: &[(u32, usize)], delay: TimeDelta, data_type: LiveDataType) -> Vec<LiveBar> {
    let mut s = state(delay, data_type);
    let mut closed = Vec::new();
    for (m, o, h, l, c) in M1 {
        for &(sec, field) in points {
            let p = [o, h, l, c][field];
            closed.extend(trade(&mut s, p, at(2, m, sec), delay));
        }
    }
    // Fin del período sin más precios: el reloj las cierra (a la hora de los datos).
    closed.extend(s.on_clock(at(2, 15, 2) + delay));
    closed
        .into_iter()
        .filter(|u| !u.forming)
        .map(|u| u.bar)
        .collect()
}

fn find(bars: &[LiveBar], tf: LiveTimeframe, start: DateTime<Utc>) -> &LiveBar {
    bars.iter()
        .find(|b| b.timeframe == tf && b.start == start)
        .unwrap_or_else(|| panic!("falta la vela {} de {start}", tf.as_str()))
}

fn ohlc(b: &LiveBar) -> (Decimal, Decimal, Decimal, Decimal) {
    (b.open, b.high, b.low, b.close)
}

/// Agregado de las velas históricas de 1m `[from, to)`.
fn aggregate(from: u32, to: u32) -> (Decimal, Decimal, Decimal, Decimal) {
    let part = &M1[from as usize..to as usize];
    (
        part[0].1,
        part.iter().map(|b| b.2).max().unwrap(),
        part.iter().map(|b| b.3).min().unwrap(),
        part[part.len() - 1].4,
    )
}

#[test]
fn every_price_gives_exactly_the_historical_bars() {
    // Apertura al segundo 0, máximo al 15, mínimo al 30, cierre al 59.
    let points = [(0, 0), (15, 1), (30, 2), (59, 3)];
    for (delay, data_type) in [
        (TimeDelta::zero(), LiveDataType::Live),
        (TimeDelta::seconds(600), LiveDataType::Delayed),
    ] {
        let closed = closed_with(&points, delay, data_type);
        for (m, o, h, l, c) in M1 {
            let b = find(&closed, LiveTimeframe::M1, at(2, m, 0));
            assert_eq!(ohlc(b), (o, h, l, c), "1m {m}");
            assert!(!b.partial);
        }
        for k in 0..3 {
            let b = find(&closed, LiveTimeframe::M5, at(2, 5 * k, 0));
            assert_eq!(ohlc(b), aggregate(5 * k, 5 * k + 5), "5m {k}");
        }
        let b = find(&closed, LiveTimeframe::M15, at(2, 0, 0));
        assert_eq!(
            ohlc(b),
            (dec!(26068.5), dec!(26093.75), dec!(26022.75), dec!(26050))
        );
        // La de 1h sigue en formación: no se cierra antes de su fin.
        assert!(closed.iter().all(|b| b.timeframe != LiveTimeframe::H1));
    }
}

#[test]
fn sampled_prices_stay_inside_the_historical_bar() {
    // Como llegan de IBKR: algunos precios por minuto, no todos los operados.
    let points = [(10, 0), (20, 2), (40, 1), (55, 3)];
    let closed = closed_with(&points, TimeDelta::seconds(600), LiveDataType::Delayed);
    for (m, _, h, l, _) in M1 {
        let b = find(&closed, LiveTimeframe::M1, at(2, m, 0));
        for p in [b.open, b.high, b.low, b.close] {
            assert!(l <= p && p <= h, "1m {m}: {p} fuera de [{l}, {h}]");
        }
    }
    let b = find(&closed, LiveTimeframe::M15, at(2, 0, 0));
    for p in [b.open, b.high, b.low, b.close] {
        assert!(dec!(26022.75) <= p && p <= dec!(26093.75));
    }
}

#[test]
fn a_closed_bar_never_changes_with_later_prices() {
    let points = [(0, 0), (15, 1), (30, 2), (59, 3)];
    let mut s = state(TimeDelta::zero(), LiveDataType::Live);
    let mut first = None;
    for (m, o, h, l, c) in M1 {
        for &(sec, field) in &points {
            let p = [o, h, l, c][field];
            for u in trade(&mut s, p, at(2, m, sec), TimeDelta::zero()) {
                if !u.forming && u.bar.timeframe == LiveTimeframe::M1 && first.is_none() {
                    first = Some(u.bar);
                }
            }
        }
    }
    let first = first.expect("se cerró la de 02:00");
    // Un precio tardío de 02:00 llega después: la vela cerrada no cambia ni se vuelve a emitir.
    let late = trade(&mut s, dec!(1), at(2, 0, 30), TimeDelta::seconds(900));
    assert!(
        late.iter()
            .all(|u| u.bar.timeframe != LiveTimeframe::M1 || u.bar.start != first.start)
    );
    assert_eq!(ohlc(&first), (M1[0].1, M1[0].2, M1[0].3, M1[0].4));
}
