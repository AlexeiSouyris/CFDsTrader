//! La pantalla entera como líneas de texto con tono (verde, rojo, gris...), sin tocar la terminal:
//! así el armado se prueba sin red y sin terminal. El binario `bolsa` solo pinta estas líneas.
//!
//! Distribución de terminal de trading:
//!
//! ```text
//! barra de título ─────────────────────────────────────────────── hora UTC
//! WATCHLIST           │ MNQ (MNQZ6) · 15m            [1m] 15m 1h 1d
//! ▶ MNQ 26160.00 -0.44│ │█                                    26180.00
//!   MES  7193.25 -0.34│ █│ │                                   26150.00
//! ─ INSTRUMENTO ──────│   █ ...
//! Último / Var. / OHLC│
//! Rango del día / Vol │ 25-11 01:00 UTC                 25-11 01:30 UTC
//! ● IB Gateway · fuente · demora · próximo pedido · teclas
//! nota: solo precios, sin indicadores ni señales
//! ```
//!
//! En una terminal angosta (menos de [`MIN_SPLIT_WIDTH`] columnas) los paneles van uno debajo
//! del otro.

use cfds_ibkr::{BarSize, RawBar};
use chrono::{DateTime, TimeDelta, Utc};

use crate::chart;
use crate::quote::{Basis, DayStats, Quote, fmt_age, fmt_price, fmt_signed};

/// Tono de un pedazo de texto. Sin colores (`NO_COLOR`), todos se pintan igual: el texto ya dice
/// lo mismo (signo de la variación, marcador de selección, cuerpo `░` de las velas bajistas).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Up,
    Down,
    Dim,
    Warn,
    Accent,
    /// Barra de título y de estado (texto sobre fondo).
    Bar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub tone: Tone,
}

impl Span {
    pub fn new(text: impl Into<String>, tone: Tone) -> Self {
        Self {
            text: text.into(),
            tone,
        }
    }
}

pub type Line = Vec<Span>;

pub fn line_text(l: &Line) -> String {
    l.iter().map(|s| s.text.as_str()).collect()
}

/// Ancho en columnas (todos los caracteres que se usan ocupan una).
pub fn line_width(l: &Line) -> usize {
    l.iter().map(|s| s.text.chars().count()).sum()
}

/// Recorta una línea a `width` columnas.
pub fn truncate(l: Line, width: usize) -> Line {
    let mut left = width;
    let mut out = Vec::new();
    for s in l {
        if left == 0 {
            break;
        }
        let n = s.text.chars().count();
        if n <= left {
            left -= n;
            out.push(s);
        } else {
            out.push(Span::new(
                s.text.chars().take(left).collect::<String>(),
                s.tone,
            ));
            left = 0;
        }
    }
    out
}

/// Recorta y rellena con espacios hasta exactamente `width` columnas.
pub fn fit(l: Line, width: usize, fill: Tone) -> Line {
    let mut l = truncate(l, width);
    let w = line_width(&l);
    if w < width {
        l.push(Span::new(" ".repeat(width - w), fill));
    }
    l
}

/// Una fila de la watchlist, con lo que hace falta para el panel del instrumento.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Row {
    pub id: String,
    pub contract: Option<String>,
    pub quote: Option<Quote>,
    pub day: Option<DayStats>,
    pub error: Option<String>,
}

/// Lo que se muestra del gráfico elegido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartView<'a> {
    pub contract: Option<&'a str>,
    pub source: Option<&'a str>,
    pub size: BarSize,
    pub sizes: &'a [BarSize],
    pub bars: Option<&'a [RawBar]>,
    pub error: Option<&'a str>,
}

/// Estado de la conexión, para la barra de abajo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    Connected { address: String },
    KillSwitch { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen<'a> {
    pub now: DateTime<Utc>,
    pub rows: &'a [Row],
    pub selected: usize,
    pub chart: ChartView<'a>,
    pub link: Link,
    /// Lo que está haciendo ahora ("pidiendo velas…", "próximo pedido en 12 s").
    pub activity: String,
    pub mono: bool,
}

/// Ancho de la columna izquierda (watchlist e instrumento).
pub const LEFT_WIDTH: usize = 36;
/// Por debajo de este ancho, los paneles van apilados.
pub const MIN_SPLIT_WIDTH: usize = LEFT_WIDTH + 1 + 40;

