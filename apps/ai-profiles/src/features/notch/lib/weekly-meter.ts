import type { AppId } from '@/lib/app-registry'
import type { QuotaUsage, UsageWindow } from '@/lib/types'

import { codexWindowLabel } from '@/features/profiles/components/profile-detail-usage-card'

const weekMinutes = 7 * 24 * 60
const weekMs = weekMinutes * 60 * 1000

/**
 * What the notch HUD draws for one profile: its weekly limit, how far through
 * the week the clock is, and the short session window beside it.
 */
export type WeeklyMeter = {
  /** Share of the weekly limit used, 0-100 and beyond, or null when unknown. */
  percent: number | null
  /** Share of the week already gone, 0-1, or null when the reset is unknown. */
  elapsed: number | null
  resetsAt: Date | null
  /** The short window, e.g. Claude's 5-hour session, when the quota has one. */
  session: { label: string; percent: number } | null
}

/**
 * Pure: the window that holds a profile's weekly limit. Claude reports it as
 * `secondary`; Codex windows are positions, so its weekly one is whichever
 * lasts a week.
 */
function weeklyWindow(app: AppId, quota: QuotaUsage): UsageWindow | null {
  if (app === 'codex') {
    return [quota.primary, quota.secondary].find((window) => window?.windowDurationMins === weekMinutes) ?? null
  }
  return quota.secondary
}

/** Pure: the shorter window beside the weekly one, labelled for a tight row. */
function sessionWindow(app: AppId, quota: QuotaUsage): { label: string; window: UsageWindow } | null {
  if (app === 'codex') {
    const window = [quota.primary, quota.secondary].find(
      (candidate) => candidate && candidate.windowDurationMins !== weekMinutes,
    )
    return window ? { label: codexWindowLabel(window.windowDurationMins).shortLabel, window } : null
  }
  return quota.primary ? { label: '5h', window: quota.primary } : null
}

function parseDate(value: string | null): Date | null {
  if (!value) {
    return null
  }
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? null : date
}

/**
 * Pure: the share of the week gone by `now`, from the time it resets. A tick
 * at this point on the bar says whether usage is ahead of an even pace.
 */
export function elapsedFraction(resetsAt: Date, now: Date): number {
  const remaining = resetsAt.getTime() - now.getTime()
  return Math.min(1, Math.max(0, 1 - remaining / weekMs))
}

/** Pure: the notch HUD's reading of one profile's quota. */
export function weeklyMeter(app: AppId, quota: QuotaUsage, now: Date): WeeklyMeter {
  const weekly = weeklyWindow(app, quota)
  const resetsAt = parseDate(weekly?.resetsAt ?? null)
  const session = sessionWindow(app, quota)
  return {
    percent: weekly?.utilization ?? null,
    elapsed: resetsAt ? elapsedFraction(resetsAt, now) : null,
    resetsAt,
    session:
      session && session.window.utilization !== null
        ? { label: session.label, percent: session.window.utilization }
        : null,
  }
}

/**
 * Pure: the time to a reset in at most two units, `4d 23h`, `5h 12m`, `12m`.
 * Null once the reset has passed.
 */
export function formatResetsIn(resetsAt: Date, now: Date): string | null {
  const minutes = Math.ceil((resetsAt.getTime() - now.getTime()) / 60_000)
  if (minutes <= 0) {
    return null
  }
  const days = Math.floor(minutes / (24 * 60))
  const hours = Math.floor((minutes % (24 * 60)) / 60)
  const rest = minutes % 60
  if (days > 0) {
    return `${days}d ${hours}h`
  }
  if (hours > 0) {
    return `${hours}h ${rest}m`
  }
  return `${rest}m`
}

/** How pressed a limit is, for its colour. */
export type MeterLevel = 'ok' | 'warn' | 'full'

/** Pure: `warn` from 80% used, `full` at the limit. */
export function meterLevel(percent: number | null): MeterLevel {
  if (percent === null || percent < 80) {
    return 'ok'
  }
  return percent >= 100 ? 'full' : 'warn'
}

/**
 * Pure: the figure for the collapsed notch's wing — the most-used weekly
 * limit across profiles, so the tightest one shows without opening the HUD.
 */
export function highestPercent(meters: ReadonlyArray<WeeklyMeter | null>): number | null {
  let highest: number | null = null
  for (const meter of meters) {
    if (meter?.percent != null && (highest === null || meter.percent > highest)) {
      highest = meter.percent
    }
  }
  return highest
}

/** What the panel says about a usage alert. */
export type AlertCopy = { title: string; detail: string }

/**
 * Pure: the banner for a crossed threshold. Ahead of an even pace it is a
 * warning; on or behind pace, a milestone.
 */
export function alertCopy(
  alert: { window: 'weekly' | 'session'; windowMinutes: number; used: number; elapsed: number; ahead: boolean },
  profileName: string,
): AlertCopy {
  const limit = alert.window === 'weekly' ? 'weekly limit' : `${codexWindowLabel(alert.windowMinutes).shortLabel} limit`
  const period = alert.window === 'weekly' ? 'week' : 'window'
  return {
    title: alert.ahead ? 'Burn warning' : 'Usage milestone',
    detail: `${profileName} · ${limit} ${Math.round(alert.used)}% used, ${Math.round(alert.elapsed)}% of the ${period} gone`,
  }
}
