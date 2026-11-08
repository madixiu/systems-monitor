import { useEffect, useState } from 'react'
import { listAgents, revokeAgent, reinstateAgent } from '../api'
import { fmtDate, relTime } from '../lib/format'

export default function AgentsPanel({ apiKey, calendar, onClose }) {
  const [agents, setAgents] = useState([])
  const [loading, setLoading] = useState(true)
  const [filter, setFilter] = useState('')

  async function load() {
    setLoading(true)
    try {
      const r = await listAgents(apiKey)
      setAgents(r.items || [])
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => { load() }, [apiKey])

  async function toggle(a) {
    if (a.revoked) await reinstateAgent(apiKey, a.agent_id)
    else {
      if (!confirm(`Revoke token for ${a.computer_name || a.agent_id}?\nThe agent will need to re-enroll.`)) return
      await revokeAgent(apiKey, a.agent_id)
    }
    await load()
  }

  const shown = agents.filter((a) => {
    if (!filter) return true
    const q = filter.toLowerCase()
    return (
      (a.computer_name || '').toLowerCase().includes(q) ||
      a.agent_id.toLowerCase().includes(q)
    )
  })

  return (
    <>
      <div className="fixed inset-0 z-40" style={{ background: 'rgba(0,0,0,0.5)' }} onClick={onClose} />
      <div
        className="fixed top-0 right-0 bottom-0 z-50 flex flex-col border-l overflow-hidden"
        style={{ width: 'min(720px, 100vw)', background: 'var(--bg-panel)', borderColor: 'var(--border-bright)' }}
      >
        <div className="px-4 py-3 border-b flex items-center justify-between" style={{ borderColor: 'var(--border)' }}>
          <div>
            <div className="font-bold glow text-sm">AGENT REGISTRY</div>
            <div className="text-xs mt-1" style={{ color: 'var(--fg-mute)' }}>
              {agents.filter(a => !a.revoked).length} active · {agents.filter(a => a.revoked).length} revoked
            </div>
          </div>
          <button onClick={onClose} className="px-2 py-1 text-xs hover:opacity-70" style={{ color: 'var(--fg-dim)' }}>✕</button>
        </div>

        <div className="px-4 py-2 border-b" style={{ borderColor: 'var(--border)' }}>
          <input
            type="text"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder="filter by hostname or agent id _"
            className="w-full bg-transparent border-b outline-none py-1 text-xs"
            style={{ borderColor: 'var(--border)', color: 'var(--fg)' }}
          />
        </div>

        <div className="flex-1 overflow-auto">
          {loading && <div className="p-4 text-xs" style={{ color: 'var(--fg-mute)' }}>loading<span className="blink">_</span></div>}
          {!loading && shown.length === 0 && (
            <div className="p-4 text-xs" style={{ color: 'var(--fg-mute)' }}>no agents match</div>
          )}
          {!loading && shown.length > 0 && (
            <table className="w-full text-xs border-collapse">
              <thead style={{ background: 'var(--bg-panel)' }}>
                <tr style={{ color: 'var(--fg-dim)', borderBottom: '1px solid var(--border)' }}>
                  <th className="text-left px-3 py-1 font-normal">HOSTNAME</th>
                  <th className="text-left px-3 py-1 font-normal">AGENT ID</th>
                  <th className="text-left px-3 py-1 font-normal">ENROLLED</th>
                  <th className="text-left px-3 py-1 font-normal">LAST SEEN</th>
                  <th className="text-center px-3 py-1 font-normal w-20">STATE</th>
                  <th className="text-right px-3 py-1 font-normal w-24"></th>
                </tr>
              </thead>
              <tbody>
                {shown.map((a, i) => (
                  <tr key={a.agent_id} style={{
                    background: i % 2 === 0 ? 'var(--bg-row)' : 'var(--bg-row-alt)',
                    borderBottom: '1px solid var(--border)',
                    opacity: a.revoked ? 0.55 : 1,
                  }}>
                    <td className="px-3 py-1 truncate max-w-[160px]">{a.computer_name || '—'}</td>
                    <td className="px-3 py-1 font-mono" style={{ color: 'var(--fg-mute)' }} title={a.agent_id}>
                      {a.agent_id.slice(0, 8)}…{a.agent_id.slice(-4)}
                    </td>
                    <td className="px-3 py-1" style={{ color: 'var(--fg-dim)' }}>{fmtDate(a.enrolled_at, calendar)}</td>
                    <td className="px-3 py-1" style={{ color: 'var(--fg-dim)' }}>
                      {a.last_seen ? relTime(a.last_seen) : '—'}
                    </td>
                    <td className="px-3 py-1 text-center">
                      <span style={{ color: a.revoked ? 'var(--red)' : 'var(--green)' }}>
                        {a.revoked ? 'REVOKED' : 'ACTIVE'}
                      </span>
                    </td>
                    <td className="px-3 py-1 text-right">
                      <button
                        onClick={() => toggle(a)}
                        className="px-2 py-1 border text-[10px] hover:opacity-80"
                        style={{
                          borderColor: a.revoked ? 'var(--green)' : 'var(--red)',
                          color: a.revoked ? 'var(--green)' : 'var(--red)',
                        }}
                      >
                        {a.revoked ? 'REINSTATE' : 'REVOKE'}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </>
  )
}