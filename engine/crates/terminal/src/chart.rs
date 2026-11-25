//! Mini gráfico de velas en texto: una columna por vela (más un espacio), mecha `│` y cuerpo `█`.
//! Solo dibuja las velas que recibe; no calcula nada sobre ellas más allá de la escala de la pantalla.

use cfds_ibkr::{BarSize, RawBar};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::quote::fmt_price;
use crate::view::{Line, Span, Tone};

/// Fila (0 = arriba) de un precio dentro de `[lo, hi]` en una grilla de `rows` filas.
pub fn row_of(p: Decimal, lo: Decimal, hi: Decimal, rows: usize) -> usize {
    let last = rows.saturating_sub(1);
    let range = hi - lo;
    if range <= Decimal::ZERO || last == 0 {
        return last / 2;
    }
    let p = p.clamp(lo, hi);
    let r = ((hi - p) * Decimal::from(last) / range).round();
    r.to_usize().unwrap_or(0).min(last)
}

/// Cuántas velas entran en `plot_width` columnas (dos columnas por vela).
pub fn capacity(plot_width: usize) -> usize {
    plot_width / 2
}

fn time_label(b: &RawBar, size: BarSize) -> String {
    match size {
        BarSize::D1 => b.start.format("%d-%m-%Y").to_string(),
        _ => b.start.format("%d-%m %H:%M UTC").to_string(),
    }
}