/// Texto fijo del pie: qué son los datos y qué no son.
pub const NOTE: &str = "Solo precios: sin indicadores ni señales · IBKR paper con demora (~10 min, \
sin suscripción de datos) · Var. día: último contra el cierre de la sesión anterior";

pub const KEYS: &str = "↑↓ instrumento  ←→ timeframe  q salir";

fn pad(s: &str, w: usize) -> String {
    format!("{s:<w$}")
}

fn rpad(s: &str, w: usize) -> String {
    format!("{s:>w$}")
}

fn change_tone(c: rust_decimal::Decimal) -> Tone {
    if c.is_zero() {
        Tone::Plain
    } else if c.is_sign_negative() {
        Tone::Down
    } else {
        Tone::Up
    }
}

fn when_short(q: &Quote) -> String {
    match q.basis {
        Basis::Minute => q.at.format("%H:%M").to_string(),
        Basis::Daily => (q.at - TimeDelta::days(1)).format("%d-%m").to_string(),
    }
}

fn when_long(q: &Quote) -> String {
    match q.basis {
        Basis::Minute => q.at.format("%d-%m %H:%M UTC").to_string(),
        Basis::Daily => format!(
            "cierre diario {}",
            (q.at - TimeDelta::days(1)).format("%d-%m-%Y")
        ),
    }
}

fn section(title: &str, width: usize) -> Line {
    let head = format!("─ {title} ");
    let rest = width.saturating_sub(head.chars().count());
    vec![Span::new(format!("{head}{}", "─".repeat(rest)), Tone::Dim)]
}

/// Watchlist: marcador, instrumento, último, variación %, hora de la última vela.
fn watchlist(s: &Screen<'_>, width: usize) -> Vec<Line> {
    let mut out = vec![
        section("WATCHLIST", width),
        vec![Span::new(
            format!(
                "  {}{}{}{}",
                pad("Instr.", 7),
                rpad("Último", 11),
                rpad("Var.%", 9),
                rpad("Hora", 7)
            ),
            Tone::Dim,
        )],
    ];
    for (i, r) in s.rows.iter().enumerate() {
        let sel = i == s.selected;
        let mut l = vec![
            Span::new(if sel { "▶ " } else { "  " }, Tone::Accent),
            Span::new(pad(&r.id, 7), if sel { Tone::Accent } else { Tone::Plain }),
        ];
        match (&r.quote, &r.error) {
            (Some(q), _) => {
                l.push(Span::new(rpad(&fmt_price(q.last), 11), Tone::Plain));
                match (q.change(), q.change_pct()) {
                    (Some(c), Some(p)) => l.push(Span::new(
                        rpad(&format!("{}%", fmt_signed(p)), 9),
                        change_tone(c),
                    )),
                    _ => l.push(Span::new(rpad("—", 9), Tone::Dim)),
                }
                l.push(Span::new(rpad(&when_short(q), 7), Tone::Dim));
                if r.error.is_some() {
                    l.push(Span::new(" !", Tone::Warn));
                }
            }
            (None, Some(_)) => l.push(Span::new(" error (ver abajo)", Tone::Warn)),
            (None, None) => l.push(Span::new(" cargando…", Tone::Dim)),
        }
        out.push(l);
    }
    out
}

fn kv(k: &str, v: Line) -> Line {
    let mut l = vec![Span::new(pad(k, 12), Tone::Dim)];
    l.extend(v);
    l
}

