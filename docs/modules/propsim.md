---
modulo: propsim
ruta: `engine/crates/propsim` (crate `cfds-propsim`)
lenguaje: Rust
estado: en curso (#802)
resumen: Simulador de cuentas fondeadas de la Academia: reglas de prop firms configurables, recorrido determinista de una lista de operaciones y Monte Carlo.
---

# propsim

Diseño: `docs/design/2026-11-academia.md` §6 (#709). Contratos: `prop_rules.v1`, `prop_sim_request.v1` y `prop_sim_result.v1`.

## Responsabilidad (única)

- Reglas de una cuenta fondeada como **funciones puras** (`rules.rs`): el documento del contrato (`RulesDoc`, con valores en blanco en una plantilla) y su validación para simular (`Rules`: montos en dinero con `Decimal`, porcentajes como fracción, zona IANA y hora de corte del día de la firma), con **todos** los errores y su ruta.
- Plantillas de `config/propsim.toml` (validadas al arrancar): **solo la genérica en blanco**. Una plantilla con algún valor es de una firma real y necesita su reglamento publicado (`source`: firma, enlace `https`, fecha de lectura). No se inventan reglas (CLAUDE.md §5b).
- Recorrido determinista (`walk.rs`): operaciones (cierre UTC, R × riesgo en dinero) agrupadas en días de la firma; cada fase arranca en el tamaño de la cuenta; pérdida diaria desde el saldo al inicio del día (% del tamaño o de ese saldo), caída máxima fija o que sigue al máximo al cierre del día (con o sin traba en el saldo inicial), objetivo al cierre del día con días mínimos y regla de consistencia, días máximos (se acaba el tiempo) y etapa fondeada hasta el primer cobro (ganancia × reparto). Tocar un límite es fallar.
- La caída que sigue al máximo **intradía** se marca **no simulable** (`Rules::unsimulable`): necesita el capital vela por vela (PR 13 del plan de la Academia); no se aproxima y el resultado dice que pasar es un tope superior.
- Pedido → resultado (`request.rs`): `prop_sim_request.v1` (reglas, riesgo por operación, fuente `trades`, `report` o `model`) → `prop_sim_result.v1`. Los R de un informe guardado los busca quien llama (el gateway).

No manda órdenes, no toca el riesgo ni el paper y no guarda nada. La UI no calcula nada de esto.

## Estado

- Contratos, reglas, plantilla genérica, recorrido determinista y resultado sin Monte Carlo (#802).
