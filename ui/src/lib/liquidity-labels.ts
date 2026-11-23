/** Nombres en español de los niveles y estados de liquidez (#231, spec §5.4.1). Solo texto. */
import type { LevelKind, LevelState, LiquidityLevel, TradingSession } from '@/mocks'

export const LEVEL_LABEL: Record<LevelKind, string> = {
  eqh: 'Máximos iguales (EQH)',
  eql: 'Mínimos iguales (EQL)',
  pdh: 'Máximo del día anterior (PDH)',
  pdl: 'Mínimo del día anterior (PDL)',
  pwh: 'Máximo de la semana anterior (PWH)',
  pwl: 'Mínimo de la semana anterior (PWL)',
  session_high: 'Máximo de la sesión',
  session_low: 'Mínimo de la sesión',
}

export const LEVEL_SHORT: Record<LevelKind, string> = {
  eqh: 'EQH',
  eql: 'EQL',
  pdh: 'PDH',
  pdl: 'PDL',
  pwh: 'PWH',
  pwl: 'PWL',
  session_high: 'máx',
  session_low: 'mín',
}

export const SESSION_LABEL: Record<TradingSession, string> = {
  asia: 'Asia',
  london: 'Londres',
  new_york: 'Nueva York',
}

const SESSION_SHORT: Record<TradingSession, string> = {
  asia: 'Asia',
  london: 'Lon',
  new_york: 'NY',
}

export const STATE_LABEL: Record<LevelState, string> = {
  intact: 'Intacto',
  swept_reversal: 'Barrido con reversión',
  broken: 'Roto',
}

export const levelName = (l: LiquidityLevel) =>
  l.session ? `${LEVEL_LABEL[l.kind]} de ${SESSION_LABEL[l.session]}` : LEVEL_LABEL[l.kind]

export const levelShort = (l: LiquidityLevel) =>
  l.session ? `${SESSION_SHORT[l.session]} ${LEVEL_SHORT[l.kind]}` : LEVEL_SHORT[l.kind]
