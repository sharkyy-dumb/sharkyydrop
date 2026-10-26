const T = window.__TAURI__;
const invoke = T && T.core ? T.core.invoke : null;

const $ = (id) => document.getElementById(id);
const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

const THEME_KEY = "sharkyydrop-theme";

let info = null;
let tab = "received";
let refreshing = false;

function setTheme(theme) {
  document.documentElement.dataset.theme = theme;
  try { localStorage.setItem(THEME_KEY, theme); } catch (e) {}
}

function toggleTheme() {
  setTheme(document.documentElement.dataset.theme === "dark" ? "light" : "dark");
}

async function copyText(text) {
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(text);
      toast("Скопировано");
      return;
    }
  } catch (e) {}
  try {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.top = "-1000px";
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    ta.remove();
    toast("Скопировано");
  } catch (e) {
    toast("Не удалось скопировать");
  }
}

function toast(msg) {
  const t = $("toast");
  t.textContent = msg;
  t.classList.add("on");
  clearTimeout(toast._t);
  toast._t = setTimeout(() => t.classList.remove("on"), 2600);
}

function fmtSize(n) {
  if (!n || n < 0) return "0 Б";
  const u = ["Б", "КБ", "МБ", "ГБ"];
  let i = 0, v = n;
  while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
  return (i === 0 ? v : v.toFixed(v < 10 ? 1 : 0)) + " " + u[i];
}

function fmtTime(ms) {
  if (!ms) return "";
  const d = new Date(ms);
  return d.toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit" });
}

function ext(name) {
  const p = String(name).split(".");
  if (p.length < 2) return "file";
  return p.pop().slice(0, 3);
}

function linkify(s) {
  return esc(s).replace(/(https?:\/\/[^\s<]+)/g, '<a href="$1" target="_blank" rel="noopener">$1</a>');
}

function renderReceived() {
  const list = info.received || [];
  $("cReceived").textContent = list.length;
  if (!list.length) {
    $("receivedList").innerHTML = '<div class="empty"><b>Тут появятся файлы с телефона</b>Открой QR и скинь что-нибудь</div>';
    return;
  }
  $("receivedList").innerHTML = list.slice().reverse().map((f) => `
    <div class="item" data-open="${esc(f.path)}">
      <div class="avatar">${esc(ext(f.name))}</div>
      <div class="meta">
        <div class="name">${esc(f.name)}</div>
        <div class="sub">${fmtSize(f.size)} <span>·</span> ${fmtTime(f.at)}</div>
      </div>
      <div class="action"><svg viewBox="0 0 24 24" fill="none" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M14 4h6v6"/><path d="M20 4l-9 9"/><path d="M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5"/></svg></div>
    </div>`).join("");
}

function renderShared() {
  const list = info.shared || [];
  $("cShared").textContent = list.length;
  if (!list.length) {
    $("sharedList").innerHTML = '<div class="empty"><b>Раздавать пока нечего</b>Кинь файлы в окно выше — они появятся на телефоне</div>';
    return;
  }
  $("sharedList").innerHTML = list.slice().reverse().map((f) => `
    <div class="item" data-open="${esc(f.path)}">
      <div class="avatar">${esc(ext(f.name))}</div>
      <div class="meta">
        <div class="name">${esc(f.name)}</div>
        <div class="sub">${fmtSize(f.size)} <span class="tag">на телефоне</span></div>
      </div>
      <div class="action"><svg viewBox="0 0 24 24" fill="none" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M12 4v12m0 0-4.5-4.5M12 16l4.5-4.5"/><path d="M4 19h16"/></svg></div>
    </div>`).join("");
}

function renderTexts() {
  const list = info.texts || [];
  $("cTexts").textContent = list.length;
  if (!list.length) {
    $("messages").innerHTML = '<div class="empty"><b>Ничего нет</b>Кинь ссылку или текст — и он появится на телефоне</div>';
    return;
  }
  $("messages").innerHTML = list.map((t, i) =>
    `<div class="msg ${t.from === "pc" ? "mine" : ""}"><button class="copy-msg" data-copy="${i}" title="Скопировать текст" aria-label="Скопировать текст"><svg viewBox="0 0 24 24" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="11" height="11" rx="2"/><path d="M15 9V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h4"/></svg></button><span class="who">${t.from === "pc" ? "с компьютера" : "с телефона"} · ${fmtTime(t.at)}</span><span class="body">${linkify(t.text)}</span></div>`
  ).join("");
  $("messages").scrollTop = $("messages").scrollHeight;
}

function render() {
  if (!info) return;
  $("qrBox").innerHTML = info.qr || '<div class="qr-load">QR не создался</div>';
  $("url").textContent = info.url;
  $("url").title = info.url;
  $("statusDot").classList.remove("off");
  $("statusText").textContent = "работает · " + info.ip + ":" + info.port;
  $("netNote").hidden = info.ip !== "127.0.0.1";
  renderReceived();
  renderShared();
  renderTexts();
  updateTabs();
}

