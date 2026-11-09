export function fmtBytes(n) {
  if (!n || n === 0) return '—'
  const gb = n / 1024 / 1024 / 1024
  if (gb >= 1000) return (gb / 1024).toFixed(2) + ' TB'
  return gb.toFixed(2) + ' GB'
}

export function fmtPct(free, total) {
  if (!total) return '—'
  const p = (free / total) * 100
  return p.toFixed(0) + '%'
}

export function diskColor(free, total) {
  if (!total) return 'var(--fg-mute)'
  const pct = (free / total) * 100
  if (pct < 10) return 'var(--red)'
  if (pct < 25) return 'var(--amber)'
  return 'var(--fg)'
}

// Gregorian -> Jalali (Persian calendar). Returns [year, month, day].
export function g2j(gy, gm, gd) {
  const g_d_m = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334]
  const gy2 = gm > 2 ? gy + 1 : gy
  let days = 355666 + (365 * gy) + Math.floor((gy2 + 3) / 4)
    - Math.floor((gy2 + 99) / 100) + Math.floor((gy2 + 399) / 400)
    + gd + g_d_m[gm - 1]
  let jy = -1595 + 33 * Math.floor(days / 12053)
  days %= 12053
  jy += 4 * Math.floor(days / 1461)
  days %= 1461
  if (days > 365) { jy += Math.floor((days - 1) / 365); days = (days - 1) % 365 }
  const jm = days < 186 ? 1 + Math.floor(days / 31) : 7 + Math.floor((days - 186) / 30)
  const jd = 1 + (days < 186 ? days % 31 : (days - 186) % 30)
  return [jy, jm, jd]
}

const MONTHS_FA = ['Farvardin','Ordibehesht','Khordad','Tir','Mordad','Shahrivar','Mehr','Aban','Azar','Dey','Bahman','Esfand']

// Format a timestamp; calendar = 'gregorian' | 'jalali'
export function fmtDate(iso, calendar = 'gregorian') {
  if (!iso) return '—'
  const d = new Date(iso)
  if (isNaN(d.getTime())) return '—'
  const pad = (n) => String(n).padStart(2, '0')
  if (calendar === 'jalali') {
    const [jy, jm, jd] = g2j(d.getFullYear(), d.getMonth() + 1, d.getDate())
    return `${jy}-${pad(jm)}-${pad(jd)} ${pad(d.getHours())}:${pad(d.getMinutes())}`
  }
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

export function relTime(iso) {
  if (!iso) return '—'
  const d = new Date(iso)
  if (isNaN(d.getTime())) return '—'
  const s = Math.floor((Date.now() - d.getTime()) / 1000)
  if (s < 0) return 'now'
  if (s < 60) return s + 's ago'
  if (s < 3600) return Math.floor(s / 60) + 'm ago'
  if (s < 86400) return Math.floor(s / 3600) + 'h ago'
  return Math.floor(s / 86400) + 'd ago'
}

export function isStale(iso, thresholdSecs = 90) {
  if (!iso) return true
  const d = new Date(iso)
  if (isNaN(d.getTime())) return true
  return Date.now() - d.getTime() > thresholdSecs * 1000
}

export function currentLiveState(system, now = Date.now()) {
  if (system.maintenance) return 'maintenance'
  if (!system.last_seen) return 'never'
  const seen = Date.parse(system.last_seen)
  if (!Number.isFinite(seen) || typeof system.offline_threshold_secs !== 'number') {
    return system.live_state
  }
  return now - seen > system.offline_threshold_secs * 1000 ? 'offline' : 'online'
}

export function isObservationFresh(system, now = Date.now()) {
  const seen = Date.parse(system.last_seen)
  if (Number.isFinite(seen) && typeof system.offline_threshold_secs === 'number') {
    return now - seen <= system.offline_threshold_secs * 1000
  }
  return system.live_state === 'online'
}

export function userSessionStatus(system, now = Date.now()) {
  if (!isObservationFresh(system, now) || system.console_user_logged_in == null) return 'unknown'
  return system.console_user_logged_in ? 'signed in' : 'signed out'
}

export function fmtUptime(system, now = Date.now()) {
  const boot = Date.parse(system.last_boot)
  const seen = Date.parse(system.last_seen)
  const end = isObservationFresh(system, now) ? now : seen
  let seconds = system.uptime_secs
  if (Number.isFinite(boot) && Number.isFinite(end) && end >= boot) {
    seconds = Math.floor((end - boot) / 1000)
  }
  if (typeof seconds !== 'number' || !Number.isFinite(seconds) || seconds < 0) return '—'
  return `${Math.floor(seconds / 86400)}d ${Math.floor((seconds % 86400) / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`
}
