import { useEffect, useState } from "react";

export default function Header({
  stats,
  connected,
  calendar,
  setCalendar,
  theme,
  setTheme,
  onLogout,
  filter,
  setFilter,
  onShowAgents,
}) {
  const [clock, setClock] = useState(new Date());
  useEffect(() => {
    const t = setInterval(() => setClock(new Date()), 1000);
    return () => clearInterval(t);
  }, []);

  const pad = (n) => String(n).padStart(2, "0");
  const timeStr = `${pad(clock.getHours())}:${pad(clock.getMinutes())}:${pad(clock.getSeconds())}`;

  const total = stats.online + stats.offline;
  const online = stats.online;

  return (
    <div
      className="flex items-center gap-4 px-4 py-2 border-b"
      style={{ borderColor: "var(--border)", background: "var(--bg-panel)" }}
    >
      <div className="font-bold glow text-base whitespace-nowrap">
        PAK://MONITOR
      </div>

      <div className="text-xs" style={{ color: "var(--fg-dim)" }}>
        {timeStr}
      </div>

      <div className="h-4 w-px" style={{ background: "var(--border)" }} />

      <div className="flex items-center gap-3 text-xs">
        <span style={{ color: "var(--fg-dim)" }}>ONLINE</span>
        <span className="glow" style={{ color: "var(--green)" }}>
          {online}/{total}
        </span>
        {stats.maintenance > 0 && (
          <>
            <span style={{ color: "var(--fg-mute)" }}>·</span>
            <span style={{ color: "var(--amber)" }}>
              {stats.maintenance} maint
            </span>
          </>
        )}
      </div>

      <div className="flex items-center gap-1 text-xs ml-2">
        <span
          className={connected ? "" : "blink"}
          style={{
            display: "inline-block",
            width: 8,
            height: 8,
            borderRadius: "50%",
            background: connected ? "var(--green)" : "var(--red)",
          }}
        />
        <span style={{ color: connected ? "var(--green)" : "var(--red)" }}>
          {connected ? "streaming" : "reconnecting"}
        </span>
      </div>

      <div className="flex-1" />

      <div className="flex items-center gap-2">
        <button
          onClick={onShowAgents}
          className="text-xs px-2 py-1 border hover:opacity-80"
          style={{
            borderColor: "var(--border-bright)",
            color: "var(--fg-dim)",
          }}
          title="Agent registry"
        >
          AGENTS
        </button>
        <button
          onClick={() =>
            setCalendar(calendar === "gregorian" ? "jalali" : "gregorian")
          }
          className="text-xs px-2 py-1 border hover:opacity-80"
          style={{
            borderColor: "var(--border-bright)",
            color: "var(--fg-dim)",
          }}
          title="Toggle calendar"
        >
          {calendar === "gregorian" ? "GREG" : "JALALI"}
        </button>
        <button
          onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
          className="text-xs px-2 py-1 border hover:opacity-80"
          style={{
            borderColor: "var(--border-bright)",
            color: "var(--fg-dim)",
          }}
          title="Toggle theme"
        >
          {theme === "dark" ? "☀" : "☾"}
        </button>
        <button
          onClick={onLogout}
          className="text-xs px-2 py-1 border hover:opacity-80"
          style={{
            borderColor: "var(--border-bright)",
            color: "var(--fg-mute)",
          }}
          title="Logout"
        >
          ⏻
        </button>
      </div>
    </div>
  );
}