/// Panel del instrumento elegido: último, variación, referencia, OHLC y volumen de la última vela
/// del gráfico, y rango y volumen de la sesión.
fn info(s: &Screen<'_>, width: usize) -> Vec<Line> {
    let Some(r) = s.rows.get(s.selected) else {
        return Vec::new();
    };
    let mut out = vec![section(&r.id, width)];
    let mut title = vec![Span::new(
        r.contract.clone().unwrap_or_else(|| "-".into()),
        Tone::Plain,
    )];
    if let Some(src) = s.chart.source {
        title.push(Span::new(format!(" · fuente {src}"), Tone::Dim));
    }
    out.push(title);
    if let Some(q) = &r.quote {
        out.push(kv(
            "Último",
            vec![Span::new(fmt_price(q.last), Tone::Plain)],
        ));
        match (q.change(), q.change_pct(), q.reference) {
            (Some(c), Some(p), Some(rf)) => {
                out.push(kv(
                    "Var. día",
                    vec![Span::new(
                        format!("{} ({}%)", fmt_signed(c), fmt_signed(p)),
                        change_tone(c),
                    )],
                ));
                out.push(kv(
                    "Cierre ant.",
                    vec![Span::new(fmt_price(rf), Tone::Plain)],
                ));
            }
            _ => out.push(kv("Var. día", vec![Span::new("sin referencia", Tone::Dim)])),
        }
        out.push(kv(
            "Hora",
            vec![
                Span::new(when_long(q), Tone::Plain),
                Span::new(format!(" ({})", fmt_age(s.now - q.at)), Tone::Dim),
            ],
        ));
    }
    if let Some(d) = &r.day {
        out.push(kv(
            "Rango día",
            vec![Span::new(
                format!("{} – {}", fmt_price(d.low), fmt_price(d.high)),
                Tone::Plain,
            )],
        ));
        if let Some(v) = d.volume {
            out.push(kv(
                "Vol. día",
                vec![Span::new(v.normalize().to_string(), Tone::Plain)],
            ));
        }
    }
    if let Some(b) = s.chart.bars.and_then(|b| b.last()) {
        let label = match s.chart.size {
            BarSize::D1 => b.start.format("%d-%m").to_string(),
            _ => b.start.format("%H:%M").to_string(),
        };
        out.push(kv(
            &format!("Vela {}", s.chart.size.as_str()),
            vec![Span::new(format!("{label} UTC"), Tone::Dim)],
        ));
        let tone = if b.close >= b.open {
            Tone::Up
        } else {
            Tone::Down
        };
        out.push(vec![
            Span::new(" Ap  ", Tone::Dim),
            Span::new(pad(&fmt_price(b.open), 11), tone),
            Span::new(" Máx ", Tone::Dim),
            Span::new(fmt_price(b.high), tone),
        ]);
        out.push(vec![
            Span::new(" Cie ", Tone::Dim),
            Span::new(pad(&fmt_price(b.close), 11), tone),
            Span::new(" Mín ", Tone::Dim),
            Span::new(fmt_price(b.low), tone),
        ]);
        if let Some(v) = b.volume {
            out.push(kv(
                " Vol.",
                vec![Span::new(v.normalize().to_string(), Tone::Plain)],
            ));
        }
    }
    if let Some(e) = &r.error {
        out.push(vec![Span::new(e.clone(), Tone::Warn)]);
    }
    out
}

fn chart_title(id: &str, c: &ChartView<'_>, width: usize) -> Line {
    let mut left = vec![Span::new(id.to_owned(), Tone::Accent)];
    if let Some(k) = c.contract {
        left.push(Span::new(format!(" ({k})"), Tone::Dim));
    }
    let count = c
        .bars
        .map(|b| format!(" · {} velas", b.len()))
        .unwrap_or_default();
    left.push(Span::new(
        format!(" · {}{count}", c.size.as_str()),
        Tone::Plain,
    ));
    let mut right: Line = Vec::new();
    for s in c.sizes {
        if *s == c.size {
            right.push(Span::new(format!("[{}]", s.as_str()), Tone::Accent));
        } else {
            right.push(Span::new(format!(" {} ", s.as_str()), Tone::Dim));
        }
    }
    let gap = width.saturating_sub(line_width(&left) + line_width(&right));
    left.push(Span::new(" ".repeat(gap.max(1)), Tone::Plain));
    left.extend(right);
    left
}

fn chart_panel(s: &Screen<'_>, width: usize, height: usize) -> Vec<Line> {
    let id = s.rows.get(s.selected).map(|r| r.id.as_str()).unwrap_or("-");
    let mut out = vec![chart_title(id, &s.chart, width)];
    let h = height.saturating_sub(1);
    let body = match (s.chart.bars, s.chart.error) {
        (_, Some(e)) => vec![vec![Span::new(e, Tone::Warn)]],
        (Some(b), None) => chart::render(b, s.chart.size, width, h, s.mono),
        (None, None) => vec![vec![Span::new("cargando el gráfico…", Tone::Dim)]],
    };
    out.extend(body);
    out
}

fn title_bar(s: &Screen<'_>, width: usize) -> Line {
    let left = " CFDsTrader · bolsa │ IBKR paper · solo lectura ";
    let right = format!(" {} ", s.now.format("%d-%m-%Y %H:%M:%S UTC"));
    let gap = width.saturating_sub(left.chars().count() + right.chars().count());
    vec![Span::new(
        format!("{left}{}{right}", " ".repeat(gap)),
        Tone::Bar,
    )]
}

