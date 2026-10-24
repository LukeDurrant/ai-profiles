import type { SegmentedOption } from '@/design'
import type { AppState, AppStatePatch } from '@/lib/types'

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { PortalTooltip, Segmented } from '@/design'
import { computeOptimisticAppState } from '@/lib/app-state/use-app-state'
import { loadAppState, updateAppState } from '@/lib/commands'
import { queryKeys } from '@/lib/query/keys'

/**
 * Profile `profileId`'s notch settings, and the way to change them.
 *
 * Reads the app state without suspending: the controls sit in the usage
 * card's header, and a missing reading only hides them, never the card.
 */
function useNotchProfile(profileId: string) {
  const queryClient = useQueryClient()
  const { data } = useQuery({ queryKey: queryKeys.appState, queryFn: () => loadAppState() })
  const mutation = useMutation({
    mutationFn: (patch: AppStatePatch) => updateAppState(patch),
    onMutate: (patch) => {
      const previous = queryClient.getQueryData<AppState>(queryKeys.appState)
      if (previous) {
        queryClient.setQueryData(queryKeys.appState, computeOptimisticAppState(previous, patch))
      }
      return { previous }
    },
    onError: (_error, _patch, context) => {
      if (context?.previous) {
        queryClient.setQueryData(queryKeys.appState, context.previous)
      }
    },
    onSuccess: (next) => {
      queryClient.setQueryData(queryKeys.appState, next)
    },
  })
  if (!data) {
    return null
  }
  return {
    shown: !data.notchHiddenProfiles.includes(profileId),
    week: (data.fiveDayWeekProfiles.includes(profileId) ? '5' : '7') as WeekOption,
    hudOff: data.notchHudOff,
    setShown: (shown: boolean) => mutation.mutate({ notchProfile: { profileId, shown } }),
    setWeek: (week: WeekOption) => mutation.mutate({ workingWeek: { profileId, days: week === '5' ? 5 : 7 } }),
  }
}

type WeekOption = '5' | '7'

const weekOptions: ReadonlyArray<SegmentedOption<WeekOption>> = [
  { value: '5', label: '5d', ariaLabel: 'Five-day week, Monday to Friday' },
  { value: '7', label: '7d', ariaLabel: 'Seven-day week' },
]

/**
 * The usage card's notch controls for one profile: whether it shows in the
 * notch, and whether its weekly pace counts five days or seven. Absent while
 * the notch HUD is off as a whole.
 */
export function NotchProfileControls({ profileId }: { profileId: string }) {
  const notch = useNotchProfile(profileId)
  if (!notch || notch.hudOff) {
    return null
  }
  return (
    <>
      <PortalTooltip label="Working week for the weekly pace">
        <span className="inline-flex">
          <Segmented<WeekOption>
            size="small"
            ariaLabel="Working week"
            options={weekOptions}
            value={notch.week}
            onChange={notch.setWeek}
          />
        </span>
      </PortalTooltip>
      <button
        type="button"
        role="switch"
        aria-checked={notch.shown}
        onClick={() => notch.setShown(!notch.shown)}
        className="group inline-flex min-h-[22px] cursor-pointer items-center gap-1.5 rounded px-1 font-mono text-mono text-muted-strong transition-colors duration-(--duration-snap) ease-(--ease-natural) hover:text-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"
      >
        In notch
        <span
          aria-hidden
          className={`relative inline-block h-[14px] w-[24px] rounded-full transition-colors duration-(--duration-snap) ease-(--ease-natural) ${notch.shown ? 'bg-(--app-accent)' : 'bg-ink/20'}`}
        >
          <span
            className={`absolute top-[2px] left-[2px] h-[10px] w-[10px] rounded-full bg-white shadow-sm transition-transform duration-(--duration-snap) ease-(--ease-natural) ${notch.shown ? 'translate-x-[10px]' : ''}`}
          />
        </span>
      </button>
    </>
  )
}
