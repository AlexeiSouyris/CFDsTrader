//! Solo lectura y solo presentación (#541): el código de este crate no usa nada de `ibapi` para
//! órdenes (ni siquiera depende de `ibapi`: habla con IBKR solo por la sesión de `cfds-ibkr`), y no
//! depende de los crates que calculan indicadores, patrones o señales.

const SOURCES: [(&str, &str); 7] = [
    ("Cargo.toml", include_str!("../Cargo.toml")),
    ("src/lib.rs", include_str!("../src/lib.rs")),
    ("src/config.rs", include_str!("../src/config.rs")),
    ("src/quote.rs", include_str!("../src/quote.rs")),
    ("src/chart.rs", include_str!("../src/chart.rs")),
    ("src/schedule.rs", include_str!("../src/schedule.rs")),
    ("src/bin/bolsa.rs", include_str!("../src/bin/bolsa.rs")),
];

const FORBIDDEN: [&str; 16] = [
    "place_order",
    "submit_order",
    "modify_order",
    "cancel_order",
    "global_cancel",
    "order_builder",
    "ibapi",
    "PlaceOrder",
    "exercise_options",
    "bracket_order",
    "next_valid_order_id",
    "OrderBuilder",
    "cfds-indicators",
    "cfds-patterns",
    "cfds-strategy",
    "cfds-broker",
];

#[test]
fn no_orders_and_no_trading_logic() {
    for (path, text) in SOURCES {
        for name in FORBIDDEN {
            assert!(!text.contains(name), "{path} usa {name}");
        }
    }
}
