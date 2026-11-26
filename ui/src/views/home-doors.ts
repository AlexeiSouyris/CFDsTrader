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
  },
  {
    view: 'graficos',
    section: 'charts',
    title: 'Gráficos',
    description: 'Un gráfico a pantalla completa o hasta cuatro a la vez.',
    icon: BarChart3,
    shortcut: '2',
  },
  {
    view: 'investigacion',
    section: 'research',
    title: 'Investigación',
    description: 'Corridas, walk-forward, modelo, propuestas, liquidez y glosario.',
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
  },
  {
    view: 'competencia',
    section: 'trophies',
    title: 'Trofeos',
    description: 'Trofeos de proceso: disciplina, respetar el stop, completar el diario.',
    icon: Trophy,
    shortcut: '7',
  },
]
