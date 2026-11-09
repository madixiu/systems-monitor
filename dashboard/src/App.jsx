import { useEffect, useMemo, useState } from "react";
import { API_URL, listSystems, getStats } from "./api";
import { isStale } from "./lib/format";
import KeyGate from "./components/KeyGate";
import Header from "./components/Header";
import FilterBar from "./components/FilterBar";
import HackTable from "./components/HackTable";
import SystemDrawer from "./components/SystemDrawer";
import AgentsPanel from "./components/AgentsPanel";

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
  const [threshold] = useState(90);
  const [pendingRefresh, setPendingRefresh] = useState({});
  const [flashSet, setFlashSet] = useState({});
  const [showAgents, setShowAgents] = useState(false);

  const [filter, setFilter] = useState({
    q: "",
    domain: "",
    os: "",
    state: "all",
  });

  const [sort, setSort] = useState({ key: "computer_name", dir: "asc" });

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem(LS_THEME, theme);
  }, [theme]);

  useEffect(() => {
    localStorage.setItem(LS_CAL, calendar);
  }, [calendar]);

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
            getStats(apiKey)
              .then(setStats)
              .catch(() => {});
          } else if (data.kind === "heartbeat") {
            const { agent_id, last_seen } = data;
            setSystems((cur) => {
              const idx = cur.findIndex((x) => x.agent_id === agent_id);
              if (idx === -1) return cur;
              const copy = cur.slice();
              copy[idx] = { ...copy[idx], last_seen };
              return copy;
            });
          }
        } catch (_) {}
      };
    }
    connect();

    return () => {
      clearTimeout(retry);
      if (es) es.close();
    };
  }, [apiKey]);

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
    return systems.filter((s) => {
      if (q) {
        const hay = [
          s.computer_name,
          s.fqdn,
          s.domain,
          s.console_user,
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
  }, [systems, filter, threshold]);

  const sorted = useMemo(() => {
    const out = filtered.slice();
    const { key, dir } = sort;
    const mult = dir === "asc" ? 1 : -1;

    function stateOf(s) {
      if (!s.reported_at) return 2; // new
      if (s.maintenance) return 1;
      return s.live_state === "online" ? 0 : 3;
    }

    out.sort((a, b) => {
      let av, bv;
      switch (key) {
        case "ip":
          av = a.ip_addresses?.[0] || "";
          bv = b.ip_addresses?.[0] || "";
          break;
        case "mac":
          av = a.mac_addresses?.[0] || "";
          bv = b.mac_addresses?.[0] || "";
          break;
        case "user":
          av = a.console_user || "";
          bv = b.console_user || "";
          break;
        case "os":
          av = a.os_name || "";
          bv = b.os_name || "";
          break;
        case "cpu":
          av = a.cpu_model || "";
          bv = b.cpu_model || "";
          break;
        case "ram":
          av = a.ram_bytes || 0;
          bv = b.ram_bytes || 0;
          break;
        case "disk_free":
          av = a.system_drive_total
            ? a.system_drive_free / a.system_drive_total
            : 0;
          bv = b.system_drive_total
            ? b.system_drive_free / b.system_drive_total
            : 0;
          break;
        case "last_seen":
          av = a.last_seen ? Date.parse(a.last_seen) : 0;
          bv = b.last_seen ? Date.parse(b.last_seen) : 0;
          break;
        case "computer_name":
        default:
          av = a.computer_name || "";
          bv = b.computer_name || "";
      }
      if (typeof av === "string") return mult * av.localeCompare(bv);
      return mult * (av - bv);
    });

    return out;
  }, [filtered, sort]);

  const liveStats = useMemo(() => {
    let online = 0,
      offline = 0,
      maintenance = 0;
    systems.forEach((s) => {
      if (s.maintenance) maintenance++;
      else if (!s.reported_at) {
        // new
      } else if (s.live_state === "online") online++;
      else offline++;
    });
    return { online, offline, maintenance, total: systems.length };
  }, [systems]);

  function logout() {
    setApiKey("");
    localStorage.removeItem(LS_KEY);
    setSystems([]);
    setStats({ online: 0, offline: 0, maintenance: 0 });
  }

  function markRefresh(agentId) {
    setPendingRefresh((p) => ({ ...p, [agentId]: true }));
  }

  async function afterMutation() {
    try {
      const sys = await listSystems(apiKey);
      setSystems(sys.items || []);
    } catch (e) {
      console.error(e);
    }
  }

  function handleSort(key) {
    setSort((s) =>
      s.key === key
        ? { key, dir: s.dir === "asc" ? "desc" : "asc" }
        : { key, dir: "asc" }
    );
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
          systems={sorted}
          selected={selected}
          onSelect={setSelected}
          calendar={calendar}
          pendingRefresh={pendingRefresh}
          flashSet={flashSet}
          sort={sort}
          onSort={handleSort}
        />
      </div>
      <div
        className="px-4 py-1 text-[10px] border-t flex justify-between"
        style={{ borderColor: "var(--border)", color: "var(--fg-mute)" }}
      >
        <span>
          {filtered.length} / {systems.length} systems · sort{" "}
          {sort.key} {sort.dir === "asc" ? "▲" : "▼"}
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
