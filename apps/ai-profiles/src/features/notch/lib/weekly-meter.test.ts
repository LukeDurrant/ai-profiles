import type { QuotaUsage } from '@/lib/types'

import { describe, expect, it } from 'vitest'

import { elapsedFraction, formatResetsIn, highestPercent, meterLevel, weeklyMeter } from './weekly-meter'

const now = new Date('2026-10-13T00:00:00Z')

function quota(overrides: Partial<QuotaUsage>): QuotaUsage {
  return { primary: null, secondary: null, scopedWeekly: [], ...overrides }
}

describe('weeklyMeter', () => {
  it('reads Claude’s weekly limit from the secondary window and its session from the primary', () => {
    const meter = weeklyMeter(
      'claude',
      quota({
        primary: { utilization: 12, resetsAt: '2026-10-13T03:00:00Z' },
        secondary: { utilization: 32, resetsAt: '2026-10-17T23:00:00Z' },
      }),
      now,
    )
    expect(meter.percent).toBe(32)
    expect(meter.resetsAt).toEqual(new Date('2026-10-17T23:00:00Z'))
    expect(meter.session).toEqual({ label: '5h', percent: 12 })
  })

  it('finds Codex’s weekly limit by its duration, whichever slot it is in', () => {
    const meter = weeklyMeter(
      'codex',
      quota({
        primary: { utilization: 60, resetsAt: '2026-10-19T00:00:00Z', windowDurationMins: 10080 },
        secondary: { utilization: 5, resetsAt: '2026-10-13T05:00:00Z', windowDurationMins: 300 },
      }),
      now,
    )
    expect(meter.percent).toBe(60)
    expect(meter.session).toEqual({ label: '5h', percent: 5 })
  })

  it('has no weekly reading when no window lasts a week', () => {
    const meter = weeklyMeter(
      'codex',
      quota({ primary: { utilization: 5, resetsAt: null, windowDurationMins: 300 } }),
      now,
    )
    expect(meter.percent).toBeNull()
    expect(meter.elapsed).toBeNull()
  })
})

describe('elapsedFraction', () => {
  it('is the share of the week gone before the reset', () => {
    expect(elapsedFraction(new Date('2026-10-16T12:00:00Z'), now)).toBeCloseTo(0.5)
    expect(elapsedFraction(new Date('2026-10-20T00:00:00Z'), now)).toBe(0)
    expect(elapsedFraction(new Date('2026-10-12T00:00:00Z'), now)).toBe(1)
  })
})

describe('formatResetsIn', () => {
  it('shows at most two units', () => {
    expect(formatResetsIn(new Date('2026-10-17T23:00:00Z'), now)).toBe('4d 23h')
    expect(formatResetsIn(new Date('2026-10-13T05:12:00Z'), now)).toBe('5h 12m')
    expect(formatResetsIn(new Date('2026-10-13T00:12:00Z'), now)).toBe('12m')
  })

  it('has nothing to say once the reset has passed', () => {
    expect(formatResetsIn(now, now)).toBeNull()
  })
})

describe('meterLevel', () => {
  it('warns from 80% and is full at the limit', () => {
    expect(meterLevel(null)).toBe('ok')
    expect(meterLevel(79)).toBe('ok')
    expect(meterLevel(80)).toBe('warn')
    expect(meterLevel(100)).toBe('full')
  })
})

describe('highestPercent', () => {
  it('is the most-used weekly limit, ignoring profiles with no reading', () => {
    const meter = (percent: number | null) => ({ percent, elapsed: null, resetsAt: null, session: null })
    expect(highestPercent([meter(32), null, meter(null), meter(71)])).toBe(71)
    expect(highestPercent([null])).toBeNull()
  })
})
