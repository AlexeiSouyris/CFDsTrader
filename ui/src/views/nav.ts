/** Las vistas de la app, en el orden de la navegación. */
import {
  BarChart3,
  BookOpen,
  ClipboardCheck,
  FlaskConical,
  CandlestickChart,
  Droplets,
  FolderOpen,
  Gauge,
  House,
  LayoutDashboard,
  LayoutGrid,
  NotebookPen,
  Trophy,
  Wallet,
} from 'lucide-react'

import type { ShellView } from '@/components/ui/app-shell'

export const VIEWS: ShellView[] = [
  {
    id: 'inicio',
    label: 'Inicio',
    short: 'Inicio',
    icon: House,
    description:
      'Elegí a dónde ir y mirá los resúmenes de la simulación. Resultados del pasado, no promesas.',
  },
  {
    id: 'terminal',
    label: 'Terminal',
    short: 'Terminal',
    icon: CandlestickChart,
    description:
      'Mercados, gráfico, ticket, posiciones, órdenes y actividad. Solo simulación: no se envía ninguna orden real.',
  },
  {
    id: 'graficos',
    label: 'Gráficos',
    short: 'Gráficos',
    icon: BarChart3,
    description:
      'Un gráfico a pantalla completa con las velas del motor. Solo para mirar: sin órdenes.',
  },
  {
    id: 'multigrafico',
    label: 'Multigráfico',
    short: 'Multigráfico',
    icon: LayoutGrid,
    description:
      'Hasta cuatro gráficos a la vez, cada uno con su instrumento y timeframe. Solo para mirar: sin órdenes.',
  },
  {
    id: 'investigacion',
    label: 'Investigación',
    short: 'Investigación',
    icon: FlaskConical,
    description:
      'Corridas, modelo, propuestas, liquidez y glosario en un solo informe. Resultados del pasado, no promesas.',
  },
  {
    id: 'performance',
    label: 'Performance',
    short: 'Performance',
    icon: Gauge,
    description:
      'Resumen de la simulación: corridas de backtest, el fondo y sus estadísticas. Resultados del pasado, no promesas.',
  },
  {
    id: 'propuestas',
    label: 'Propuestas',
    short: 'Propuestas',
    icon: ClipboardCheck,
    description: 'Las propuestas del motor que esperan al responsable.',
  },
  {
    id: 'liquidez',
    label: 'Liquidez',
    short: 'Liquidez',
    icon: Droplets,
    description:
      'Niveles de liquidez, FVG, sus estadísticas y el desequilibrio, como los manda el motor.',
  },
  {
    id: 'glosario',
    label: 'Glosario',
    short: 'Glosario',
    icon: BookOpen,
    description: 'Cada término técnico con su explicación y un ejemplo.',
  },
  {
    id: 'noticias',
    label: 'Noticias',
    short: 'Noticias',
    icon: FolderOpen,
    description:
      'Las noticias con su hora de publicación, las carpetas de cada miembro y el contexto de mercado.',
  },
  {
    id: 'fondo',
    label: 'Fondo',
    short: 'Fondo',
    icon: Wallet,
    description: 'El valor de la unidad y lo que tiene cada miembro.',
  },
  {
    id: 'operaciones',
    label: 'Diario',
    short: 'Diario',
    icon: NotebookPen,
    description:
      'El diario de las operaciones en simulación: cada una con su motivo, el resultado del motor y la nota.',
  },
  {
    id: 'competencia',
    label: 'Trofeos',
    short: 'Trofeos',
    icon: Trophy,
    description:
      'Logros del proceso: respetar el stop, anotar cada operación y cumplir las reglas. Nunca premian la ganancia.',
  },
  {
    id: 'panel',
    label: 'Panel',
    short: 'Panel',
    icon: LayoutDashboard,
    description: 'Cómo va el fondo, los índices, lo último de la simulación y la liquidez.',
  },
]

/** La vista de inicio (sin hash). `#panel` sigue existiendo y abre el panel de siempre (#581). */
export const HOME_VIEW = 'inicio'

/**
 * Las secciones del mapa (#581): las que van en la barra lateral y en la del celular. Las demás
 * vistas siguen abriéndose con su hash y desde la paleta, y marcan la sección a la que pertenecen.
 */
export const NAV_SECTIONS = [
  'inicio',
  'terminal',
  'graficos',
  'investigacion',
  'noticias',
  'fondo',
  'operaciones',
  'competencia',
] as const

/** La sección de cada vista que vive dentro de otra (los hashes viejos siguen funcionando). */
const PARENT: Record<string, string> = {
  panel: 'inicio',
  multigrafico: 'graficos',
  performance: 'investigacion',
  propuestas: 'investigacion',
  liquidez: 'investigacion',
  glosario: 'investigacion',
}
export const sectionOf = (id: string) => PARENT[id] ?? id

/**
 * "Todas las vistas" (#618): cada sección del mapa con las vistas que viven adentro, en el orden de
 * `VIEWS`. Lo abre "Más": con la barra (lateral o la de abajo) cualquier vista queda a dos toques.
 */
export const MENU_GROUPS = NAV_SECTIONS.map((id) => ({
  id,
  children: VIEWS.filter((v) => PARENT[v.id] === id).map((v) => v.id),
}))

/**
 * Las pestañas principales (#286, #581), en orden: Inicio · Terminal · Gráficos · Investigación.
 * El gesto del trackpad y los atajos de teclado pasan de una a la siguiente (desde una vista de
 * adentro, como `#performance`, cuenta la de su sección). Cada una tiene su hash y entra en el
 * historial del navegador.
 */
export const MAIN_TABS = ['inicio', 'terminal', 'graficos', 'investigacion'] as const

/**
 * La barra de abajo del celular (#618): las cuatro pestañas principales y "Más". Entran sin
 * desplazarse en 320 px; el resto de las secciones y las vistas de adentro están en "Más".
 */
export const BAR_VIEWS = MAIN_TABS

/**
 * Atajos de función (#708), como en una terminal: F1 a F4 las pestañas principales y F6 a F9 las
 * demás secciones. F5 queda libre (recarga la página en el navegador).
 */
export const FKEYS: Readonly<Record<string, string>> = {
  inicio: 'F1',
  terminal: 'F2',
  graficos: 'F3',
  investigacion: 'F4',
  noticias: 'F6',
  fondo: 'F7',
  operaciones: 'F8',
  competencia: 'F9',
}
