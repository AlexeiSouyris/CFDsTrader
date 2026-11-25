/**
 * Las respuestas del gateway simulado cumplen su contrato de `contracts/` (#661): si un contrato
 * cambia o un fixture inventa un campo, esta prueba falla antes que las vistas.
 */
import { readdirSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { expect, test } from '@playwright/test'
import { Ajv2020 } from 'ajv/dist/2020.js'

import { ROUTES } from './gateway'

const CONTRACTS = fileURLToPath(new URL('../../contracts/', import.meta.url))

const ajv = new Ajv2020({ allErrors: true, strict: false })
for (const file of readdirSync(CONTRACTS).filter((f) => f.endsWith('.schema.json'))) {
  const schema = JSON.parse(readFileSync(CONTRACTS + file, 'utf8')) as { $id?: string }
  // los `$ref` entre contratos usan el nombre del archivo, relativo al `$id`
  ajv.addSchema(schema, schema.$id ?? file)
}

const QUERIES: Record<string, string[]> = {
  '/api/v1/liquidity': ['instrument=US100&timeframe=15m', 'instrument=DE40&timeframe=15m'],
  '/api/v1/ibkr/bars': [
    'instrument=MNQ&timeframe=15m',
    'instrument=MES&timeframe=15m',
    'instrument=QQQ&timeframe=1h',
  ],
  '/api/v1/ibkr/live': ['instrument=MNQ&timeframe=15m', 'instrument=QQQ&timeframe=1h'],
}

for (const route of ROUTES) {
  for (const query of QUERIES[route.path] ?? ['']) {
    test(`${route.path}${query ? `?${query}` : ''} cumple ${route.schema}`, () => {
      const schema = JSON.parse(readFileSync(CONTRACTS + route.schema, 'utf8')) as { $id: string }
      const validate = ajv.getSchema(schema.$id)
      expect(validate, `no se cargó ${route.schema}`).toBeTruthy()
      const body = route.body(new URLSearchParams(query))
      const items = route.array ? (body as unknown[]) : body === null ? [] : [body]
      expect(Array.isArray(items)).toBe(true)
      for (const item of items) {
        const ok = validate!(item)
        expect(ok, JSON.stringify(validate!.errors, null, 1)).toBe(true)
      }
    })
  }
}
