# 0002 – Broker: IG (IG Europe GmbH) con cuenta demo

- **Estado:** propuesta
- **Fecha:** 2026-11-20
- **Aprobada por:** pendiente (las dos personas)
- **Issue:** #7

## Contexto

El grupo vive en España (regulación ESMA) y decidió (spec §1): el sistema **propone** y un responsable **ejecuta a mano**, con P&L simulado en paralelo. Hacen falta un broker con:
- CFDs de los índices de la spec (US Tech 100, US 500, Wall Street, Alemania 40) y de acciones;
- **API con cuenta demo**, para datos en tiempo real y precios reales de la simulación (#8);
- costos publicados, para que el backtest no use valores inventados (spec §11).

El grupo hoy usa Revolut, que no tiene API pública de trading.

## Decisión (propuesta)

**IG**, a través de **IG Europe GmbH**: autorizada y regulada por BaFin y el Deutsche Bundesbank, opera en España con una sucursal registrada en la CNMV (n.º 121), según el pie de [ig.com/es](https://www.ig.com/es/indices).

- **Cuenta demo** a nombre del titular, para datos y pruebas (#8). Modo de operación del sistema: paper (spec §5).
- **Costos y márgenes** en `config/instruments.toml`, **cada valor con su fuente** y fecha de consulta. Lo que IG no publica queda **pendiente** con su motivo, y el motor (`Catalog::ready_for_backtest`) **no deja validar un backtest** mientras falte un costo.

### Lo confirmado en la web oficial de IG España (consultado el 20-11-2026)

| Dato | Valor | Fuente |
|---|---|---|
| Margen minorista: EEUU Tech 100, US 500, Wall Street, Alemania 40 | 5 % (1:20) | [Índices](https://www.ig.com/es/indices) |
| Spread mínimo del CFD al contado | Tech 100 1,6 · US 500 0,8 · Wall Street 3,4 · Alemania 40 1,8 | [Índices](https://www.ig.com/es/indices), [Costes](https://www.ig.com/es/nuestros-costes) |
| Comisión de CFD de índices | No hay: solo spread | [Costes](https://www.ig.com/es/nuestros-costes) |
| Financiación nocturna de índices | `noches × precio × tamaño × (referencia ± 2,5 %) / 360` (365 solo para el FTSE 100). Se cobra a las 23:00 CE(S)T | [Costes](https://www.ig.com/es/nuestros-costes) |
| CFD de acciones de EE. UU. | 2 centavos por acción, mínimo 15 USD (al abrir y al cerrar) | [Costes](https://www.ig.com/es/nuestros-costes) |
| CFD de acciones europeas | 0,06 %, mínimo 5 € | [Costes](https://www.ig.com/es/nuestros-costes) |
| Margen de acciones (ejemplos: Apple, Inditex) | 20 % (1:5) | [Márgenes](https://www.ig.com/es/margenes) |
| Cierre de posiciones por margen | 50 % del margen requerido (mínimo regulatorio) | Decisión (UE) 2018/796 de ESMA |

### Lo que falta (pendiente en `config/instruments.toml`)

| Dato | Por qué falta | Cómo se obtiene |
|---|---|---|
| Epic de cada mercado | No es público en la web | API de IG con la cuenta demo (#8) |
| Spread **típico** por franja horaria | IG solo publica el mínimo | Medir bid/ask en la demo (#8) |
| Valor por punto y tamaño mínimo del CFD **al contado** | La ficha pública muestra el contrato de futuros | API de IG (#8) |
| Tipo de referencia para índices en USD | La página dice "ESTR (o el tipo de referencia correspondiente)" | Confirmar con IG o la documentación de la API |
| Ajuste de fin de semana de la financiación de índices | La página solo lo explica para forex | Confirmar con IG o medir en la demo |
| Euro Stoxx 50 | Su margen no figura en las tablas y falta el calendario de Eurex | Agregar cuando se confirme |

## Alternativas consideradas

- **Interactive Brokers:** API muy completa (TWS), pero más compleja y con más requisitos para abrir la cuenta. Se puede reconsiderar si IG no alcanza.
- **Revolut:** sin API pública de trading. No sirve para datos ni para la simulación.
- **Seguir sin broker, con datos gratuitos:** sirve para empezar (decisión del grupo, spec §2), pero los costos y precios reales del CFD solo salen del broker.

## Consecuencias

- La conexión con IG va detrás del trait `MarketDataProvider` (#8). Cambiar de broker después no toca la estrategia.
- **Ningún backtest es válido** hasta completar los pendientes: el código lo impide.
- Los costos de la web cambian sin aviso. Antes de la fase 5 hay que **volver a consultarlos**, actualizar la fecha y, donde sea posible, contrastarlos con la API.
- Para conectarse a la API demo hacen falta la cuenta demo y su clave. Se cargan como secreto del entorno, nunca en el repo (CLAUDE.md §5).