fn status_bar(s: &Screen<'_>) -> Line {
    let mut l = match &s.link {
        Link::Connected { address } => vec![
            Span::new(" ● ", Tone::Up),
            Span::new(format!("IB Gateway {address} conectado"), Tone::Plain),
        ],
        Link::KillSwitch { reason } => vec![
            Span::new(" ● ", Tone::Down),
            Span::new(
                format!("KILL SWITCH: {reason} — no se pide nada más"),
                Tone::Warn,
            ),
        ],
    };
    let fresh = s
        .rows
        .get(s.selected)
        .and_then(|r| r.quote.as_ref())
        .map(|q| format!(" │ demora {}", fmt_age(s.now - q.at)))
        .unwrap_or_default();
    let src = s
        .chart
        .source
        .map(|x| format!(" │ fuente {x}"))
        .unwrap_or_default();
    l.push(Span::new(
        format!("{src}{fresh} │ {}", s.activity),
        Tone::Dim,
    ));
    l
}

/// Une dos columnas: la izquierda rellenada a `lw`, un separador `│` y la derecha.
fn columns(left: Vec<Line>, lw: usize, right: Vec<Line>, rw: usize, height: usize) -> Vec<Line> {
    let mut l = left.into_iter();
    let mut r = right.into_iter();
    (0..height)
        .map(|_| {
            let mut line = fit(l.next().unwrap_or_default(), lw, Tone::Plain);
            line.push(Span::new("│", Tone::Dim));
            line.extend(truncate(r.next().unwrap_or_default(), rw));
            line
        })
        .collect()
}

/// Arma la pantalla para una terminal de `width` × `height`. Nunca devuelve más líneas que
/// `height` ni líneas más anchas que `width`.
pub fn render(s: &Screen<'_>, width: usize, height: usize) -> Vec<Line> {
    let footer = vec![
        status_bar(s),
        vec![
            Span::new(format!(" {KEYS}"), Tone::Accent),
            Span::new(format!(" │ {NOTE}"), Tone::Dim),
        ],
    ];
    let body_h = height.saturating_sub(1 + footer.len());
    let mut out = vec![title_bar(s, width)];
    if width >= MIN_SPLIT_WIDTH {
        let mut left = watchlist(s, LEFT_WIDTH);
        left.push(Vec::new());
        left.extend(info(s, LEFT_WIDTH));
        let rw = width - LEFT_WIDTH - 1;
        let right = chart_panel(s, rw, body_h);
        out.extend(columns(left, LEFT_WIDTH, right, rw, body_h));
    } else {
        let mut body = watchlist(s, width);
        body.extend(info(s, width));
        let rest = body_h.saturating_sub(body.len());
        if rest >= 4 {
            body.extend(chart_panel(s, width, rest));
        }
        body.resize(body_h, Vec::new());
        out.extend(body);
    }
    out.extend(footer);
    out.truncate(height);
    out.into_iter().map(|l| truncate(l, width)).collect()
}

