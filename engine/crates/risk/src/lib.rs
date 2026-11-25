//! Riesgo (spec §7, §8 y §10). Módulo **independiente**: toda propuesta pasa por `check` y no
//! hay otro camino para obtener un tamaño de posición. Funciones puras: misma entrada, misma
//! salida, sin estado oculto. La persistencia del kill switch es de quien llama.

pub mod config;

use cfds_core::InstrumentId;
use cfds_market_data::InstrumentSpec;
use rust_decimal::Decimal;

pub use config::{
    CorrelationGroup, DrawdownMode, DrawdownPoint, DrawdownScaling, RiskConfig, VolTarget,
};

/// Lo que el módulo necesita saber de un instrumento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstrumentLimits {
    /// Tope propio de apalancamiento (índices 10:1, acciones 3:1, oro 10:1; ADR 0013).
    pub max_leverage: Decimal,
    /// Margen requerido por el broker, en % del nocional.
    pub margin_pct: Decimal,
    /// Tamaño mínimo del broker. `None` mientras no esté confirmado (#8): entonces no se abre.
    pub min_deal_size: Option<Decimal>,
}

impl InstrumentLimits {
    /// Desde el catálogo. `None` si el margen todavía no tiene fuente.
    pub fn from_spec(spec: &InstrumentSpec) -> Option<Self> {
        Some(Self {
            max_leverage: spec.max_leverage,
            margin_pct: *spec.terms.margin_retail_pct.value()?,
            min_deal_size: spec.terms.min_deal_size.value().copied(),
        })
    }
}

/// Estado de la cuenta al cierre de la vela, en la moneda de la cuenta (EUR).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// Capital actual (incluye el P/L no realizado).
    pub equity: Decimal,
    /// Máximo histórico del capital (para el drawdown).
    pub peak_equity: Decimal,
    /// Capital a las 00:00 de Madrid de hoy.
    pub day_start_equity: Decimal,
    pub free_margin: Decimal,
    pub kill_switch: bool,
    /// Cada posición abierta (también las que esperan su fill).
    pub open_positions: Vec<OpenPosition>,
    /// Volatilidad anual realizada del capital, en % (#671). Solo la usa la volatilidad
    /// objetivo; `None` mientras no haya historia suficiente (calentamiento).
    pub realized_vol_pct: Option<Decimal>,
}

/// Una posición abierta, para los límites de cantidad y el tope por grupo correlacionado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenPosition {
    pub instrument: InstrumentId,
    /// Lo que todavía se puede perder hasta su stop, en la moneda de la cuenta (0 si el stop ya
    /// está en ganancia). Solo lo usa el tope por grupo (#670); nunca negativo.
    pub risk: Decimal,
}

/// Una operación que la estrategia quiere abrir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub instrument: InstrumentId,
    pub entry: Decimal,
    pub stop: Decimal,
    pub atr: Decimal,
    /// Unidades de la moneda del instrumento por 1 unidad de la moneda de la cuenta
    /// (por ejemplo, USD por EUR). 1 si son la misma.
    pub account_to_instrument: Decimal,
}

/// Tamaño aprobado. Montos en la moneda del instrumento, salvo que se indique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sizing {
    /// Moneda del instrumento por punto de movimiento.
    pub size: Decimal,
    pub notional: Decimal,
    /// Nocional / capital, ya con el tope aplicado.
    pub leverage: Decimal,
    pub margin: Decimal,
    /// Lo que se pierde si salta el stop, más el colchón por gaps de `config/risk.toml`
    /// (`gap_atr`, #670; con 0 es solo la distancia al stop). Sin costos.
    pub risk: Decimal,
    /// `true` si el tope de apalancamiento redujo el tamaño (el riesgo queda por debajo del 1 %).
    pub leverage_capped: bool,
    /// `true` si el tope de un grupo correlacionado redujo el tamaño (#670).
    pub group_capped: bool,
    /// Factor aplicado al riesgo por operación por la caída y la volatilidad objetivo (#671);
    /// 1 con las dos apagadas.
    pub risk_scale: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// Datos imposibles (capital, precios, ATR o tipo de cambio no positivos, stop en la entrada).
    InvalidInput(&'static str),
    KillSwitchActive,
    /// El drawdown llegó al límite: quien llama tiene que **activar** el kill switch y alertar.
    DrawdownLimit,
    DailyLossLimit,
    MaxOpenPositions,
    InstrumentAlreadyOpen,
    StopTooClose,
    StopTooFar,
    InsufficientMargin,
    BelowMinDealSize,
    /// El mínimo del broker no está confirmado todavía: no se abre (no se inventa).
    MinDealSizeUnknown,
    /// Un grupo correlacionado del instrumento ya llegó a su tope de riesgo abierto (#670).
    GroupRiskLimit,
}

fn pct(x: Decimal) -> Decimal {
    x / Decimal::ONE_HUNDRED
}

/// El máximo del capital después de valuar la cuenta a mercado al cierre de una vela (#691).
///
/// Es la única regla del máximo para el drawdown (la reducción gradual y el kill switch): el
/// backtest de cartera y el broker simulado la aplican con el capital (realizado + no
/// realizado) al cierre de **cada** vela cerrada, en orden, no solo cuando se decide una orden.
/// Función pura: mismo máximo y mismo capital, mismo resultado.
pub fn mark_peak(peak: Decimal, equity: Decimal) -> Decimal {
    peak.max(equity)
}

