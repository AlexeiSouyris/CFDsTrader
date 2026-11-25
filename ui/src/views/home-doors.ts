/** Las puertas del Inicio (#581): a qué vista lleva cada una, su sección (acento) y su atajo. */
import {
  BarChart3,
  CandlestickChart,
  FlaskConical,
  Newspaper,
  NotebookPen,
  Trophy,
  Wallet,
} from 'lucide-react'

import type { Door } from '@/components/ui/section-door'

export const DOORS: Door[] = [
  {
    view: 'terminal',
    section: 'terminal',
    title: 'Terminal',
    description: 'Mercados, gráfico, ticket y posiciones. Solo simulación: ninguna orden real.',
    icon: CandlestickChart,
    shortcut: '1',
    size: 'tall',
  },
  {
    view: 'graficos',
    section: 'charts',
    title: 'Gráficos',
    description: 'Precio con volumen, RSI y MACD; hasta cuatro a la vez.',
    icon: BarChart3,
    shortcut: '2',
  },
  {
    view: 'performance',
    section: 'research',
    title: 'Investigación',
    description: 'Corridas, walk-forward, modelo y propuestas.',
    icon: FlaskConical,
    shortcut: '3',
  },
  {
    view: 'noticias',
    section: 'news',
    title: 'Noticias',
    description: 'Las carpetas de cada miembro y el contexto de mercado.',
    icon: Newspaper,
    shortcut: '4',
  },
  {
    view: 'fondo',
    section: 'fund',
    title: 'Fondo',
    description: 'El valor de la unidad y lo que tiene cada miembro.',
    icon: Wallet,
    shortcut: '5',
  },
  {
    view: 'operaciones',
    section: 'journal',
    title: 'Diario',
    description: 'Cada operación simulada, con su motivo y su nota.',
    icon: NotebookPen,
    shortcut: '6',
    size: 'wide',
  },
  {
    view: 'competencia',
    section: 'trophies',
    title: 'Competencia',
    description: 'Trofeos de proceso: disciplina, respetar el stop, completar el diario.',
    icon: Trophy,
    shortcut: '7',
    size: 'wide',
  },
]
