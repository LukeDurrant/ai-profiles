import type { AppState, Profile, SidebarEntry } from '@/lib/types'
import type { NotchState } from '../api/notch-ipc'

import { fireEvent, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { fetchUsableUsage } from '@/features/profiles/api/use-profile-usage'
import { useSidebarEntries } from '@/features/profiles/api/use-sidebar-entries'
import { useAppState } from '@/lib/app-state/use-app-state'
import { renderWithQuery } from '@/test/render-with-query'

import { getNotchState, onNotchState, openProfileFromNotch } from '../api/notch-ipc'
import { NotchHud } from './notch-hud'

vi.mock('../api/notch-ipc', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api/notch-ipc')>()),
  getNotchState: vi.fn(),
  onNotchState: vi.fn(),
  openProfileFromNotch: vi.fn(),
  setNotchContentHeight: vi.fn(),
}))
vi.mock('@/features/profiles/api/use-sidebar-entries', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/features/profiles/api/use-sidebar-entries')>()),
  useSidebarEntries: vi.fn(),
}))
vi.mock('@/features/profiles/api/use-profile-usage', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/features/profiles/api/use-profile-usage')>()),
  fetchUsableUsage: vi.fn(),
}))
vi.mock('@/lib/app-state/use-app-state', () => ({ useAppState: vi.fn() }))

const expanded: NotchState = {
  phase: 'expanded',
  notch: 189,
  height: 32,
  wing: 30,
  fillet: 19,
  bodyWidth: 320,
  bodyMaxHeight: 1000,
}

function managed(id: string, name: string, cli = true): SidebarEntry {
  const profile: Profile = {
    id,
    app: 'claude',
    name,
    slug: id,
    color: '#7c9a6e',
    createdAt: '2026-01-01T00:00:00Z',
    surfaces: { gui: true, cli },
    distinctDockIcon: false,
    lastUsedAt: null,
  }
  return { kind: 'managed', profile }
}

function appState(notchHiddenProfiles: Array<string> = []): AppState {
  return {
    welcomeShown: true,
    migrationDismissedAt: null,
    pathBannerDismissedAt: null,
    themeMode: 'system',
    selectedEntryId: null,
    dockIconAcknowledgedAt: null,
    defaultProfileNames: {},
    dismissedRepairSessions: {},
    notchHudOff: false,
    notchHiddenProfiles,
  }
}

function given({ entries, hidden = [] }: { entries: Array<SidebarEntry>; hidden?: Array<string> }) {
  vi.mocked(useSidebarEntries).mockReturnValue(entries)
  vi.mocked(useAppState).mockReturnValue({
    state: appState(hidden),
    update: vi.fn(),
    refresh: vi.fn(),
  })
}

beforeEach(() => {
  vi.mocked(getNotchState).mockResolvedValue(expanded)
  vi.mocked(onNotchState).mockResolvedValue(() => undefined)
  vi.mocked(fetchUsableUsage).mockImplementation(async (id) => ({
    quota: {
      primary: { utilization: 12, resetsAt: new Date(Date.now() + 3 * 3600_000).toISOString() },
      secondary: {
        utilization: id === 'work' ? 32 : 85,
        resetsAt: new Date(Date.now() + (4 * 24 + 23) * 3600_000 + 30_000).toISOString(),
      },
      scopedWeekly: [],
    },
    quotaError: null,
    fetchedAt: new Date().toISOString(),
  }))
})

describe('NotchHud', () => {
  it('shows each profile’s name, weekly limit and reset time', async () => {
    given({ entries: [managed('work', 'Work'), managed('home', 'Home')] })
    renderWithQuery(<NotchHud />)

    expect(await screen.findByText('32%')).toBeInTheDocument()
    expect(screen.getByText('Work')).toBeInTheDocument()
    expect(screen.getByText('Home')).toBeInTheDocument()
    expect(screen.getAllByText('Resets in 4d 23h')).toHaveLength(2)
    expect(screen.getAllByText('5h 12%')).toHaveLength(2)
    // The folded wing carries the tightest limit.
    expect(screen.getAllByText('85%')).toHaveLength(2)
  })

  it('leaves out profiles hidden from the notch and profiles without a CLI', async () => {
    given({
      entries: [managed('work', 'Work'), managed('home', 'Home'), managed('gui', 'Desktop only', false)],
      hidden: ['home'],
    })
    renderWithQuery(<NotchHud />)

    expect(await screen.findByText('Work')).toBeInTheDocument()
    expect(screen.queryByText('Home')).not.toBeInTheDocument()
    expect(screen.queryByText('Desktop only')).not.toBeInTheDocument()
  })

  it('opens the clicked profile in the main window', async () => {
    given({ entries: [managed('work', 'Work')] })
    renderWithQuery(<NotchHud />)

    fireEvent.click(await screen.findByText('Work'))
    expect(openProfileFromNotch).toHaveBeenCalledWith('work')
  })
})
