//! Guarda 3 del ADR 0017 §2: la etapa 1 es solo lectura. El código del crate (y su ejemplo) no
//! usa ninguna función de `ibapi` que envíe, modifique o cancele órdenes, ni su módulo de órdenes.

const SOURCES: [(&str, &str); 6] = [
    ("src/lib.rs", include_str!("../src/lib.rs")),
    ("src/account.rs", include_str!("../src/account.rs")),
    ("src/client.rs", include_str!("../src/client.rs")),
    ("src/config.rs", include_str!("../src/config.rs")),
    ("src/session.rs", include_str!("../src/session.rs")),
    (
        "examples/ibkr_read.rs",
        include_str!("../examples/ibkr_read.rs"),
    ),
];

/// Nombres de `ibapi` para órdenes y ejercicios (cliente async y prelude).
const FORBIDDEN: [&str; 14] = [
    "place_order",
    "submit_order",
    "modify_order",
    "cancel_order",
    "global_cancel",
    "order_builder",
    "ibapi::orders",
    "PlaceOrder",
    "exercise_options",
    "bracket_order",
    "next_valid_order_id",
    "update_config",
    "OrderBuilder",
    "Action::Buy",
];

#[test]
fn no_order_code_anywhere() {
    for (path, text) in SOURCES {
        for name in FORBIDDEN {
            assert!(
                !text.contains(name),
                "{path} usa {name}: la etapa 1 es solo lectura"
            );
        }
    }
}