/// Versión de una sola vez (`--once`): la watchlist como texto, sin gráfico ni colores.
pub fn render_table(now: DateTime<Utc>, rows: &[Row]) -> Vec<String> {
    let mut out = vec![format!(
        "CFDsTrader · bolsa · IBKR paper, solo lectura · {}",
        now.format("%d-%m-%Y %H:%M:%S UTC")
    )];
    out.push(format!(
        "{}{}{}{}  {}",
        pad("Instr.", 8),
        pad("Contrato", 10),
        rpad("Último", 11),
        rpad("Var. día", 20),
        "Última vela (demora)"
    ));
    for r in rows {
        let mut l = format!(
            "{}{}",
            pad(&r.id, 8),
            pad(r.contract.as_deref().unwrap_or("-"), 10)
        );
        match &r.quote {
            Some(q) => {
                let ch = match (q.change(), q.change_pct()) {
                    (Some(c), Some(p)) => format!("{} ({}%)", fmt_signed(c), fmt_signed(p)),
                    _ => "sin referencia".into(),
                };
                l.push_str(&format!(
                    "{}{}  {} ({})",
                    rpad(&fmt_price(q.last), 11),
                    rpad(&ch, 20),
                    when_long(q),
                    fmt_age(now - q.at)
                ));
            }
            None => l.push_str("sin datos"),
        }
        if let Some(e) = &r.error {
            l.push_str(&format!("  [{e}]"));
        }
        out.push(l);
    }
    out.push(NOTE.to_owned());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rust_decimal::{Decimal, dec};

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 25, 14, 42, 5).unwrap()
    }

    fn rows() -> Vec<Row> {
        vec![
            Row {
                id: "MNQ".into(),
                contract: Some("MNQZ6".into()),
                quote: Some(Quote {
                    last: dec!(26222.75),
                    at: Utc.with_ymd_and_hms(2026, 11, 25, 14, 31, 0).unwrap(),
                    basis: Basis::Minute,
                    reference: Some(dec!(26275.75)),
                }),
                day: Some(DayStats {
                    session: now().date_naive(),
                    high: dec!(26351.75),
                    low: dec!(26087),
                    volume: Some(dec!(257647)),
                }),
                error: None,
            },
            Row {
                id: "QQQ".into(),
                contract: Some("QQQ".into()),
                quote: Some(Quote {
                    last: dec!(512.3),
                    at: Utc.with_ymd_and_hms(2026, 11, 22, 0, 0, 0).unwrap(),
                    basis: Basis::Daily,
                    reference: Some(dec!(500)),
                }),
                day: None,
                error: None,
            },
            Row {
                id: "EURUSD".into(),
                error: Some("IBKR rechazó el pedido: sin permiso".into()),
                ..Row::default()
            },
            Row {
                id: "MES".into(),
                ..Row::default()
            },
        ]
    }

    fn bars() -> Vec<RawBar> {
        (0..30)
            .map(|i| {
                let p = dec!(100) + Decimal::from(i % 7);
                RawBar {
                    start: now() - TimeDelta::minutes(15 * (31 - i)),
                    open: p,
                    high: p + dec!(2),
                    low: p - dec!(1),
                    close: if i % 2 == 0 {
                        p + dec!(1)
                    } else {
                        p - dec!(0.5)
                    },
                    volume: Some(dec!(42)),
                }
            })
            .collect()
    }

    fn screen<'a>(rows: &'a [Row], bars: &'a [RawBar], sizes: &'a [BarSize]) -> Screen<'a> {
        Screen {
            now: now(),
            rows,
            selected: 0,
            chart: ChartView {
                contract: Some("MNQZ6"),
                source: Some("ibkr_paper"),
                size: BarSize::M15,
                sizes,
                bars: Some(bars),
                error: None,
            },
            link: Link::Connected {
                address: "127.0.0.1:4002".into(),
            },
            activity: "próximo pedido en 12 s".into(),
            mono: false,
        }
    }

    const SIZES: [BarSize; 4] = [BarSize::M1, BarSize::M15, BarSize::H1, BarSize::D1];

    #[test]
    fn fills_the_terminal_without_overflowing() {
        let rows = rows();
        let bars = bars();
        for (w, h) in [(160, 50), (120, 40), (80, 24), (60, 30), (20, 5), (0, 0)] {
            let lines = render(&screen(&rows, &bars, &SIZES), w, h);
            assert_eq!(lines.len(), h, "{w}x{h}");
            for l in &lines {
                assert!(line_width(l) <= w, "{w}x{h}: {:?}", line_text(l));
            }
        }
    }

    fn text(w: usize, h: usize, s: &Screen<'_>) -> Vec<String> {
        render(s, w, h).iter().map(line_text).collect()
    }

    #[test]
    fn trading_layout_watchlist_left_chart_right_status_bottom() {
        let rows = rows();
        let bars = bars();
        let s = screen(&rows, &bars, &SIZES);
        let t = text(140, 40, &s);
        assert!(
            t[0].contains("IBKR paper · solo lectura") && t[0].contains("25-11-2026 14:42:05 UTC")
        );
        // Watchlist a la izquierda; el gráfico a la derecha del separador.
        assert!(t[1].starts_with("─ WATCHLIST"));
        assert!(t[1].contains("│MNQ (MNQZ6) · 15m · 30 velas"), "{}", t[1]);
        assert!(t[1].trim_end().ends_with("[15m] 1h  1d"), "{}", t[1]);
        let mnq = t.iter().find(|l| l.starts_with("▶ MNQ")).unwrap();
        assert!(mnq.contains("26222.75") && mnq.contains("-0.20%") && mnq.contains("14:31"));
        assert_eq!(mnq.chars().nth(LEFT_WIDTH), Some('│'));
        let qqq = t.iter().find(|l| l.starts_with("  QQQ")).unwrap();
        assert!(qqq.contains("+2.46%") && qqq.contains("21-11"), "{qqq}");
        assert!(
            t.iter()
                .any(|l| l.starts_with("  EURUSD") && l.contains("error"))
        );
        assert!(
            t.iter()
                .any(|l| l.starts_with("  MES") && l.contains("cargando"))
        );
        // Panel del instrumento.
        let joined = t.join("\n");
        for want in [
            "─ MNQ ",
            "MNQZ6 · fuente ibkr_paper",
            "Var. día    -53.00 (-0.20%)",
            "Cierre ant. 26275.75",
            "Hora        25-11 14:31 UTC (11 min)",
            "Rango día   26087.00 – 26351.75",
            "Vol. día    257647",
            "Vela 15m    14:12 UTC",
            " Ap  ",
            " Máx ",
            " Mín ",
            " Vol.       42",
        ] {
            assert!(joined.contains(want), "falta {want:?}\n{joined}");
        }
        // Barra de estado y nota.
        let st = &t[t.len() - 2];
        assert!(st.contains("● IB Gateway 127.0.0.1:4002 conectado"), "{st}");
        assert!(st.contains("fuente ibkr_paper") && st.contains("demora 11 min"));
        let last = &t[t.len() - 1];
        assert!(last.starts_with(&format!(
            " {KEYS} │ Solo precios: sin indicadores ni señales"
        )));
    }

    #[test]
    fn colors_follow_direction() {
        let rows = rows();
        let bars = bars();
        let lines = render(&screen(&rows, &bars, &SIZES), 140, 40);
        let mnq = lines
            .iter()
            .find(|l| line_text(l).starts_with("▶ MNQ"))
            .unwrap();
        assert!(
            mnq.iter()
                .any(|s| s.tone == Tone::Down && s.text.contains("-0.20%"))
        );
        let qqq = lines
            .iter()
            .find(|l| line_text(l).starts_with("  QQQ"))
            .unwrap();
        assert!(
            qqq.iter()
                .any(|s| s.tone == Tone::Up && s.text.contains("+2.46%"))
        );
    }

    #[test]
    fn narrow_terminal_stacks_panels() {
        let rows = rows();
        let bars = bars();
        let t = text(60, 40, &screen(&rows, &bars, &SIZES));
        assert!(t[1].starts_with("─ WATCHLIST"));
        assert!(!t[2].contains('│'));
        assert!(t.iter().any(|l| l.starts_with("MNQ (MNQZ6) · 15m")));
    }

    #[test]
    fn kill_switch_chart_error_and_loading() {
        let rows = rows();
        let mut s = screen(&rows, &[], &SIZES);
        s.chart.bars = None;
        s.link = Link::KillSwitch {
            reason: "conexión perdida".into(),
        };
        let t = text(140, 30, &s);
        assert!(t.iter().any(|l| l.contains("cargando el gráfico")));
        assert!(t[t.len() - 2].contains("KILL SWITCH: conexión perdida"));
        s.chart.error = Some("IBKR rechazó el pedido");
        assert!(
            text(140, 30, &s)
                .iter()
                .any(|l| l.contains("IBKR rechazó el pedido"))
        );
    }

    #[test]
    fn once_table() {
        let t = render_table(now(), &rows());
        assert_eq!(t.len(), 2 + 4 + 1);
        assert!(t[2].starts_with("MNQ     MNQZ6"));
        assert!(t[2].contains("-53.00 (-0.20%)") && t[2].contains("(11 min)"));
        assert!(t[3].contains("cierre diario 21-11-2026"));
        assert!(t[4].contains("sin datos") && t[4].contains("sin permiso"));
        assert!(!t.iter().any(|l| l.contains('▶')));
    }

    #[test]
    fn truncate_and_fit() {
        let l = vec![Span::new("abc", Tone::Up), Span::new("def", Tone::Down)];
        let t = truncate(l.clone(), 4);
        assert_eq!(line_text(&t), "abcd");
        assert_eq!(t[1].tone, Tone::Down);
        assert!(truncate(vec![Span::new("x", Tone::Plain)], 0).is_empty());
        assert_eq!(line_text(&fit(l, 8, Tone::Plain)), "abcdef  ");
    }
}
