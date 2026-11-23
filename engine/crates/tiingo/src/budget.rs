//! Topes propios con ventanas móviles (ADR 0009 §3). Función pura sobre el uso ya registrado.

use std::collections::BTreeSet;

use crate::TiingoError;
use crate::config::Budget;

/// Uso registrado en `tiingo_usage_log`, contado hacia atrás desde ahora.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Usage {
    pub requests_last_hour: u64,
    pub requests_last_day: u64,
    pub symbols_last_31_days: BTreeSet<String>,
    pub bytes_last_31_days: u64,
}

/// Si un pedido más de `symbol`, que puede traer hasta `max_bytes`, entra en los topes. Si no,
/// kill switch: no se pide nada.
pub fn check_budget(
    usage: &Usage,
    symbol: &str,
    max_bytes: u64,
    budget: &Budget,
) -> Result<(), TiingoError> {
    let over = |what: &str, used: u64, cap: u64| {
        Err(TiingoError::Budget(format!(
            "tope de Tiingo: {what} (usado {used}, tope {cap}, config/tiingo.toml). No se pide nada"
        )))
    };
    if usage.requests_last_hour + 1 > budget.requests_per_hour {
        return over(
            "pedidos en los últimos 60 minutos",
            usage.requests_last_hour,
            budget.requests_per_hour,
        );
    }
    if usage.requests_last_day + 1 > budget.requests_per_day {
        return over(
            "pedidos en las últimas 24 horas",
            usage.requests_last_day,
            budget.requests_per_day,
        );
    }
    let symbols = usage.symbols_last_31_days.len() as u64
        + u64::from(!usage.symbols_last_31_days.contains(symbol));
    if symbols > budget.symbols_per_31_days {
        return over(
            "símbolos distintos en los últimos 31 días",
            usage.symbols_last_31_days.len() as u64,
            budget.symbols_per_31_days,
        );
    }
    if usage.bytes_last_31_days.saturating_add(max_bytes) > budget.bytes_per_31_days {
        return over(
            "bytes en los últimos 31 días",
            usage.bytes_last_31_days,
            budget.bytes_per_31_days,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: Budget = Budget {
        requests_per_hour: 2,
        requests_per_day: 3,
        symbols_per_31_days: 2,
        bytes_per_31_days: 1000,
    };

    fn usage(h: u64, d: u64, syms: &[&str], bytes: u64) -> Usage {
        Usage {
            requests_last_hour: h,
            requests_last_day: d,
            symbols_last_31_days: syms.iter().map(|s| (*s).to_owned()).collect(),
            bytes_last_31_days: bytes,
        }
    }

    #[test]
    fn fits_until_each_cap() {
        assert!(check_budget(&usage(1, 2, &["SPY"], 0), "QQQ", 100, &B).is_ok());
        let hour = check_budget(&usage(2, 2, &[], 0), "SPY", 1, &B).unwrap_err();
        assert!(hour.to_string().contains("60 minutos"));
        let day = check_budget(&usage(0, 3, &[], 0), "SPY", 1, &B).unwrap_err();
        assert!(day.to_string().contains("24 horas"));
    }

    #[test]
    fn a_known_symbol_does_not_count_again_but_a_new_one_does() {
        assert!(check_budget(&usage(0, 0, &["SPY", "QQQ"], 0), "SPY", 1, &B).is_ok());
        let e = check_budget(&usage(0, 0, &["SPY", "QQQ"], 0), "DIA", 1, &B).unwrap_err();
        assert!(e.to_string().contains("símbolos"));
    }

    #[test]
    fn bytes_reserve_the_largest_possible_answer() {
        assert!(check_budget(&usage(0, 0, &[], 700), "SPY", 300, &B).is_ok());
        assert!(check_budget(&usage(0, 0, &[], 701), "SPY", 300, &B).is_err());
    }
}
