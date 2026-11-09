import { useEffect, useState } from "react";
import { fmtBytes, fmtDate, fmtUptime, userSessionStatus } from "../lib/format";
import {
  forceRefresh,
  setMaintenance,
  listNotes,
  addNote,
} from "../api";

export default function SystemDrawer({
  agentId,
  system: sys,
  apiKey,
  onClose,
  calendar,
  onMutated,
  onForceRefresh,
}) {
  const [notes, setNotes] = useState([]);
  const [newNote, setNewNote] = useState("");
  const [tab, setTab] = useState("overview");
  const [busy, setBusy] = useState(false);
  const [swFilter, setSwFilter] = useState("");

  useEffect(() => {
    if (!agentId) return;
    listNotes(apiKey, agentId)
      .then((r) => setNotes(r.items || []))
      .catch(() => setNotes([]));
  }, [agentId, apiKey]);

  async function doRefresh() {
    if (!agentId) return;
    setBusy(true);
    try {
      await forceRefresh(apiKey, agentId);
      onForceRefresh(agentId);
    } finally {
      setBusy(false);
    }
  }

  async function doMaint() {
    if (!agentId || !sys) return;
    setBusy(true);
    try {
      const next = !sys.maintenance;
      await setMaintenance(apiKey, agentId, next);
      await onMutated(agentId);
    } finally {
      setBusy(false);
    }
  }

  async function submitNote(e) {
    e.preventDefault();
    if (!newNote.trim() || !agentId) return;
    setBusy(true);
    try {
      await addNote(apiKey, agentId, newNote.trim());
      setNewNote("");
      const r = await listNotes(apiKey, agentId);
      setNotes(r.items || []);
    } finally {
      setBusy(false);
    }
  }

  if (!agentId) return null;

  const software = (sys?.installed_software || []).filter(
    (s) => !swFilter || s.name.toLowerCase().includes(swFilter.toLowerCase()),
  );
  const updates = sys?.updates || [];

  return (
    <>
      <div
        className="fixed inset-0 z-40"
        style={{ background: "rgba(0,0,0,0.5)" }}
        onClick={onClose}
      />
      <div
        className="fixed top-0 right-0 bottom-0 z-50 flex flex-col border-l overflow-hidden"
        style={{
          width: "min(560px, 100vw)",
          background: "var(--bg-panel)",
          borderColor: "var(--border-bright)",
        }}
      >
        <div
          className="px-4 py-3 border-b flex items-start justify-between"
          style={{ borderColor: "var(--border)" }}
        >
          <div>
            <div className="font-bold glow text-sm">
              {sys ? sys.computer_name : "..."}
            </div>
            <div className="text-xs mt-1" style={{ color: "var(--fg-mute)" }}>
              {sys ? sys.fqdn : ""}
            </div>
          </div>
          <button
            onClick={onClose}
            className="px-2 py-1 text-xs hover:opacity-70"
            style={{ color: "var(--fg-dim)" }}
          >
            ✕
          </button>
        </div>

        {sys && (
          <div
            className="px-4 py-2 border-b flex items-center gap-2 text-xs flex-wrap"
            style={{ borderColor: "var(--border)" }}
          >
            {/* NEW HEART BUTTON */}
            <button
              onClick={doRefresh}
              disabled={busy}
              title="Request a report at the next agent heartbeat"
              className="px-2 py-1 border hover:opacity-80 flex items-center gap-1"
              style={{ borderColor: "var(--red)", color: "var(--red)" }}
            >
              ❤️ PING
            </button>
            <button
              onClick={doRefresh}
              disabled={busy}
              className="px-2 py-1 border hover:opacity-80"
              style={{ borderColor: "var(--cyan)", color: "var(--cyan)" }}
            >
              ⟳ FORCE REFRESH
            </button>
            <button
              onClick={doMaint}
              disabled={busy}
              className="px-2 py-1 border hover:opacity-80"
              style={{
                borderColor: sys.maintenance ? "var(--amber)" : "var(--fg-dim)",
                color: sys.maintenance ? "var(--amber)" : "var(--fg-dim)",
              }}
            >
              {sys.maintenance ? "✓ IN MAINTENANCE" : "⚙ MAINTENANCE"}
            </button>
            <div className="flex-1" />
            <span style={{ color: "var(--fg-mute)" }}>
              SEEN {fmtDate(sys.last_seen, calendar)}
            </span>
          </div>
        )}

        <div
          className="px-4 py-1 border-b flex gap-3 text-xs"
          style={{ borderColor: "var(--border)" }}
        >
          {["overview", "software", "updates", "notes"].map((t) => (
            <button
              key={t}
              onClick={() => setTab(t)}
              className="py-2 px-1 border-b-2 uppercase"
              style={{
                borderColor: tab === t ? "var(--green)" : "transparent",
                color: tab === t ? "var(--green)" : "var(--fg-mute)",
              }}
            >
              {t}
              {t === "software" && sys && (
                <span className="ml-1" style={{ color: "var(--fg-mute)" }}>
                  ({sys.software_count})
                </span>
              )}
              {t === "updates" && sys && (
                <span className="ml-1" style={{ color: "var(--fg-mute)" }}>
                  ({sys.update_count})
                </span>
              )}
              {t === "notes" && (
                <span className="ml-1" style={{ color: "var(--fg-mute)" }}>
                  ({notes.length})
                </span>
              )}
            </button>
          ))}
        </div>

        <div className="flex-1 overflow-auto p-4 text-xs">
          {!sys && (
            <div style={{ color: "var(--fg-mute)" }}>
              loading<span className="blink">_</span>
            </div>
          )}

          {sys && tab === "overview" && (
            <div className="space-y-3">
              <Section title="IDENTITY">
                <Row label="Name" value={sys.computer_name} />
                <Row label="FQDN" value={sys.fqdn} />
                <Row label="Domain" value={sys.domain} />
                <Row label="IPs" value={(sys.ip_addresses || []).join(", ")} />
                <Row
                  label="MACs"
                  value={(sys.mac_addresses || []).join(", ")}
                />
                <Row label="Console User" value={sys.console_user || "—"} />
                <Row label="User Session" value={userSessionStatus(sys)} />
              </Section>

              <Section title="OS">
                <Row label="Name" value={sys.os_name} />
                <Row label="Version" value={sys.os_version} />
                <Row label="Build" value={sys.os_build} />
                <Row label="Arch" value={sys.os_arch} />
                <Row
                  label="Uptime"
                  value={fmtUptime(sys)}
                />
                <Row
                  label="Last Boot"
                  value={sys.last_boot ? fmtDate(sys.last_boot, calendar) : "—"}
                />
                <Row
                  label="Clock Skew"
                  value={
                    sys.clock_skew_secs != null
                      ? `${sys.clock_skew_secs}s`
                      : "—"
                  }
                />
                <Row
                  label="OS Activation"
                  value={
                    sys.windows_activation ||
                    (sys.platform === "Linux" ? "N/A" : "—")
                  }
                />
                <Row
                  label="Pending Reboot"
                  value={sys.pending_reboot == null ? "—" : sys.pending_reboot ? "⚠️ YES" : "No"}
                />
              </Section>

              <Section title="HARDWARE">
                <Row label="CPU" value={sys.cpu_model} />
                <Row label="Cores" value={sys.cpu_cores} />
                <Row label="RAM" value={fmtBytes(sys.ram_bytes)} />
                <Row label="Motherboard" value={sys.motherboard} />
                <Row label="GPU" value={sys.gpu_model || "—"} />
                <Row label="System Type" value={sys.system_type} />
              </Section>

              <Section title="STORAGE">
                <Row
                  label="Physical Disk"
                  value={fmtBytes(sys.main_storage_size)}
                />
                {(sys.drives || []).map((d) => (
                  <Row
                    key={d.letter}
                    label={`${d.letter} ${d.label ? "(" + d.label + ")" : ""} ${d.fs}`}
                    value={`${fmtBytes(d.free)} free / ${fmtBytes(d.total)} total${d.is_system ? "  [SYSTEM]" : ""}`}
                  />
                ))}
              </Section>

              <Section title="AGENT">
                <Row label="Agent ID" value={sys.agent_id} mono />
                <Row
                  label="Reported"
                  value={fmtDate(sys.reported_at, calendar)}
                />
                <Row
                  label="Last Seen"
                  value={fmtDate(sys.last_seen, calendar)}
                />
                <Row
                  label="First Seen"
                  value={fmtDate(sys.created_at, calendar)}
                />
                <Row
                  label="Maintenance"
                  value={sys.maintenance ? "YES" : "no"}
                />
                <Row
                  label="Refresh Pending"
                  value={sys.force_refresh ? "YES" : "no"}
                />
              </Section>
            </div>
          )}

          {sys && tab === "software" && (
            <div>
              <input
                type="text"
                value={swFilter}
                onChange={(e) => setSwFilter(e.target.value)}
                placeholder="filter software _"
                className="w-full bg-transparent border-b outline-none py-1 mb-3 text-xs"
                style={{ borderColor: "var(--border)", color: "var(--fg)" }}
              />
              <div className="space-y-1">
                {software.map((s, i) => (
                  <div
                    key={i}
                    className="flex justify-between gap-2 py-1 border-b"
                    style={{ borderColor: "var(--border)" }}
                  >
                    <div className="flex-1 truncate" title={s.name}>
                      {s.name}
                    </div>
                    <div style={{ color: "var(--fg-mute)" }}>
                      {s.version || "—"}
                    </div>
                  </div>
                ))}
                {software.length === 0 && (
                  <div style={{ color: "var(--fg-mute)" }}>
                    no matching software
                  </div>
                )}
              </div>
            </div>
          )}

          {sys && tab === "updates" && (
            <div className="space-y-1">
              {updates.map((u, i) => (
                <div
                  key={i}
                  className="flex justify-between gap-2 py-1 border-b"
                  style={{ borderColor: "var(--border)" }}
                >
                  <div>{u.kb}</div>
                  <div style={{ color: "var(--fg-mute)" }} className="truncate">
                    {u.description}
                  </div>
                  <div style={{ color: "var(--fg-dim)" }}>{u.installed_on}</div>
                </div>
              ))}
              {updates.length === 0 && (
                <div style={{ color: "var(--fg-mute)" }}>
                  no updates recorded
                </div>
              )}
            </div>
          )}

          {sys && tab === "notes" && (
            <div>
              <form onSubmit={submitNote} className="mb-4">
                <textarea
                  value={newNote}
                  onChange={(e) => setNewNote(e.target.value)}
                  placeholder="add a note _"
                  rows={3}
                  className="w-full bg-transparent border p-2 text-xs resize-none"
                  style={{ borderColor: "var(--border)", color: "var(--fg)" }}
                />
                <button
                  type="submit"
                  disabled={busy || !newNote.trim()}
                  className="mt-2 px-3 py-1 border text-xs hover:opacity-80 disabled:opacity-40"
                  style={{ borderColor: "var(--green)", color: "var(--green)" }}
                >
                  POST NOTE
                </button>
              </form>
              <div className="space-y-2">
                {notes.map((n) => (
                  <div
                    key={n.id}
                    className="border p-2"
                    style={{ borderColor: "var(--border)" }}
                  >
                    <div
                      className="text-[10px] mb-1"
                      style={{ color: "var(--fg-mute)" }}
                    >
                      {n.author || "anon"} · {fmtDate(n.created_at, calendar)}
                    </div>
                    <div className="whitespace-pre-wrap">{n.body}</div>
                  </div>
                ))}
                {notes.length === 0 && (
                  <div style={{ color: "var(--fg-mute)" }}>no notes yet</div>
                )}
              </div>
            </div>
          )}
        </div>
      </div>
    </>
  );
}

function Section({ title, children }) {
  return (
    <div>
      <div
        className="text-[10px] tracking-widest mb-1"
        style={{ color: "var(--fg-mute)" }}
      >
        ── {title} ──
      </div>
      <div className="space-y-0.5">{children}</div>
    </div>
  );
}

function Row({ label, value, mono }) {
  return (
    <div className="flex gap-2 py-0.5">
      <div className="w-32 shrink-0" style={{ color: "var(--fg-mute)" }}>
        {label}
      </div>
      <div className={"flex-1 break-all " + (mono ? "font-mono" : "")}>
        {value ?? "—"}
      </div>
    </div>
  );
}
