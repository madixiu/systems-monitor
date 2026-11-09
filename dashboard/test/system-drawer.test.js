import { after, test } from 'node:test'
import assert from 'node:assert/strict'
import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { createServer } from 'vite'
import { currentLiveState, fmtUptime, userSessionStatus } from '../src/lib/format.js'

const vite = await createServer({ server: { middlewareMode: true }, appType: 'custom' })
after(() => vite.close())
const { default: SystemDrawer } = await vite.ssrLoadModule('/src/components/SystemDrawer.jsx')
const { default: HackTable } = await vite.ssrLoadModule('/src/components/HackTable.jsx')

function renderSystem(system) {
  return renderToStaticMarkup(createElement(SystemDrawer, {
    agentId: 'agent-1',
    system: { agent_id: 'agent-1', computer_name: 'test-host', ...system },
    apiKey: 'test-key',
    calendar: 'gregorian',
  }))
}

test('displays the current health fields, including zero and false', () => {
  const html = renderSystem({
    platform: 'Windows',
    uptime_secs: 0,
    clock_skew_secs: 0,
    pending_reboot: false,
    gpu_model: 'Test GPU',
    windows_activation: 'Activated',
  })
  assert.match(html, /0d 0h/)
  assert.match(html, /0s/)
  assert.match(html, />No</)
  assert.match(html, /Test GPU/)
  assert.match(html, /Activated/)
})

test('shows missing health fields as unknown and renders updated reports', () => {
  const initial = renderSystem({ platform: 'Linux' })
  assert.doesNotMatch(initial, /undefineds|>No</)
  assert.match(initial, /N\/A/)

  const updated = renderSystem({
    uptime_secs: 90000,
    clock_skew_secs: -4,
    pending_reboot: true,
    gpu_model: 'Updated GPU',
  })
  assert.match(updated, /1d 1h/)
  assert.match(updated, /-4s/)
  assert.match(updated, /YES/)
  assert.match(updated, /Updated GPU/)
})

test('uses the server offline threshold rather than a fixed heartbeat interval', () => {
  const now = Date.parse('2026-01-01T00:10:00Z')
  const system = {
    last_seen: '2026-01-01T00:05:00Z',
    offline_threshold_secs: 600,
    live_state: 'online',
  }
  assert.equal(currentLiveState(system, now), 'online')
  assert.equal(currentLiveState(system, now + 301000), 'offline')
  assert.equal(currentLiveState({ ...system, maintenance: true }, now), 'maintenance')
  assert.equal(currentLiveState({ ...system, last_seen: null }, now), 'never')
  assert.equal(currentLiveState({ ...system, offline_threshold_secs: undefined }, now), 'online')
})

test('advances uptime from the last boot while online and freezes it when offline', () => {
  const now = Date.parse('2026-01-02T00:00:00Z')
  const system = {
    last_boot: '2026-01-01T22:30:00Z',
    last_seen: '2026-01-01T23:59:30Z',
    offline_threshold_secs: 600,
    uptime_secs: 0,
  }
  assert.equal(fmtUptime(system, now), '0d 1h 30m')
  assert.equal(fmtUptime(system, now + 3600000), '0d 1h 29m')
  assert.equal(fmtUptime({ uptime_secs: 42 }, now), '0d 0h 0m')
  assert.equal(fmtUptime({}, now), '—')
})

test('shows a user badge for signed in, signed out and unknown states', () => {
  const system = {
    agent_id: 'agent-1', computer_name: 'test-host', console_user: 'DOMAIN\\alice',
    last_seen: new Date().toISOString(), offline_threshold_secs: 600, live_state: 'online',
    reported_at: new Date().toISOString(), drives: [],
  }
  function render(status, overrides = {}) {
    return renderToStaticMarkup(createElement(HackTable, {
      systems: [{ ...system, console_user_logged_in: status, ...overrides }],
      pendingRefresh: {}, flashSet: {}, sort: {}, calendar: 'gregorian',
    }))
  }
  assert.match(render(true), /● IN/)
  assert.match(render(false), /○ OUT/)
  assert.match(render(null), /\? UNKNOWN/)
  assert.match(render(true, { last_seen: '2020-01-01T00:00:00Z' }), /\? UNKNOWN/)
  assert.match(render(false), /alice/)
  assert.equal(userSessionStatus({ ...system, console_user_logged_in: false }), 'signed out')
})