function updateTabs() {
  document.querySelectorAll(".tab").forEach((b) => b.classList.toggle("active", b.dataset.tab === tab));
  $("receivedList").hidden = tab !== "received";
  $("sharedList").hidden = tab !== "shared";
  $("textsPane").hidden = tab !== "texts";
  $("clearBtn").hidden = false;
}

async function refresh() {
  if (!invoke || refreshing) return;
  refreshing = true;
  try {
    info = await invoke("get_info");
    render();
  } catch (e) {
    $("statusDot").classList.add("off");
    $("statusText").textContent = "ошибка";
  } finally {
    refreshing = false;
  }
}

async function pickFiles() {
  $("fileInput").click();
}

async function readAndShare(files) {
  if (!files || !files.length) return;
  const items = [];
  for (const f of files) {
    const buf = await f.arrayBuffer();
    items.push({ name: f.name, bytes: Array.from(new Uint8Array(buf)) });
  }
  try {
    await invoke("share_blobs", { items });
    toast(files.length > 1 ? `Добавил ${files.length} файлов` : "Файл готов на телефоне");
    refresh();
  } catch (e) {
    toast("Не получилось: " + e);
  }
}

async function sendText() {
  const el = $("textInput");
  const value = el.value.trim();
  if (!value) return;
  $("sendBtn").disabled = true;
  try {
    await invoke("send_text", { text: value });
    el.value = "";
    refresh();
  } catch (e) {
    toast("Не получилось отправить");
  } finally {
    $("sendBtn").disabled = false;
  }
}

function bindTauri() {
  if (!invoke) return;
  if (T.event && T.event.listen) {
    T.event.listen("changed", () => refresh());
    T.event.listen("received", (e) => {
      const name = e && e.payload ? e.payload.name : "";
      toast(name ? `Получено: ${name}` : "Пришёл файл");
      refresh();
    });
  }
  if (T.webview && T.webview.getCurrentWebview) {
    T.webview.getCurrentWebview().onDragDropEvent((event) => {
      const p = event.payload || {};
      if (p.type === "over" || p.type === "enter") {
        $("dropzone").classList.add("over");
      } else if (p.type === "drop") {
        $("dropzone").classList.remove("over");
        const paths = p.paths || [];
        if (paths.length) {
          invoke("add_files", { paths })
            .then(() => { toast(paths.length > 1 ? `Добавил ${paths.length} файлов` : "Файл готов на телефоне"); refresh(); })
            .catch((e) => toast("Не получилось: " + e));
        }
      } else {
        $("dropzone").classList.remove("over");
      }
    });
  }
}

document.addEventListener("click", (e) => {
  const copy = e.target.closest("[data-copy]");
  if (copy && info && info.texts) {
    const item = info.texts[Number(copy.getAttribute("data-copy"))];
    if (item) {
      copyText(item.text);
      return;
    }
  }
  const item = e.target.closest("[data-open]");
  if (item && invoke) {
    const path = item.getAttribute("data-open");
    invoke("open_path", { path }).catch((err) => toast("Не открылось: " + err));
  }
});

document.querySelectorAll(".tab").forEach((b) => {
  b.addEventListener("click", () => { tab = b.dataset.tab; updateTabs(); });
});

$("pickBtn").addEventListener("click", pickFiles);
$("fileInput").addEventListener("change", (e) => { readAndShare(e.target.files); e.target.value = ""; });
$("copyBtn").addEventListener("click", () => {
  if (info) copyText(info.url);
});
$("themeBtn").addEventListener("click", toggleTheme);
$("openDir").addEventListener("click", () => invoke && invoke("open_downloads").catch(() => {}));
$("sendBtn").addEventListener("click", sendText);
$("textInput").addEventListener("keydown", (e) => {
  if ((e.metaKey || e.ctrlKey) && e.key === "Enter") sendText();
});
$("clearBtn").addEventListener("click", async () => {
  if (!invoke) return;
  const cmd = tab === "received" ? "clear_received" : tab === "shared" ? "clear_shared" : "clear_texts";
  await invoke(cmd);
  refresh();
});

const dz = $("dropzone");
dz.addEventListener("dragover", (e) => { e.preventDefault(); dz.classList.add("over"); });
dz.addEventListener("dragleave", () => dz.classList.remove("over"));
dz.addEventListener("drop", (e) => { e.preventDefault(); dz.classList.remove("over"); });

if (invoke) {
  bindTauri();
  refresh();
  setInterval(() => { if (!document.hidden) refresh(); }, 3000);
} else {
  $("statusText").textContent = "нет связи с приложением";
  $("statusDot").classList.add("off");
}
