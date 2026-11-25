import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { ProposalCard } from '@/components/ui/proposal-card'
import { PROPOSALS } from '@/mocks'

describe('Propuestas como ticket (#643)', () => {
  it('los niveles van en una franja de ticket, no en una caja redondeada dentro de la tarjeta', () => {
    render(<ProposalCard item={PROPOSALS[0]} canApprove />)
    const card = screen.getAllByRole('article')[0]
    const strip = card.querySelector('dl')!
    expect(strip.className).toMatch(/border-dashed/)
    expect(strip.className).not.toMatch(/rounded/)
    expect(card.querySelectorAll('.rounded-2xl')).toHaveLength(0)
  })
})