/// Dibuja las últimas velas que entran en `width` × `height` (la última fila es el eje de tiempo y
/// a la derecha va el eje de precios). `mono`: sin colores, el cuerpo de las velas bajistas es `░`.
pub fn render(
    bars: &[RawBar],
    size: BarSize,
    width: usize,
    height: usize,
    mono: bool,
) -> Vec<Line> {
    if bars.is_empty() {
        return vec![vec![Span::new(
            "sin velas cerradas en la ventana pedida (¿mercado cerrado?)",
            Tone::Dim,
        )]];
    }
    let rows = height.saturating_sub(1);
    let first_guess_axis = bars
        .iter()
        .map(|b| fmt_price(b.high).len().max(fmt_price(b.low).len()))
        .max()
        .unwrap_or(0)
        + 1;
    let plot_w = width.saturating_sub(first_guess_axis);
    let n = capacity(plot_w).min(bars.len());
    if rows < 3 || n == 0 {
        return vec![vec![Span::new(
            "la ventana es muy chica para el gráfico",
            Tone::Dim,
        )]];
    }
    let shown = &bars[bars.len() - n..];
    let hi = shown.iter().map(|b| b.high).max().unwrap_or(Decimal::ZERO);
    let lo = shown.iter().map(|b| b.low).min().unwrap_or(Decimal::ZERO);
    let mut grid: Vec<Vec<(char, Tone)>> = vec![vec![(' ', Tone::Plain); n * 2]; rows];
    for (i, b) in shown.iter().enumerate() {
        let up = b.close >= b.open;
        let tone = if up { Tone::Up } else { Tone::Down };
        let body = if up || !mono { '█' } else { '░' };
        let top = row_of(b.high, lo, hi, rows);
        let bot = row_of(b.low, lo, hi, rows);
        let body_top = row_of(b.open.max(b.close), lo, hi, rows);
        let body_bot = row_of(b.open.min(b.close), lo, hi, rows);
        for (r, line) in grid.iter_mut().enumerate().take(bot + 1).skip(top) {
            let ch = if (body_top..=body_bot).contains(&r) {
                body
            } else {
                '│'
            };
            line[i * 2] = (ch, tone);
        }
    }
    // El del medio con los decimales de los extremos, para que no ensanche el eje.
    let mid = ((hi + lo) / Decimal::TWO).round_dp(hi.scale().max(lo.scale()).max(2));
    let labels = [(0, hi), (rows / 2, mid), (rows - 1, lo)];
    let mut out: Vec<Line> = Vec::with_capacity(rows + 1);
    for (r, cells) in grid.into_iter().enumerate() {
        let mut line: Line = Vec::new();
        for (ch, tone) in cells {
            match line.last_mut() {
                Some(s) if s.tone == tone => s.text.push(ch),
                _ => line.push(Span::new(ch.to_string(), tone)),
            }
        }
        if let Some((_, p)) = labels.iter().find(|(lr, _)| *lr == r) {
            line.push(Span::new(format!(" {}", fmt_price(*p)), Tone::Dim));
        }
        out.push(line);
    }
    let left = time_label(&shown[0], size);
    let right = time_label(&shown[n - 1], size);
    let gap = (n * 2).saturating_sub(left.chars().count() + right.chars().count());
    let axis = if n > 1 && gap >= 1 {
        format!("{left}{}{right}", " ".repeat(gap))
    } else {
        right
    };
    out.push(vec![Span::new(axis, Tone::Dim)]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::{line_text, line_width};
    use chrono::{TimeDelta, TimeZone, Utc};
    use rust_decimal::dec;

    fn bar(i: i64, o: Decimal, h: Decimal, l: Decimal, c: Decimal) -> RawBar {
        RawBar {
            start: Utc.with_ymd_and_hms(2026, 11, 25, 14, 0, 0).unwrap()
                + TimeDelta::minutes(15 * i),
            open: o,
            high: h,
            low: l,
            close: c,
            volume: None,
        }
    }

    #[test]
    fn rows_scale_top_to_bottom() {
        assert_eq!(row_of(dec!(110), dec!(100), dec!(110), 11), 0);
        assert_eq!(row_of(dec!(100), dec!(100), dec!(110), 11), 10);
        assert_eq!(row_of(dec!(105), dec!(100), dec!(110), 11), 5);
        // Rango cero (todas las velas iguales): al medio.
        assert_eq!(row_of(dec!(5), dec!(5), dec!(5), 11), 5);
        // Fuera del rango se recorta.
        assert_eq!(row_of(dec!(200), dec!(100), dec!(110), 11), 0);
    }

    #[test]
    fn draws_up_and_down_candles_with_axes() {
        let bars = [
            bar(0, dec!(100), dec!(110), dec!(100), dec!(108)),
            bar(1, dec!(108), dec!(108), dec!(100), dec!(101)),
        ];
        let lines = render(&bars, BarSize::M15, 40, 12, false);
        assert_eq!(lines.len(), 12);
        let top = line_text(&lines[0]);
        assert!(top.starts_with('│'), "{top:?}");
        assert!(top.ends_with(" 110.00"), "{top:?}");
        assert!(line_text(&lines[10]).ends_with(" 100.00"));
        assert!(line_text(&lines[5]).ends_with(" 105.00"));
        // La primera es alcista (verde) y la segunda bajista (roja).
        assert!(
            lines[2]
                .iter()
                .any(|s| s.tone == Tone::Up && s.text.contains('█'))
        );
        assert!(
            lines[2]
                .iter()
                .any(|s| s.tone == Tone::Down && s.text.contains('█'))
        );
        let axis = line_text(&lines[11]);
        assert!(axis.contains("25-11 14:15 UTC"), "{axis:?}");
        for l in &lines {
            assert!(line_width(l) <= 40);
        }
    }

    #[test]
    fn mono_marks_down_bodies_without_color() {
        let bars = [bar(0, dec!(108), dec!(110), dec!(100), dec!(101))];
        let lines = render(&bars, BarSize::M15, 30, 8, true);
        assert!(lines.iter().any(|l| line_text(l).contains('░')));
        assert!(!lines.iter().any(|l| line_text(l).contains('█')));
    }

    #[test]
    fn keeps_only_the_last_candles_that_fit() {
        let bars: Vec<RawBar> = (0..100)
            .map(|i| {
                let p = Decimal::from(100 + i);
                bar(i, p, p + dec!(1), p, p + dec!(1))
            })
            .collect();
        let width = 30;
        let lines = render(&bars, BarSize::M15, width, 10, false);
        for l in &lines {
            assert!(line_width(l) <= width, "{:?}", line_text(l));
        }
        // La última vela (máximo 200) marca el tope de la escala.
        assert!(line_text(&lines[0]).ends_with(" 200.00"));
    }

    #[test]
    fn empty_or_tiny() {
        assert_eq!(render(&[], BarSize::M1, 80, 20, false).len(), 1);
        let bars = [bar(0, dec!(1), dec!(1), dec!(1), dec!(1))];
        assert_eq!(render(&bars, BarSize::M1, 80, 2, false).len(), 1);
        assert_eq!(render(&bars, BarSize::M1, 5, 20, false).len(), 1);
    }
}
