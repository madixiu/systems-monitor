export default function FilterBar({ filter, setFilter, domains, oses }) {
  function update(k, v) {
    setFilter((f) => ({ ...f, [k]: v }))
  }

  return (
    <div
      className="flex flex-wrap items-center gap-2 px-4 py-2 border-b text-xs"
      style={{ borderColor: 'var(--border)', background: 'var(--bg-panel)' }}
    >
      <span style={{ color: 'var(--green)' }}>&gt;</span>
      <input
        type="text"
        value={filter.q}
        onChange={(e) => update('q', e.target.value)}
        placeholder="filter _  (name / ip / mac / domain / user)"
        className="flex-1 min-w-48 bg-transparent border-b outline-none py-1 text-xs"
        style={{ borderColor: 'var(--border)', color: 'var(--fg)' }}
      />

      <select
        aria-label="Peripheral type"
        value={filter.peripheralKind}
        onChange={(e) => update('peripheralKind', e.target.value)}
        className="bg-transparent border px-2 py-1 text-xs"
        style={{ borderColor: 'var(--border)', color: 'var(--fg-dim)' }}
      >
        <option value="">peripheral: all</option>
        <option value="printer">printer</option>
        <option value="monitor">monitor</option>
        <option value="imaging">imaging</option>
      </select>

      <input
        type="text"
        aria-label="Peripheral name prefix"
        value={filter.peripheralName}
        onChange={(e) => update('peripheralName', e.target.value)}
        placeholder="device name starts with..."
        className="bg-transparent border-b outline-none py-1 text-xs"
        style={{ borderColor: 'var(--border)', color: 'var(--fg)' }}
      />

      <select
        value={filter.domain}
        onChange={(e) => update('domain', e.target.value)}
        className="bg-transparent border px-2 py-1 text-xs"
        style={{ borderColor: 'var(--border)', color: 'var(--fg-dim)' }}
      >
        <option value="">domain: all</option>
        {domains.map((d) => (
          <option key={d} value={d} style={{ background: 'var(--bg-panel)' }}>{d}</option>
        ))}
      </select>

      <select
        value={filter.os}
        onChange={(e) => update('os', e.target.value)}
        className="bg-transparent border px-2 py-1 text-xs"
        style={{ borderColor: 'var(--border)', color: 'var(--fg-dim)' }}
      >
        <option value="">os: all</option>
        {oses.map((o) => (
          <option key={o} value={o} style={{ background: 'var(--bg-panel)' }}>{o}</option>
        ))}
      </select>

      <select
        value={filter.state}
        onChange={(e) => update('state', e.target.value)}
        className="bg-transparent border px-2 py-1 text-xs"
        style={{ borderColor: 'var(--border)', color: 'var(--fg-dim)' }}
      >
        <option value="all">state: all</option>
        <option value="online">online</option>
        <option value="offline">offline</option>
        <option value="maintenance">maintenance</option>
      </select>
    </div>
  )
}
