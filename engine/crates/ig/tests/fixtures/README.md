# Respuestas grabadas de la API demo de IG (#66)

Los tests no llaman a la API: usan estas respuestas.

| Archivo | Pedido | Origen |
|---|---|---|
| `prices_de40_15m.json` | `GET /prices/IX.D.DAX.IFE.IP` v3, `MINUTE_15`, `from=2026-11-20T10:00:00`, `to=2026-11-20T11:00:00` (hora local de la cuenta) | Respuesta real del 2026-11-20, sin cambios |
| `prices_us100_1h_p{1,2,3}.json` | `GET /prices/IX.D.NASDAQ.IFE.IP` v3, `HOUR`, `pageSize=2`, páginas 1 a 3 | Respuestas reales del 2026-11-20, sin cambios |
| `market_us100_ife.json` | `GET /markets/IX.D.NASDAQ.IFE.IP` v3 | Respuesta real del 2026-11-20, sin cambios (datos públicos del contrato) |
| `client_sentiment_4.json` | `GET /clientsentiment?marketIds=DE30,US500,USTECH,WALL` v1 | Respuesta real del 2026-11-20, sin cambios |
| `client_sentiment_unknown.json` | `GET /clientsentiment?marketIds=NO_EXISTE_XYZ` v1 (IG responde 0 % / 0 %, no un error) | Respuesta real del 2026-11-20, sin cambios |
| `prices_daily_synthetic.json` | `GET /prices/IX.D.DAX.IFE.IP` v3, `DAY`, del 23 al 27-10-2026 (cruza el cambio de horario del 25-10) | **Inventado**, con la forma y las horas de inicio medidas el 2026-11-21 (medianoche en Europa central). Los precios son falsos: no se publican precios reales de IG |
| `session_v2.json` | `POST /session` v2 | **Inventado**, con la misma forma que la respuesta real: ids, saldos y endpoint son falsos |

Las respuestas de precios solo tienen datos de mercado (precios, cupo y paginado).
Nunca se graban tokens (`CST`, `X-SECURITY-TOKEN`), ids de cuenta reales ni credenciales.
