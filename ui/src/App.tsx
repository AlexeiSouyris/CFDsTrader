import { Gallery } from '@/components/gallery/Gallery'
import { entries } from '@/gallery'

export default function App() {
  return (
    <div className="min-h-screen bg-zinc-950 px-4 py-8 text-zinc-200">
      <main className="mx-auto max-w-5xl">
        <h1 className="text-2xl font-bold text-zinc-50">CFDsTrader · prototipo de la interfaz</h1>
        <p
          role="note"
          className="mt-3 mb-8 rounded-lg border border-amber-700/60 bg-amber-950/40 px-4 py-2 text-sm text-amber-200"
        >
          Maqueta con <strong>datos de ejemplo</strong>: no está conectada al motor ni a IG y no
          calcula nada. Nada de lo que se ve acá es una señal real.
        </p>
        <Gallery entries={entries} />
      </main>
    </div>
  )
}
