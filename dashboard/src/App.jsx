import { useEffect, useMemo, useRef, useState } from "react";
import { API_URL, listSystems, getStats } from "./api";
import { isStale } from "./lib/format";
import KeyGate from "./components/KeyGate";
import Header from "./components/Header";
import FilterBar from "./components/FilterBar";
import HackTable from "./components/HackTable";
import SystemDrawer from "./components/SystemDrawer";
import AgentsPanel from './components/AgentsPanel'

const LS_KEY = "pak-monitor-api-key";
const LS_THEME = "pak-monitor-theme";
const LS_CAL = "pak-monitor-calendar";

export default function App() {
  const [apiKey, setApiKey] = useState(
    () => localStorage.getItem(LS_KEY) || "",
  );
  const [theme, setTheme] = useState(
    () => localStorage.getItem(LS_THEME) || "dark",
  );
  const [calendar, setCalendar] = useState(
    () => localStorage.getItem(LS_CAL) || "gregorian",
  );

  const [systems, setSystems] = useState([]);
  const [stats, setStats] = useState({ online: 0, offline: 0, maintenance: 0 });
  const [connected, setConnected] = useState(false);
  const [selected, setSelected] = useState(null);
  const [threshold, setThreshold] = useState(90);
  const [pendingRefresh, setPendingRefresh] = useState({});
  const [flashSet, setFlashSet] = useState({});
  const [showAgents, setShowAgents] = useState(false);
  const [filter, setFilter] = useState({
    q: "",
    domain: "",
    os: "",
    state: "all",
    sort: "name",
  });

  // apply theme
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem(LS_THEME, theme);
  }, [theme]);

  useEffect(() => {
    localStorage.setItem(LS_CAL, calendar);
  }, [calendar]);

  // initial data load
  useEffect(() => {
    if (!apiKey) return;
    localStorage.setItem(LS_KEY, apiKey);
    refreshAll();
  }, [apiKey]);

  async function refreshAll() {
    try {
      const [sys, st] = await Promise.all([
        listSystems(apiKey),
        getStats(apiKey),
      ]);
      setSystems(sys.items || []);
      setStats(st);
    } catch (e) {
      console.error(e);
    }
  }

  // SSE
  useEffect(() => {
    if (!apiKey) return;
    let es;
    let retry;

    function connect() {
      es = new EventSource(
        `${API_URL}/api/v1/stream?key=${encodeURIComponent(apiKey)}`,
      );
      es.onopen = () => setConnected(true);
      es.onerror = () => {
        setConnected(false);
        es.close();
        retry = setTimeout(connect, 3000);
      };
      es.onmessage = (ev) => {
        try {
          const data = JSON.parse(ev.data);
          if (data.kind === "ingest") {
            const s = data.system;
            setSystems((cur) => {
              const idx = cur.findIndex((x) => x.agent_id === s.agent_id);
              if (idx === -1) return [...cur, s];
              const copy = cur.slice();
              copy[idx] = s;
              return copy;
            });
            setPendingRefresh((p) => {
              if (!p[s.agent_id]) return p;
              const c = { ...p };
              delete c[s.agent_id];
              return c;
            });
            setFlashSet((f) => ({ ...f, [s.agent_id]: Date.now() }));
            // refresh stats occasionally
            getStats(apiKey)
              .then(setStats)
              .catch(() => {});
          }
        } catch (_) {
          /* ignore */
        }
      };
    }
    connect();

    return () => {
      clearTimeout(retry);
      if (es) es.close();
    };
  }, [apiKey]);

  // clear flash set after animation
  useEffect(() => {
    const t = setInterval(() => {
      const now = Date.now();
      setFlashSet((cur) => {
        const next = {};
        let changed = false;
        for (const k of Object.keys(cur)) {
          if (now - cur[k] < 1500) next[k] = cur[k];
          else changed = true;
        }
        return changed ? next : cur;
      });
    }, 500);
    return () => clearInterval(t);
  }, []);

  const domains = useMemo(() => {
    const s = new Set();
    systems.forEach((x) => {
      if (x.domain) s.add(x.domain);
    });
    return Array.from(s).sort();
  }, [systems]);

  const oses = useMemo(() => {
    const s = new Set();
    systems.forEach((x) => {
      if (x.os_name) s.add(x.os_name);
    });
    return Array.from(s).sort();
  }, [systems]);

  const filtered = useMemo(() => {
    const q = filter.q.trim().toLowerCase();
    let out = systems.filter((s) => {
      if (q) {
        const hay = [
          s.computer_name,
          s.fqdn,
          s.domain,
          ...(s.ip_addresses || []),
          ...(s.mac_addresses || []),
        ]
          .join(" ")
          .toLowerCase();
        if (!hay.includes(q)) return false;
      }
      if (filter.domain && s.domain !== filter.domain) return false;
      if (filter.os && s.os_name !== filter.os) return false;
      if (filter.state !== "all") {
        if (filter.state === "maintenance" && !s.maintenance) return false;
        if (filter.state === "online") {
          if (s.maintenance) return false;
          if (isStale(s.last_seen, threshold)) return false;
        }
        if (filter.state === "offline") {
          if (s.maintenance) return false;
          if (!isStale(s.last_seen, threshold)) return false;
        }
      }
      return true;
    });
    const by = filter.sort;
    out.sort((a, b) => {
      if (by === "last_seen")
        return new Date(b.last_seen || 0) - new Date(a.last_seen || 0);
      if (by === "disk_free")
        return (b.system_drive_free || 0) - (a.system_drive_free || 0);
      if (by === "ram") return (b.ram_bytes || 0) - (a.ram_bytes || 0);
      return (a.computer_name || "").localeCompare(b.computer_name || "");
    });
    return out;
  }, [systems, filter, threshold]);

  // recompute online/offline count from local data (SSE keeps it fresh)
  const liveStats = useMemo(() => {
    let online = 0,
      offline = 0,
      maintenance = 0;
    systems.forEach((s) => {
      if (s.maintenance) maintenance++;
      else if (isStale(s.last_seen, threshold)) offline++;
      else online++;
    });
    return { online, offline, maintenance, total: systems.length };
  }, [systems, threshold]);

  function logout() {
    setApiKey("");
    localStorage.removeItem(LS_KEY);
    setSystems([]);
    setStats({ online: 0, offline: 0, maintenance: 0 });
  }

  function markRefresh(agentId) {
    setPendingRefresh((p) => ({ ...p, [agentId]: true }));
  }

  async function afterMutation(agentId) {
    try {
      const sys = await listSystems(apiKey);
      setSystems(sys.items || []);
    } catch (e) {
      console.error(e);
    }
  }

  if (!apiKey) return <KeyGate onSubmit={setApiKey} />;

  return (
    <div className="h-full flex flex-col">
      <Header
        stats={liveStats}
        connected={connected}
        calendar={calendar}
        setCalendar={setCalendar}
        theme={theme}
        setTheme={setTheme}
        onLogout={logout}
        onShowAgents={() => setShowAgents(true)}
      />
      <FilterBar
        filter={filter}
        setFilter={setFilter}
        domains={domains}
        oses={oses}
      />
      <div className="flex-1 overflow-hidden">
        <HackTable
          systems={filtered}
          selected={selected}
          onSelect={setSelected}
          calendar={calendar}
          pendingRefresh={pendingRefresh}
          flashSet={flashSet}
          threshold={threshold}
        />
      </div>
      <div
        className="px-4 py-1 text-[10px] border-t flex justify-between"
        style={{ borderColor: "var(--border)", color: "var(--fg-mute)" }}
      >
        <span>
          {filtered.length} / {systems.length} systems · threshold {threshold}s
        </span>
        <span>PAK://MONITOR v0.1</span>
      </div>
      <SystemDrawer
        agentId={selected}
        apiKey={apiKey}
        onClose={() => setSelected(null)}
        calendar={calendar}
        onMutated={afterMutation}
        onForceRefresh={markRefresh}
      />
      {showAgents && (
        <AgentsPanel
          apiKey={apiKey}
          calendar={calendar}
          onClose={() => setShowAgents(false)}
        />
      )}
    </div>
  );
}
