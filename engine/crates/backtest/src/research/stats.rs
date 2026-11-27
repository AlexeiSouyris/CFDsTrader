//! Estadística de la regla de decisión pre-registrada (#466 §4.3, pre-registro §2.5): prueba t
//! unilateral, momentos de la muestra y Sharpe deflactado (Bailey y López de Prado, 2014).
//!
//! Usa `f64`: son estadísticos de una muestra de retornos, **no dinero**. Precios, P/L, costos y
//! capital siguen en `Decimal` en el resto del módulo.
#![allow(
    clippy::float_arithmetic,
    reason = "estadísticos de una muestra (prueba t, Φ, momentos): no son dinero (CLAUDE.md §4)"
)]

/// Constante de Euler–Mascheroni.
const EULER_GAMMA: f64 = 0.577_215_664_901_532_9;

pub fn mean(xs: &[f64]) -> Option<f64> {
    (!xs.is_empty()).then(|| xs.iter().sum::<f64>() / xs.len() as f64)
}

/// Desvío estándar muestral (n − 1).
pub fn std_dev(xs: &[f64]) -> Option<f64> {
    if xs.len() < 2 {
        return None;
    }
    let m = mean(xs)?;
    let v = xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (xs.len() - 1) as f64;
    Some(v.sqrt())
}

/// Asimetría y curtosis (no el exceso) con los momentos centrales de la muestra.
pub fn skew_kurtosis(xs: &[f64]) -> Option<(f64, f64)> {
    let m = mean(xs)?;
    let n = xs.len() as f64;
    let m2 = xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / n;
    if m2 <= 0.0 {
        return None;
    }
    let m3 = xs.iter().map(|x| (x - m).powi(3)).sum::<f64>() / n;
    let m4 = xs.iter().map(|x| (x - m).powi(4)).sum::<f64>() / n;
    Some((m3 / m2.powf(1.5), m4 / (m2 * m2)))
}

/// ln Γ(x) (Lanczos, g = 7, 9 coeficientes), x > 0.
fn ln_gamma(x: f64) -> f64 {
    const C: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        let pi = std::f64::consts::PI;
        return (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = C[0];
    let t = x + 7.5;
    for (i, c) in C.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// Fracción continua de la beta incompleta (Lentz modificado).
fn beta_cf(a: f64, b: f64, x: f64) -> f64 {
    const TINY: f64 = 1e-300;
    let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..=300 {
        let m = f64::from(m);
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-15 {
            break;
        }
    }
    h
}

/// Beta incompleta regularizada I_x(a, b).
fn beta_inc(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let ln_front = ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln();
    let front = ln_front.exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        front * beta_cf(a, b, x) / a
    } else {
        1.0 - front * beta_cf(b, a, 1.0 - x) / b
    }
}

/// P(T > t) para una t de Student con `df` grados de libertad.
pub fn student_t_upper(t: f64, df: f64) -> f64 {
    let tail = 0.5 * beta_inc(df / 2.0, 0.5, df / (df + t * t));
    if t >= 0.0 { tail } else { 1.0 - tail }
}

/// Φ(x), con la función de error complementaria (Numerical Recipes `erfcc`, error < 1,2e-7).
pub fn normal_cdf(x: f64) -> f64 {
    let z = x.abs() / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.5 * z);
    let poly = -z * z - 1.265_512_23
        + t * (1.000_023_68
            + t * (0.374_091_96
                + t * (0.096_784_18
                    + t * (-0.186_288_06
                        + t * (0.278_868_07
                            + t * (-1.135_203_98
                                + t * (1.488_515_87 + t * (-0.822_152_23 + t * 0.170_872_77))))))));
    let erfc = t * poly.exp();
    if x >= 0.0 {
        1.0 - 0.5 * erfc
    } else {
        0.5 * erfc
    }
}

/// Φ⁻¹(p), 0 < p < 1 (algoritmo de Acklam, error relativo < 1,2e-9).
pub fn normal_inv(p: f64) -> f64 {
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    let low = 0.024_25;
    let tail = |q: f64| {
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    };
    if p < low {
        tail((-2.0 * p.ln()).sqrt())
    } else if p > 1.0 - low {
        -tail((-2.0 * (1.0 - p).ln()).sqrt())
    } else {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    }
}

/// Prueba t de una muestra, unilateral (H0: media ≤ 0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TTest {
    pub n: usize,
    pub mean: f64,
    pub std_dev: f64,
    pub t: f64,
    /// P(T > t) con n − 1 grados de libertad.
    pub p_value: f64,
}

pub fn t_test_greater(xs: &[f64]) -> Option<TTest> {
    let (m, s) = (mean(xs)?, std_dev(xs)?);
    if s <= 0.0 {
        return None;
    }
    let n = xs.len();
    let t = m / (s / (n as f64).sqrt());
    Some(TTest {
        n,
        mean: m,
        std_dev: s,
        t,
        p_value: student_t_upper(t, (n - 1) as f64),
    })
}

/// Sharpe deflactado (Bailey y López de Prado, 2014) de una serie de retornos, con `trials`
/// pruebas y la varianza de los Sharpe bajo la nula `1 / (T − 1)` (pre-registro §2.5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Deflated {
    /// Sharpe por período, sin anualizar.
    pub sharpe: f64,
    /// Sharpe esperado del máximo de `trials` pruebas sin ventaja.
    pub sharpe0: f64,
    pub skew: f64,
    pub kurtosis: f64,
    pub dsr: f64,
}

