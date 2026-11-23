/** Las vistas de la app, en el orden de la navegación. */
import {
  BarChart3,
  BookOpen,
  ClipboardCheck,
  Droplets,
  FolderOpen,
  LayoutDashboard,
  ListChecks,
  Trophy,
  Wallet,
} from 'lucide-react'

import type { ShellView } from '@/components/ui/glass-shell'

export const VIEWS: ShellView[] = [
  {
    id: 'panel',
    label: 'Panel',
    short: 'Panel',
    icon: LayoutDashboard,
    description: 'Cómo va el fondo, los índices y lo último de la simulación.',
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
