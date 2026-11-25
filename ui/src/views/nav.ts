/** Las vistas de la app, en el orden de la navegación. */
import {
  BarChart3,
  BookOpen,
  ClipboardCheck,
  CandlestickChart,
  Droplets,
  FolderOpen,
  Gauge,
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
    description: 'Precio con volumen, RSI y MACD, y la profundidad del mercado.',
  },
  {
    id: 'multigrafico',
    label: 'Multigráfico',
    short: 'Multi',
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
    description: 'Las carpetas de noticias de cada miembro.',
  },
  {
    id: 'operaciones',
    label: 'Operaciones',
    short: 'Diario',
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
    short: 'Trofeos',
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

/** La vista de inicio (sin hash). */
export const HOME_VIEW = 'panel'

/**
 * Las tres pestañas principales (#286), en orden: el gesto del trackpad y los atajos de teclado
 * pasan de una a la siguiente. Cada una tiene su hash y entra en el historial del navegador.
 */
export const MAIN_TABS = ['performance', 'panel', 'terminal'] as const
