import { useState } from 'react'
import { API_URL } from '../api'

export default function KeyGate({ onSubmit }) {
  const [key, setKey] = useState('')
  const [error, setError] = useState('')

  async function handleSubmit(e) {
    e.preventDefault()
    if (!key.trim()) return
    setError('')
    try {
      const res = await fetch(`${API_URL}/api/v1/stats`, {
        headers: { 'X-API-Key': key.trim() },
      })
      if (!res.ok) {
        setError('invalid API key')
        return
      }
      onSubmit(key.trim())
    } catch (err) {
      setError('cannot reach server: ' + err.message)
    }
  }

  return (
    <div className="h-full flex items-center justify-center p-8">
      <form onSubmit={handleSubmit} className="w-full max-w-md">
        <div className="mb-8 glow">
          <div className="text-2xl font-bold">PAK://MONITOR</div>
          <div className="text-xs mt-2" style={{ color: 'var(--fg-mute)' }}>
            enter API key to continue
          </div>
        </div>

        <div className="mb-4">
          <div className="text-xs mb-2" style={{ color: 'var(--fg-dim)' }}>
            API_ENDPOINT: {API_URL}
          </div>
          <div className="flex items-center gap-2">
            <span style={{ color: 'var(--green)' }}>&gt;</span>
            <input
              type="password"
              autoFocus
              value={key}
              onChange={(e) => setKey(e.target.value)}
              placeholder="64-char hex key"
              className="flex-1 bg-transparent border px-3 py-2 text-sm"
              style={{ borderColor: 'var(--border)', color: 'var(--fg)' }}
            />
          </div>
        </div>

        {error && (
          <div className="mb-4 text-xs" style={{ color: 'var(--red)' }}>
            [ERR] {error}
          </div>
        )}

        <button
          type="submit"
          className="w-full border px-4 py-2 text-sm hover:opacity-80 transition"
          style={{ borderColor: 'var(--green)', color: 'var(--green)' }}
        >
          CONNECT<span className="blink">_</span>
        </button>
      </form>
    </div>
  )
}
