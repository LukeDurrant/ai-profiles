import type { AppState } from '@/lib/types'

import { fireEvent, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { loadAppState, updateAppState } from '@/lib/commands'
import { renderWithQuery } from '@/test/render-with-query'

import { NotchProfileControls } from './notch-profile-controls'

vi.mock('@/lib/commands', () => ({ loadAppState: vi.fn(), updateAppState: vi.fn() }))

function appState(overrides: Partial<AppState> = {}): AppState {
  return {
    welcomeShown: true,
    migrationDismissedAt: null,
    pathBannerDismissedAt: null,
    themeMode: 'system',
    selectedEntryId: null,
    dockIconAcknowledgedAt: null,
    defaultProfileNames: {},
    dismissedRepairSessions: {},
    defaultProfileColors: {},
    notchHudOff: false,
    notchHiddenProfiles: [],
    usageAlertThresholds: null,
    fiveDayWeekProfiles: [],
    ...overrides,
  }
}

beforeEach(() => {
  vi.mocked(updateAppState).mockImplementation(async (patch) =>
    appState({ notchHiddenProfiles: patch.notchProfile?.shown === false ? ['p1'] : [] }),
  )
})

describe('NotchProfileControls', () => {
  it('is a labelled switch that hides the profile from the notch', async () => {
    vi.mocked(loadAppState).mockResolvedValue(appState())
    renderWithQuery(<NotchProfileControls profileId="p1" />)

    const toggle = await screen.findByRole('switch', { name: 'In notch' })
    expect(toggle).toHaveAttribute('aria-checked', 'true')

    fireEvent.click(toggle)
    await waitFor(() =>
      expect(updateAppState).toHaveBeenCalledWith({ notchProfile: { profileId: 'p1', shown: false } }),
    )
    await waitFor(() => expect(toggle).toHaveAttribute('aria-checked', 'false'))
  })

  it('reads as off for a hidden profile', async () => {
    vi.mocked(loadAppState).mockResolvedValue(appState({ notchHiddenProfiles: ['p1'] }))
    renderWithQuery(<NotchProfileControls profileId="p1" />)

    expect(await screen.findByRole('switch', { name: 'In notch' })).toHaveAttribute('aria-checked', 'false')
  })

  it('is absent while the notch HUD is off', async () => {
    vi.mocked(loadAppState).mockResolvedValue(appState({ notchHudOff: true }))
    renderWithQuery(<NotchProfileControls profileId="p1" />)

    await waitFor(() => expect(loadAppState).toHaveBeenCalled())
    expect(screen.queryByRole('switch')).not.toBeInTheDocument()
  })

  it('gives the profile a five-day week', async () => {
    vi.mocked(loadAppState).mockResolvedValue(appState())
    renderWithQuery(<NotchProfileControls profileId="p1" />)

    fireEvent.click(await screen.findByRole('radio', { name: 'Five-day week, Monday to Friday' }))
    await waitFor(() => expect(updateAppState).toHaveBeenCalledWith({ workingWeek: { profileId: 'p1', days: 5 } }))
  })
})