pub fn deflated_sharpe(xs: &[f64], trials: u32) -> Option<Deflated> {
    let (m, s) = (mean(xs)?, std_dev(xs)?);
    let (skew, kurtosis) = skew_kurtosis(xs)?;
    if s <= 0.0 || trials < 2 {
        return None;
    }
    let t = xs.len() as f64;
    let sr = m / s;
    let n = f64::from(trials);
    let v = 1.0 / (t - 1.0);
    let sr0 = v.sqrt()
        * ((1.0 - EULER_GAMMA) * normal_inv(1.0 - 1.0 / n)
            + EULER_GAMMA * normal_inv(1.0 - 1.0 / (n * std::f64::consts::E)));
    let denom = 1.0 - skew * sr + (kurtosis - 1.0) / 4.0 * sr * sr;
    if denom <= 0.0 {
        return None;
    }
    let dsr = normal_cdf((sr - sr0) * (t - 1.0).sqrt() / denom.sqrt());
    Some(Deflated {
        sharpe: sr,
        sharpe0: sr0,
        skew,
        kurtosis,
        dsr,
    })
}

/// Regresión `y = α + β x + e` por mínimos cuadrados, con el error estándar de `α` robusto a
/// heterocedasticidad (White, HC1: factor `n / (n − 2)`) y la prueba t unilateral de `α`
/// (H0: α ≤ 0) con `n − 2` grados de libertad (pre-registro de la ronda 3 §3.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Alpha {
    pub n: usize,
    pub alpha: f64,
    pub beta: f64,
    pub se_alpha: f64,
    pub t: f64,
    /// P(T > t) con n − 2 grados de libertad.
    pub p_value: f64,
}

pub fn alpha_hc1(y: &[f64], x: &[f64]) -> Option<Alpha> {
    let n = y.len();
    if n != x.len() || n < 3 {
        return None;
    }
    let (my, mx) = (mean(y)?, mean(x)?);
    let sxx: f64 = x.iter().map(|v| (v - mx) * (v - mx)).sum();
    if sxx <= 0.0 {
        return None;
    }
    let sxy: f64 = x.iter().zip(y).map(|(a, b)| (a - mx) * (b - my)).sum();
    let beta = sxy / sxx;
    let alpha = my - beta * mx;
    let nf = n as f64;
    // (X'X)⁻¹ X' diag(e²) X (X'X)⁻¹, elemento de α, con X = [1, x].
    let sx: f64 = x.iter().sum();
    let sx2: f64 = x.iter().map(|v| v * v).sum();
    let det = nf * sx2 - sx * sx;
    if det <= 0.0 {
        return None;
    }
    // Fila de α en (X'X)⁻¹: [sx2, −sx] / det.
    let meat: f64 = x
        .iter()
        .zip(y)
        .map(|(xi, yi)| {
            let e = yi - alpha - beta * xi;
            let g = (sx2 - sx * xi) / det;
            g * g * e * e
        })
        .sum();
    let var = meat * nf / (nf - 2.0);
    let se = var.sqrt();
    if se <= 0.0 {
        return None;
    }
    let t = alpha / se;
    Some(Alpha {
        n,
        alpha,
        beta,
        se_alpha: se,
        t,
        p_value: student_t_upper(t, nf - 2.0),
    })
}

/// Prueba t unilateral (H0: media ≤ 0) de una serie con el error estándar de Newey-West
/// (programa 5 §4.2 y §13.3.6): `γ_l = (1/n) Σ_{j>l} (x_j − x̄)(x_{j−l} − x̄)`,
/// `S = γ_0 + 2 Σ_{l=1..L} (1 − l/(L+1)) γ_l`, `t = x̄ / √(S/n)`, con `n − 1` grados de libertad.
/// `None` si hay menos de dos datos o `S ≤ 0`. `std_dev` devuelve `√S`.
pub fn newey_west_t_greater(xs: &[f64], lags: usize) -> Option<TTest> {
    let n = xs.len();
    if n < 2 {
        return None;
    }
    let m = mean(xs)?;
    let nf = n as f64;
    let gamma =
        |l: usize| -> f64 { (l..n).map(|j| (xs[j] - m) * (xs[j - l] - m)).sum::<f64>() / nf };
    let big_l = lags as f64;
    let s = gamma(0)
        + 2.0
            * (1..=lags.min(n - 1))
                .map(|l| (1.0 - l as f64 / (big_l + 1.0)) * gamma(l))
                .sum::<f64>();
    if s <= 0.0 {
        return None;
    }
    let t = m / (s / nf).sqrt();
    Some(TTest {
        n,
        mean: m,
        std_dev: s.sqrt(),
        t,
        p_value: student_t_upper(t, nf - 1.0),
    })
}

/// Prueba t de Welch unilateral (H0: media de `a` ≤ media de `b`), con los grados de libertad de
/// Welch-Satterthwaite (programa 5 §5.3 y §13.3.10).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Welch {
    pub diff: f64,
    pub se: f64,
    pub t: f64,
    pub df: f64,
    pub p_value: f64,
}

pub fn welch_greater(a: &[f64], b: &[f64]) -> Option<Welch> {
    let (ma, mb) = (mean(a)?, mean(b)?);
    let (sa, sb) = (std_dev(a)?, std_dev(b)?);
    let (va, vb) = (sa * sa / a.len() as f64, sb * sb / b.len() as f64);
    let se2 = va + vb;
    if se2 <= 0.0 {
        return None;
    }
    let df = se2 * se2 / (va * va / (a.len() - 1) as f64 + vb * vb / (b.len() - 1) as f64);
    let t = (ma - mb) / se2.sqrt();
    Some(Welch {
        diff: ma - mb,
        se: se2.sqrt(),
        t,
        df,
        p_value: student_t_upper(t, df),
    })
}
