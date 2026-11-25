/** Las vistas de la app, en el orden de la navegación. */
import {
  BarChart3,
  BookOpen,
  ClipboardCheck,
  CandlestickChart,
  Droplets,
  FolderOpen,
  Gauge,
  House,
  LayoutDashboard,
  LayoutGrid,
  ListChecks,
  Trophy,
  Wallet,
} from 'lucide-react'

import type { ShellView } from '@/components/ui/glass-shell'

export const VIEWS: ShellView[] = [
  {
    id: 'performance',
    label: 'Performance',
    short: 'Performance',
    icon: Gauge,
    description:
      'Resumen de la simulación: corridas de backtest, el fondo y sus estadísticas. Resultados del pasado, no promesas.',
  },
  {
    id: 'inicio',
    label: 'Inicio',
    short: 'Inicio',
    icon: House,
    description:
      'Elegí a dónde ir y mirá los resúmenes de la simulación. Resultados del pasado, no promesas.',
  },
  {
    id: 'panel',
    label: 'Panel',
    short: 'Panel',
    icon: LayoutDashboard,
    description: 'Cómo va el fondo, los índices, lo último de la simulación y la liquidez.',
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
    id: 'propuestas',
    label: 'Propuestas',
    short: 'Propuestas',
    icon: ClipboardCheck,
    description: 'Las propuestas del motor que esperan al responsable.',
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
    id: 'liquidez',
    label: 'Liquidez',
    short: 'Liquidez',
    icon: Droplets,
    description:
      'Niveles de liquidez, FVG, sus estadísticas y el desequilibrio, como los manda el motor.',
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
    id: 'operaciones',
    label: 'Operaciones',
    short: 'Operaciones',
    icon: ListChecks,
    description: 'El diario de operaciones en simulación.',
  },
  {
    id: 'fondo',
    label: 'Fondo',
    short: 'Fondo',
    icon: Wallet,
    description: 'El valor de la unidad y lo que tiene cada miembro.',
  },
  {
    id: 'competencia',
    label: 'Competencia',
    short: 'Competencia',
    icon: Trophy,
    description: 'Los trofeos del día y de la semana, y la tabla del grupo.',
  },
  {
    id: 'glosario',
    label: 'Glosario',
    short: 'Glosario',
    icon: BookOpen,
    description: 'Cada término técnico con su explicación y un ejemplo.',
  },
]

/** La vista de inicio (sin hash). `#panel` sigue existiendo y abre el panel de siempre (#581). */
export const HOME_VIEW = 'inicio'

/**
 * Dónde hay vidrio y fondo animado (#581): solo en el Inicio (y en el ingreso, que no es una vista).
 * Las demás secciones tienen superficie lisa.
 */
export const GLASS_VIEWS: readonly string[] = [HOME_VIEW]

/**
 * Las tres pestañas principales (#286), en orden: el gesto del trackpad y los atajos de teclado
 * pasan de una a la siguiente. Cada una tiene su hash y entra en el historial del navegador.
 * El Inicio ocupa el lugar del Panel (#581).
 */
export const MAIN_TABS = ['performance', 'inicio', 'terminal'] as const
