export const API_URL = import.meta.env.VITE_API_URL || 'http://172.16.235.235:8080'

export async function apiFetch(path, key, opts = {}) {
  const res = await fetch(`${API_URL}${path}`, {
    ...opts,
    headers: {
      'Content-Type': 'application/json',
      'X-API-Key': key,
      ...(opts.headers || {}),
    },
  })
  if (!res.ok) {
    const text = await res.text().catch(() => '')
    throw new Error(`${res.status} ${res.statusText}: ${text}`)
  }
  if (res.status === 204) return null
  const ct = res.headers.get('content-type') || ''
  return ct.includes('application/json') ? res.json() : res.text()
}

export async function listSystems(key, filters = {}) {
  const params = new URLSearchParams({ limit: '2000' })
  if (filters.peripheralKind) params.set('peripheral_kind', filters.peripheralKind)
  if (filters.peripheralName?.trim()) params.set('peripheral_name', filters.peripheralName.trim())
  return apiFetch(`/api/v1/systems?${params}`, key)
}

export async function getSystem(key, id) {
  return apiFetch(`/api/v1/systems/${id}`, key)
}

export async function getStats(key) {
  return apiFetch('/api/v1/stats', key)
}

export async function forceRefresh(key, id) {
  return apiFetch(`/api/v1/systems/${id}/force-refresh`, key, { method: 'POST' })
}

export async function setMaintenance(key, id, on) {
  return apiFetch(`/api/v1/systems/${id}/maintenance`, key, {
    method: 'POST',
    body: JSON.stringify({ on }),
  })
}

export async function listNotes(key, id) {
  return apiFetch(`/api/v1/systems/${id}/notes`, key)
}

export async function addNote(key, id, body, author = 'admin') {
  return apiFetch(`/api/v1/systems/${id}/notes`, key, {
    method: 'POST',
    body: JSON.stringify({ body, author }),
  })
}

export async function listAgents(key) {
  return apiFetch('/api/v1/agents', key)
}

export async function revokeAgent(key, agentId) {
  return apiFetch(`/api/v1/agents/${agentId}/revoke`, key, { method: 'POST' })
}

export async function reinstateAgent(key, agentId) {
  return apiFetch(`/api/v1/agents/${agentId}/reinstate`, key, { method: 'POST' })
}