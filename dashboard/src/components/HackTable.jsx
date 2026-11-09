import {
  fmtBytes,
  fmtPct,
  diskColor,
  relTime,
  fmtDate,
  userSessionStatus,
} from "../lib/format";

export default function HackTable({
  systems,
  selected,
  onSelect,
  calendar,
  pendingRefresh,
  flashSet,
  sort,
  onSort,
}) {
  if (systems.length === 0) {
    return (
      <div
        className="flex items-center justify-center h-full text-xs"
        style={{ color: "var(--fg-mute)" }}
      >
        NO RESULTS<span className="blink ml-1">_</span>
      </div>
    );
  }

  return (
    <div className="overflow-auto h-full">
      <table className="w-full border-collapse text-xs">
        <thead
          className="sticky top-0 z-10"
          style={{ background: "var(--bg-panel)" }}
        >
          <tr
            style={{
              color: "var(--fg-dim)",
              borderBottom: "1px solid var(--border)",
            }}
          >
            <th className="text-left px-2 py-1 font-normal w-6"></th>
            <Th k="computer_name" sort={sort} onSort={onSort}>HOSTNAME</Th>
            <Th k="ip" sort={sort} onSort={onSort}>IP</Th>
            <Th k="mac" sort={sort} onSort={onSort}>MAC</Th>
            <Th k="user" sort={sort} onSort={onSort}>USER</Th>
            <Th k="os" sort={sort} onSort={onSort}>OS</Th>
            <Th k="cpu" sort={sort} onSort={onSort}>CPU</Th>
            <Th k="ram" sort={sort} onSort={onSort} align="right">RAM</Th>
            <Th k="disk_free" sort={sort} onSort={onSort} align="right">DISK C:</Th>
            <Th k="last_seen" sort={sort} onSort={onSort} align="right">LAST SEEN</Th>
            <th className="text-center px-2 py-1 font-normal w-8"></th>
          </tr>
        </thead>
        <tbody>
          {systems.map((s, i) => {
            const state = !s.reported_at
              ? "new"
              : s.maintenance
                ? "maintenance"
                : s.live_state === "online"
                  ? "online"
                  : "offline";

            const isSel = selected === s.agent_id;
            const isPending = pendingRefresh[s.agent_id];
            const isFlashing = flashSet[s.agent_id];

            const sysDrive = (s.drives || []).find((d) => d.is_system);
            const free = sysDrive?.free ?? s.system_drive_free;
            const total = sysDrive?.total ?? s.system_drive_total;

            const ip = (s.ip_addresses && s.ip_addresses[0]) || "—";
            const mac =
              s.mac_addresses && s.mac_addresses[0]
                ? s.mac_addresses[0].toUpperCase()
                : "—";
            const rawUser = s.console_user || "";
            const user = rawUser.includes("\\")
              ? rawUser.split("\\").pop()
              : rawUser || "—";
            const session = userSessionStatus(s);
            const sessionColor = session === "signed in"
              ? "var(--green)"
              : session === "signed out"
                ? "var(--amber)"
                : "var(--fg-mute)";

            return (
              <tr
                key={s.agent_id}
                onClick={() => onSelect(s.agent_id)}
                className={isFlashing ? "row-flash" : ""}
                style={{
                  background: isSel
                    ? "var(--bg-selected)"
                    : i % 2 === 0
                      ? "var(--bg-row)"
                      : "var(--bg-row-alt)",
                  borderBottom: "1px solid var(--border)",
                  cursor: "pointer",
                }}
                onMouseEnter={(e) => {
                  if (!isSel)
                    e.currentTarget.style.background = "var(--bg-hover)";
                }}
                onMouseLeave={(e) => {
                  if (!isSel)
                    e.currentTarget.style.background = isSel
                      ? "var(--bg-selected)"
                      : i % 2 === 0
                        ? "var(--bg-row)"
                        : "var(--bg-row-alt)";
                }}
              >
                <td className="px-2 py-1">
                  <StatusDot state={state} />
                </td>
                <td
                  className="px-2 py-1 glow-soft truncate max-w-[180px]"
                  title={s.fqdn}
                >
                  {s.computer_name}
                  {s.maintenance && (
                    <span
                      className="ml-2 text-[10px]"
                      style={{ color: "var(--amber)" }}
                    >
                      [MAINT]
                    </span>
                  )}
                </td>
                <td className="px-2 py-1" style={{ color: "var(--fg-dim)" }}>
                  {ip}
                </td>
                <td
                  className="px-2 py-1 font-mono text-[11px]"
                  style={{ color: "var(--fg-dim)" }}
                  title={mac}
                >
                  {mac}
                </td>
                <td
                  className="px-2 py-1 max-w-[180px]"
                  style={{
                    color: user === "—" ? "var(--fg-mute)" : "var(--fg-dim)",
                  }}
                  title={`${rawUser || "No user recorded"} · ${session}`}
                >
                  <div className="flex items-center gap-2">
                    <span className="truncate">{user}</span>
                    <span
                      className="shrink-0 rounded-full px-1.5 py-0.5 text-[9px] whitespace-nowrap"
                      style={{
                        color: sessionColor,
                        border: `1px solid ${sessionColor}`,
                      }}
                    >
                      {session === "signed in" ? "● IN" : session === "signed out" ? "○ OUT" : "?"}
                    </span>
                  </div>
                </td>
                <td
                  className="px-2 py-1 truncate max-w-[160px]"
                  style={{ color: "var(--fg-dim)" }}
                  title={s.os_name}
                >
                  {s.os_name}{" "}
                  <span style={{ color: "var(--fg-mute)" }}>{s.os_build}</span>
                </td>
                <td
                  className="px-2 py-1 truncate max-w-[160px]"
                  style={{ color: "var(--fg-dim)" }}
                  title={s.cpu_model}
                >
                  {shortCpu(s.cpu_model)}
                </td>
                <td
                  className="px-2 py-1 text-right"
                  style={{ color: "var(--fg-dim)" }}
                >
                  {fmtBytes(s.ram_bytes)}
                </td>
                <td
                  className="px-2 py-1 text-right"
                  style={{ color: diskColor(free, total) }}
                >
                  {fmtPct(free, total)}
                </td>
                <td
                  className="px-2 py-1 text-right"
                  style={{ color: "var(--fg-dim)" }}
                  title={fmtDate(s.last_seen, calendar)}
                >
                  {relTime(s.last_seen)}
                </td>
                <td className="px-2 py-1 text-center">
                  {isPending && (
                    <span className="blink" style={{ color: "var(--amber)" }}>
                      ↻
                    </span>
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function Th({ k, sort, onSort, align = "left", children }) {
  const active = sort.key === k;
  const cls =
    align === "right" ? "text-right" : align === "center" ? "text-center" : "text-left";

  return (
    <th
      className={`${cls} px-2 py-1 font-normal select-none`}
      style={{ cursor: "pointer" }}
      onClick={() => onSort(k)}
    >
      <span style={{ color: active ? "var(--fg)" : "var(--fg-dim)" }}>
        {children}
        {active && (
          <span className="ml-1" style={{ color: "var(--green)" }}>
            {sort.dir === "asc" ? "▲" : "▼"}
          </span>
        )}
      </span>
    </th>
  );
}

function StatusDot({ state }) {
  const color =
    state === "online"
      ? "var(--green)"
      : state === "maintenance"
        ? "var(--amber)"
        : state === "new"
          ? "var(--fg-mute)"
          : "var(--red)";
  return (
    <span
      style={{
        display: "inline-block",
        width: 8,
        height: 8,
        borderRadius: "50%",
        background: color,
        boxShadow: state === "online" ? "0 0 6px " + color : "none",
      }}
    />
  );
}

function shortCpu(cpu) {
  if (!cpu) return "—";
  return cpu
    .replace(/\(R\)|\(TM\)|CPU|@.*$/gi, "")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 28);
}
