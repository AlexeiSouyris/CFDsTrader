/** Las vistas de la app, en el orden de la navegación. */
import { BarChart3, FolderOpen, LayoutDashboard, ListChecks, Trophy } from 'lucide-react'

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
    id: 'graficos',
    label: 'Gráficos',
    short: 'Gráficos',
    icon: BarChart3,
    description: 'Precio con volumen, RSI y MACD, y la profundidad del mercado.',
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
    id: 'competencia',
    label: 'Competencia',
    short: 'Trofeos',
    icon: Trophy,
    description: 'Los trofeos del día y de la semana, y la tabla del grupo.',
  },
]