/// Valida la operación contra todos los límites y, si pasa, devuelve su tamaño.
pub fn check(
    req: &Request,
    account: &Account,
    limits: &InstrumentLimits,
    cfg: &RiskConfig,
) -> Result<Sizing, Rejection> {
    let positive = [
        (account.equity, "capital"),
        (account.peak_equity, "máximo del capital"),
        (account.day_start_equity, "capital al inicio del día"),
        (req.entry, "entrada"),
        (req.stop, "stop"),
        (req.atr, "ATR"),
        (req.account_to_instrument, "tipo de cambio"),
        (limits.max_leverage, "tope de apalancamiento"),
        (limits.margin_pct, "margen del instrumento"),
    ];
    if let Some((_, what)) = positive.iter().find(|(v, _)| *v <= Decimal::ZERO) {
        return Err(Rejection::InvalidInput(what));
    }
    if account
        .open_positions
        .iter()
        .any(|p| p.risk < Decimal::ZERO)
    {
        return Err(Rejection::InvalidInput("riesgo de una posición abierta"));
    }

    // Límites de la cuenta (§10), del más grave al menos grave.
    if account.kill_switch {
        return Err(Rejection::KillSwitchActive);
    }
    let drawdown_floor = account.peak_equity * (Decimal::ONE - pct(cfg.max_drawdown_pct));
    if account.equity <= drawdown_floor {
        return Err(Rejection::DrawdownLimit);
    }
    let daily_floor = account.day_start_equity * (Decimal::ONE - pct(cfg.max_daily_loss_pct));
    if account.equity <= daily_floor {
        return Err(Rejection::DailyLossLimit);
    }
    if account.open_positions.len() >= cfg.max_open_positions {
        return Err(Rejection::MaxOpenPositions);
    }
    let same = account
        .open_positions
        .iter()
        .filter(|p| p.instrument == req.instrument)
        .count();
    if same >= cfg.max_positions_per_instrument {
        return Err(Rejection::InstrumentAlreadyOpen);
    }

    // Distancia al stop (§7.7).
    let r = (req.entry - req.stop).abs();
    if r.is_zero() {
        return Err(Rejection::InvalidInput("stop en la entrada"));
    }
    if r < cfg.min_stop_atr * req.atr {
        return Err(Rejection::StopTooClose);
    }
    if r > cfg.max_stop_atr * req.atr {
        return Err(Rejection::StopTooFar);
    }

    // Tope por grupo correlacionado (#670), en la moneda de la cuenta: lo que queda libre en el
    // grupo más lleno del instrumento. Sin lugar, se rechaza.
    let room = cfg
        .groups_of(&req.instrument)
        .map(|g| {
            let used: Decimal = account
                .open_positions
                .iter()
                .filter(|p| g.instruments.contains(&p.instrument))
                .map(|p| p.risk)
                .sum();
            pct(g.max_risk_pct) * account.equity - used
        })
        .min();
    if room.is_some_and(|room| room <= Decimal::ZERO) {
        return Err(Rejection::GroupRiskLimit);
    }

    // Tamaño (§8), todo en la moneda del instrumento. El colchón por gaps se suma a la distancia
    // al stop: con gaps, el tamaño baja para que el riesgo total quede en el presupuesto.
    let fx = req.account_to_instrument;
    let equity = account.equity * fx;
    // Con 0 no se suma nada (ni siquiera la escala decimal del cero): el tamaño es el de siempre.
    let per_unit = if cfg.gap_atr.is_zero() {
        r
    } else {
        r + cfg.gap_atr * req.atr
    };
    // Reducción gradual por caída y volatilidad objetivo (#671): solo cambian el riesgo por
    // operación. Con las dos apagadas el factor es 1 y no se multiplica nada.
    let drawdown_pct = (Decimal::ONE - account.equity / account.peak_equity).max(Decimal::ZERO)
        * Decimal::ONE_HUNDRED;
    let risk_scale =
        cfg.drawdown_scaling.scale(drawdown_pct) * cfg.vol_target.scale(account.realized_vol_pct);
    let risk_pct = if risk_scale == Decimal::ONE {
        cfg.risk_per_trade_pct
    } else {
        cfg.risk_per_trade_pct * risk_scale
    };
    let mut size = pct(risk_pct) * equity / per_unit;
    let group_capped = room.is_some_and(|room| size * per_unit > room * fx);
    if let Some(room) = room.filter(|_| group_capped) {
        size = room * fx / per_unit;
    }
    let max_size = limits.max_leverage * equity / req.entry;
    let leverage_capped = size > max_size;
    if leverage_capped {
        size = max_size;
    }
    // Las divisiones de `Decimal` redondean en la cifra 28, a veces hacia arriba: el riesgo, el
    // del grupo o el apalancamiento quedaban 1e-25 por encima de su límite. Sobre el tamaño final
    // (después de la escala, el colchón por gaps y los topes) se baja la última cifra hasta que
    // los tres entran. Nunca se sube (#677).
    let budget = pct(risk_pct) * equity;
    let group_budget = room.map(|room| room * fx);
    while size > Decimal::ZERO
        && (size * per_unit > budget
            || group_budget.is_some_and(|g| size * per_unit > g)
            || size * req.entry / equity > limits.max_leverage)
    {
        size -= Decimal::new(1, size.scale());
    }
    let notional = size * req.entry;
    let margin = notional * pct(limits.margin_pct);
    if margin > account.free_margin * fx {
        return Err(Rejection::InsufficientMargin);
    }
    match limits.min_deal_size {
        None => return Err(Rejection::MinDealSizeUnknown),
        Some(min) if size < min => return Err(Rejection::BelowMinDealSize),
        Some(_) => {}
    }
    Ok(Sizing {
        size,
        notional,
        leverage: notional / equity,
        margin,
        risk: size * per_unit,
        leverage_capped,
        group_capped,
        risk_scale,
    })
}
