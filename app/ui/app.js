'use strict';

// MobaLedLib Studio frontend. Plain JS on purpose: no build step, no npm.
// All file handling, validation and code generation happens in Rust (see src-tauri/src/main.rs).

const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;
const dialog = window.__TAURI__.dialog;

/** Languages of the effect texts in the catalog (machine translated by the Excel tool, except de and en). */
const LANGS = [['en', 'English'], ['de', 'Deutsch'], ['nl', 'Nederlands'], ['fr', 'Français'], ['it', 'Italiano'], ['es', 'Español'], ['da', 'Dansk']];

// ---------------------------------------------------------------- state

const state = {
  catalog: null,
  layout: null,
  path: null,          // where Save writes; null = ask
  dirty: false,
  sheet: 0,
  sel: -1,             // selected row index
  group: null,         // filter by "Filter" column value
  query: '',
  analysis: null,
  modal: null,         // { type: 'picker' | 'upload' | 'header' | 'settings' | 'sheet', ... }
  lang: storedLang(),
};

function storedLang() {
  let l = null;
  try { l = localStorage.getItem('mll.lang'); } catch { /* storage unavailable */ }
  return LANGS.some(([v]) => v === l) ? l : 'en';
}

const FQBN_PRESETS = [
  ['arduino:avr:nano', 'Arduino Nano'],
  ['arduino:avr:nano:cpu=atmega328old', 'Arduino Nano (old bootloader, most clones)'],
  ['arduino:avr:uno', 'Arduino Uno'],
  ['esp32:esp32:esp32', 'ESP32 Dev Module'],
  ['rp2040:rp2040:rpipico', 'Raspberry Pi Pico'],
];

const PROTOCOLS = [
  ['DCC', 'DCC', 'Digital command station (Roco, ESU, Lenz…). A second Arduino reads the track signal.'],
  ['Selectrix', 'Selectrix', 'Trix / Rautenhaus SX bus.'],
  ['CAN', 'Märklin CAN', 'CS2 / CS3 via CAN bus. The central must be connected.'],
  ['LNet', 'LocoNet', 'Digitrax / Uhlenbrock LocoNet.'],
];

const PLATFORMS = [
  ['AM328', 'Arduino Nano', 'Classic main board · up to 256 RGB LEDs'],
  ['ESP32', 'ESP32', 'More memory, parallel outputs · up to 16,384 LEDs'],
  ['PICO', 'Raspberry Pi Pico', 'RP2040 · parallel outputs'],
];

const INPUT_TYPES = [['on_off', 'On / Off'], ['red', 'Red'], ['green', 'Green'], ['push', 'Push button']];

const ROOM_TYPES = ('ROOM_DARK ROOM_BRIGHT ROOM_WARM_W ROOM_RED ROOM_D_RED ROOM_COL0 ROOM_COL1 ROOM_COL2 ROOM_COL3 ' +
  'ROOM_COL4 ROOM_COL5 ROOM_COL345 ROOM_TV0 ROOM_TV1 ROOM_CHIMNEY ROOM_CHIMNEYD ROOM_CHIMNEYB ROOM_TV0_CHIMNEY ' +
  'ROOM_TV0_CHIMNEYD ROOM_TV0_CHIMNEYB ROOM_TV1_CHIMNEY ROOM_TV1_CHIMNEYD ROOM_TV1_CHIMNEYB NEON_LIGHT NEON_LIGHT1 ' +
  'NEON_LIGHT2 NEON_LIGHT3 NEON_LIGHTD NEON_LIGHT1D NEON_LIGHT2D NEON_LIGHT3D NEON_LIGHTM NEON_LIGHT1M NEON_LIGHT2M ' +
  'NEON_LIGHT3M NEON_LIGHTL NEON_LIGHT1L NEON_LIGHT2L NEON_LIGHT3L NEON_DEF_D NEON_DEF1D NEON_DEF2D NEON_DEF3D ' +
  'GAS_LIGHT GAS_LIGHT1 GAS_LIGHT2 GAS_LIGHT3 GAS_LIGHTD GAS_LIGHT1D GAS_LIGHT2D GAS_LIGHT3D FIRE FIRED FIREB ' +
  'CANDLE CANDLE1 CANDLE2 CANDLE3 SINGLE_LED1 SINGLE_LED2 SINGLE_LED3 SINGLE_LED1D SINGLE_LED2D SINGLE_LED3D SKIP_ROOM').split(' ');

const CX_VALUES = ['C1', 'C2', 'C3', 'C12', 'C23', 'C_ALL'];

// ---------------------------------------------------------------- helpers

const $ = (sel) => document.querySelector(sel);
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const sheet = () => state.layout?.sheets[state.sheet];
const rows = () => sheet()?.rows ?? [];

const ICON = {
  logo: '<svg width="22" height="22" viewBox="0 0 24 24" fill="none" aria-hidden="true"><circle cx="5" cy="12" r="3" fill="#F5B83D"/><circle cx="12" cy="12" r="3" fill="#5BD48A"/><circle cx="19" cy="12" r="3" fill="#6CB6FF"/></svg>',
  upload: '<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 16V4M6 10l6-6 6 6M4 20h16"/></svg>',
  plus: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>',
  search: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#9BA4AF" stroke-width="2" stroke-linecap="round" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/></svg>',
  gear: '<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></svg>',
  refresh: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M21 12a9 9 0 1 1-3-6.7L21 8M21 3v5h-5"/></svg>',
  servo: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#8CC6FF" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="9" width="14" height="10" rx="2"/><circle cx="13" cy="9" r="2.5"/><path d="M13 9l7-5"/></svg>',
  sound: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#C9A6FF" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 9v6h4l5 4V5L8 9z"/><path d="M16 9a4 4 0 0 1 0 6M19 6a8 8 0 0 1 0 12"/></svg>',
  logic: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#7DE0A4" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 7h6a5 5 0 0 1 0 10H4zM15 12h5"/></svg>',
};

function toast(msg, bad = false) {
  const t = $('#toast');
  t.textContent = msg;
  t.className = 'toast show' + (bad ? ' bad' : '');
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => (t.className = 'toast' + (bad ? ' bad' : '')), bad ? 6000 : 2500);
}

function errText(e) {
  if (Array.isArray(e)) return e.map((d) => (d.row != null ? `Row ${d.row + 1}: ` : '') + d.message).join('\n');
  return String(e?.message ?? e);
}

// ---------------------------------------------------------------- catalog

const cat = { byName: new Map(), byFind: new Map(), params: new Map() };

function indexCatalog(c) {
  state.catalog = c;
  for (const m of c.macros) {
    if (!cat.byName.has(m.name)) cat.byName.set(m.name, m);
    if (m.find && !cat.byFind.has(m.find)) cat.byFind.set(m.find, m);
  }
  for (const p of c.params) cat.params.set(p.name, p);
}

function macroOf(text) {
  const name = (text || '').split('\n')[0].split('(')[0].trim();
  if (!name) return null;
  return cat.byFind.get(name + '(') || cat.byName.get(name) || null;
}

/** Texts in the chosen language. Each field falls back to English, then German, when it is empty. */
function localized(text) {
  const out = {};
  for (const l of [state.lang, 'en', 'de']) {
    for (const [k, v] of Object.entries(text?.[l] || {})) if (v && !out[k]) out[k] = v;
  }
  return out;
}

function mtext(m) {
  return localized(m.text);
}

function macroTitle(m) {
  return mtext(m).title || m.name;
}

function macroGroups(m) {
  return (mtext(m).group || '').split('|').map((s) => s.trim()).filter(Boolean);
}

function paramText(p) {
  return localized(p?.text);
}

/** Language independent category key of an effect (its first English group), so switching language keeps the filter. */
function groupKey(m) {
  return (m.text.en?.group || m.text.de?.group || '').split('|')[0].trim() || 'Other';
}

function langSelect() {
  return `<select class="select" id="lang" aria-label="Language of the effect texts" style="width:auto">
    ${LANGS.map(([v, n]) => `<option value="${v}" ${v === state.lang ? 'selected' : ''}>${n}</option>`).join('')}</select>`;
}

/** What a row drives: led | servo | sound | logic */
function outputKind(row, m) {
  const g = m ? ((m.text.de?.group || '') + ' ' + (m.text.en?.group || '') + ' ' + m.name).toLowerCase() : row.macro_text.toLowerCase();
  if (g.includes('servo')) return 'servo';
  if (g.includes('sound') || g.includes('mp3') || g.includes('jq6500')) return 'sound';
  if (!row.leds.trim()) return 'logic';
  return 'led';
}

function category(m) {
  const g = (m.text.en?.group || m.text.de?.group || '').split('|')[0].trim().toLowerCase();
  if (g.startsWith('light') || g.startsWith('licht')) return ['light', 'var(--light)'];
  if (g.includes('signal') || g.includes('traffic')) return ['signal', 'var(--signal)'];
  if (g.includes('servo') || g.includes('dynam') || g.includes('motion')) return ['motion', 'var(--motion)'];
  if (g.includes('sound')) return ['sound', 'var(--sound)'];
  return ['other', 'var(--logic)'];
}

// ---------------------------------------------------------------- persistence helpers

function recentList() {
  try { return JSON.parse(localStorage.getItem('mll.recent') || '[]'); } catch { return []; }
}

function addRecent(path, name) {
  if (!path) return;
  const list = recentList().filter((r) => r.path !== path);
  list.unshift({ path, name, at: Date.now() });
  try { localStorage.setItem('mll.recent', JSON.stringify(list.slice(0, 8))); } catch { /* storage unavailable */ }
}

function setDirty(d = true) {
  state.dirty = d;
  document.title = state.layout ? `${state.layout.name}${d ? ' •' : ''} — MobaLedLib Studio` : 'MobaLedLib Studio';
  const chip = $('#dirty-chip');
  if (chip) chip.hidden = !d;
  invoke('set_dirty', { dirty: d }).catch(() => {});
}

function changed({ table = true, inspector = false } = {}) {
  setDirty(true);
  scheduleAnalyze();
  if (table) renderMain();
  if (inspector) renderInspector();
}

let analyzeTimer = null;
function scheduleAnalyze() {
  clearTimeout(analyzeTimer);
  analyzeTimer = setTimeout(analyze, 120);
}

async function analyze() {
  if (!state.layout) return;
  try {
    state.analysis = await invoke('analyze', { layout: state.layout, sheet: state.sheet });
  } catch (e) {
    state.analysis = null;
    toast(errText(e), true);
  }
  renderMain();
  renderStatus();
  renderDiag();
}

function loadLayout(layout, path, imported = false) {
  if (state.modal) closeModal();
  state.layout = layout;
  state.path = path;
  state.sheet = 0;
  state.sel = layout.sheets[0]?.rows.length ? 0 : -1;
  state.group = null;
  state.query = '';
  state.analysis = null;
  setDirty(imported);
  if (path) addRecent(path, layout.name);
  render();
  analyze();
}

// ---------------------------------------------------------------- file actions

/** Native "Save / Don't Save / Cancel" question. Returns false if the user cancelled. */
async function guardUnsaved() {
  if (!state.dirty || !state.layout) return true;
  const answer = await dialog.message('Your changes will be lost if you don’t save them.', {
    title: `Save changes to “${state.layout.name}”?`,
    kind: 'warning',
    buttons: { yes: 'Save', no: 'Don’t Save', cancel: 'Cancel' },
  });
  if (answer === 'Save') return actSave(false);
  return answer === 'Don’t Save';
}

async function actNew() {
  if (!(await guardUnsaved())) return;
  if (state.modal) closeModal();
  state.layout = null;
  state.path = null;
  setDirty(false);
  render();
}

async function actOpen(importPgf = false) {
  if (!(await guardUnsaved())) return;
  try {
    const r = await invoke('open_layout', { import: importPgf });
    if (!r) return;
    loadLayout(r.layout, r.path, r.imported);
    if (r.imported) toast('Imported. Save it as a layout file to keep your changes.');
  } catch (e) {
    toast(errText(e), true);
  }
}

async function actOpenPath(path) {
  if (!(await guardUnsaved())) return;
  try {
    const r = await invoke('open_layout_path', { path });
    loadLayout(r.layout, r.path, r.imported);
  } catch (e) {
    toast(errText(e), true);
  }
}

async function actSave(saveAs = false) {
  if (!state.layout) return false;
  try {
    const p = await invoke('save_layout', { layout: state.layout, path: saveAs ? null : state.path });
    if (!p) return false;
    state.path = p;
    addRecent(p, state.layout.name);
    setDirty(false);
    renderTopbar();
    toast('Saved');
    return true;
  } catch (e) {
    toast(errText(e), true);
    return false;
  }
}

async function actExport() {
  if (!state.layout) return;
  try {
    const p = await invoke('export_pgf', { layout: state.layout });
    if (p) toast('Exported ' + p.split(/[\\/]/).pop());
  } catch (e) {
    toast(errText(e), true);
  }
}

async function createLayout(name, protocol, platform) {
  const layout = await invoke('new_layout', { name: name || 'New layout', protocol, platform });
  loadLayout(layout, null, true);
}

// ---------------------------------------------------------------- row actions

function newRow(base = {}) {
  return {
    enabled: true, filter: '', address: '', bit_pos: '', input_type: null, start_value: '', description: '',
    distributor: '', plug: '', macro_text: '', leds: '', in_cnt: 0, loc_in_ch: 0, led_channel: 0, ...base,
  };
}

function moveRow(i, d) {
  const r = rows();
  const j = i + d;
  if (j < 0 || j >= r.length) return;
  [r[i], r[j]] = [r[j], r[i]];
  state.sel = j;
  changed({ inspector: true });
}

function deleteRow(i) {
  rows().splice(i, 1);
  state.sel = Math.min(i, rows().length - 1);
  changed({ inspector: true });
}

function duplicateRow(i) {
  const copy = JSON.parse(JSON.stringify(rows()[i]));
  rows().splice(i + 1, 0, copy);
  state.sel = i + 1;
  changed({ inspector: true });
}

// ---------------------------------------------------------------- rendering: shell

function render() {
  const app = $('#app');
  if (!state.layout) {
    app.innerHTML = welcomeHtml();
  } else {
    app.innerHTML = `
      <header class="topbar" id="topbar"></header>
      <div class="body">
        <nav class="sidebar" id="sidebar" aria-label="Sheets"></nav>
        <main class="main" id="main"></main>
        <aside class="inspector" id="inspector" aria-label="Inspector"></aside>
      </div>
      <footer class="statusbar" id="statusbar"></footer>`;
    renderTopbar();
    renderSidebar();
    renderMain();
    renderInspector();
    renderStatus();
  }
  renderModal();
  setDirty(state.dirty);
}

function renderTopbar() {
  const el = $('#topbar');
  if (!el) return;
  const l = state.layout;
  el.innerHTML = `
    <div class="left">
      ${ICON.logo}
      <span class="brand">${esc(l.name)}</span>
      <span class="path" title="${esc(state.path || '')}">${esc(state.path || 'not saved yet')}</span>
      <span class="chip warn" id="dirty-chip" ${state.dirty ? '' : 'hidden'}><span class="dot" style="background:var(--accent)"></span>Unsaved changes</span>
    </div>
    <div class="right">
      <button class="btn primary" data-act="upload">${ICON.upload} Upload</button>
    </div>`;
}

function renderSidebar() {
  const el = $('#sidebar');
  if (!el) return;
  const groups = new Map();
  for (const r of rows()) {
    const g = r.filter.trim();
    if (g) groups.set(g, (groups.get(g) || 0) + 1);
  }
  el.innerHTML = `
    <div class="navgroup">
      <div class="label">Sheets</div>
      ${state.layout.sheets.map((s, i) => `
        <button class="navitem" data-act="sheet" data-i="${i}" aria-current="${i === state.sheet}">
          <span class="badge">${esc(s.protocol === 'Selectrix' ? 'SX' : s.protocol)}</span>
          <span class="grow">${esc(s.name)}</span><span class="count">${s.rows.length}</span>
        </button>`).join('')}
      <button class="navitem add" data-act="add-sheet">${ICON.plus} New sheet</button>
    </div>
    <div class="navgroup">
      <div class="label">Groups</div>
      <button class="navitem" data-act="group" data-g="" aria-current="${state.group === null}">
        <span class="dot" style="background:var(--muted)"></span><span class="grow">All effects</span><span class="count">${rows().length}</span>
      </button>
      ${[...groups].map(([g, n]) => `
        <button class="navitem" data-act="group" data-g="${esc(g)}" aria-current="${state.group === g}">
          <span class="dot" style="background:var(--light);border-radius:2px"></span><span class="grow">${esc(g)}</span><span class="count">${n}</span>
        </button>`).join('')}
      ${groups.size === 0 ? '<div class="hint" style="padding:0 8px">Fill in “Group” in the inspector to group effects.</div>' : ''}
    </div>`;
}

// ---------------------------------------------------------------- rendering: main table

function rowVisible(r) {
  if (state.group !== null && r.filter.trim() !== state.group) return false;
  const q = state.query.trim().toLowerCase();
  if (!q) return true;
  const m = macroOf(r.macro_text);
  return [r.description, r.address, r.filter, r.macro_text, m ? macroTitle(m) : ''].some((s) => (s || '').toLowerCase().includes(q));
}

function outputCell(r, m, i) {
  const kind = outputKind(r, m);
  const start = state.analysis?.led_numbers?.[i];
  const pos = start == null ? '' : ` · #${start}`;
  if (kind === 'servo') return `${ICON.servo}<span>Servo${pos}</span>`;
  if (kind === 'sound') return `${ICON.sound}<span>Sound module${pos}</span>`;
  if (kind === 'logic') return `${ICON.logic}<span class="muted">no output</span>`;
  const leds = r.leds.trim();
  if (/C/.test(leds)) {
    const ch = leds.replace('^', '').trim().replace(/^C/, 'ch ').replace('-', '–');
    return `<span class="leddots"><span></span></span><span>Single LED ${esc(ch)}${pos}</span>`;
  }
  const n = parseInt(leds, 10) || 0;
  const dots = '<span></span>'.repeat(Math.min(Math.max(n, 1), 5));
  return `<span class="leddots">${dots}</span><span>${n} LED${n === 1 ? '' : 's'}${pos}</span>`;
}

function triggerCell(r) {
  if (!r.address.trim()) return `<div class="a">—</div><div class="b">${r.in_cnt > 0 ? 'always on' : ''}</div>`;
  const prefix = sheet().protocol === 'Selectrix' ? 'SX ' : sheet().protocol + ' ';
  const addr = sheet().protocol === 'Selectrix' && r.bit_pos ? `${r.address}.${r.bit_pos}` : r.address;
  const t = INPUT_TYPES.find(([k]) => k === r.input_type)?.[1] ?? '';
  return `<div class="a mono">${esc(prefix + addr)}</div><div class="b">${esc(t)}</div>`;
}

function renderMain() {
  const el = $('#main');
  if (!el) return;
  const s = sheet();
  const errorsByRow = new Map();
  for (const d of state.analysis?.errors ?? []) if (d.row != null) errorsByRow.set(d.row, d.message);

  const list = rows().map((r, i) => [r, i]).filter(([r]) => rowVisible(r));
  const body = list.map(([r, i]) => {
    const m = macroOf(r.macro_text);
    const err = errorsByRow.get(i);
    return `
      <div class="trow ${r.enabled ? '' : 'off'}" role="row" data-act="row" data-i="${i}" aria-selected="${i === state.sel}">
        <button class="switch" role="switch" aria-checked="${r.enabled}" aria-label="Enable" data-act="toggle" data-i="${i}"><span></span></button>
        <div class="cell-2"><div class="a">${esc(r.description || (m ? macroTitle(m) : 'Untitled'))}</div><div class="b">${esc(r.filter)}</div></div>
        <div class="cell-2">${triggerCell(r)}</div>
        <div class="cell-2"><div class="a">${esc(m ? macroTitle(m) : (r.macro_text ? r.macro_text.split('(')[0] : '—'))}</div><div class="b mono">${esc(r.macro_text.split('\n')[0])}</div></div>
        <div class="output">${outputCell(r, m, i)}</div>
        <div class="mono muted" style="font-size:12px">${esc(r.distributor || r.plug ? `${r.distributor || '–'} · ${r.plug || '–'}` : '')}</div>
        <div>${err ? `<span class="rowerr" title="${esc(err)}">!</span>` : ''}</div>
      </div>`;
  }).join('');

  const a = state.analysis;
  const ledCount = a ? a.total_leds : '…';
  el.innerHTML = `
    <div class="main-head">
      <div class="titles">
        <input class="sheet-title" id="sheet-name" value="${esc(s.name)}" aria-label="Sheet name">
        <div class="muted" style="margin-top:4px;display:flex;gap:10px;align-items:center;flex-wrap:wrap">
          <select class="select" id="sheet-protocol" style="width:auto;height:26px;font-size:12px" aria-label="Command system">
            ${PROTOCOLS.map(([v, l]) => `<option value="${v}" ${v === s.protocol ? 'selected' : ''}>${esc(l)}</option>`).join('')}
          </select>
          <span>${ledCount} LEDs · ${rows().length} effects${a ? ` · ${a.input_channels} input channels` : ''}</span>
        </div>
      </div>
      <label class="search">${ICON.search}<span class="sr-only">Filter</span>
        <input type="search" id="query" placeholder="Filter…" value="${esc(state.query)}"></label>
      <button class="btn solid" data-act="add">${ICON.plus} Add effect</button>
    </div>
    <div class="table" role="grid">
      <div class="table-inner">
        <div class="trow thead" role="row"><span>On</span><span>Name</span><span>Trigger</span><span>Effect</span><span>Output</span><span>Plug</span><span></span></div>
        <div class="tbody">${body || `<div class="empty">${rows().length ? 'No effect matches the filter.' : 'This sheet is empty.'}<br><button class="btn primary" data-act="add">${ICON.plus} Add the first effect</button></div>`}</div>
      </div>
    </div>
    ${busmapHtml()}`;
}

function busmapHtml() {
  const a = state.analysis;
  if (!a) return '';
  const owner = new Map();
  rows().forEach((r, i) => {
    const start = a.led_numbers[i];
    if (start == null || (r.led_channel ?? 0) !== 0) return;
    const m = macroOf(r.macro_text);
    const kind = outputKind(r, m);
    const leds = r.leds.trim();
    const n = /C/.test(leds) ? 1 : Math.max(parseInt(leds, 10) || 0, 0);
    for (let k = 0; k < n; k++) if (!owner.has(start + k)) owner.set(start + k, { i, kind, name: r.description || (m ? macroTitle(m) : '') });
  });
  const used = a.leds_per_channel[0];
  const shown = Math.min(Math.max(32, Math.ceil(used / 32) * 32), 256);
  let cells = '';
  for (let n = 0; n < shown; n++) {
    const o = owner.get(n);
    if (!o) { cells += `<div title="#${n} free"></div>`; continue; }
    const cls = (o.kind === 'servo' ? 'servo' : o.kind === 'sound' ? 'sound' : 'led') + (o.i === state.sel ? ' sel' : '');
    const g = o.kind === 'servo' ? 'S' : o.kind === 'sound' ? '♪' : '';
    cells += `<div class="${cls}" title="#${n} — ${esc(o.name)}" data-act="row" data-i="${o.i}">${g}</div>`;
  }
  return `
    <section class="busmap" aria-label="LED bus">
      <div class="legend">
        <b style="color:var(--text)">LED bus · channel 0</b>
        <span style="display:flex;align-items:center;gap:5px"><span class="dot" style="background:var(--light)"></span>LED</span>
        <span style="display:flex;align-items:center;gap:5px"><span class="dot" style="border:1.5px solid var(--motion);border-radius:2px"></span>Servo board</span>
        <span style="display:flex;align-items:center;gap:5px"><span class="dot" style="border:1.5px solid var(--sound);border-radius:2px"></span>Sound board</span>
        <span style="flex:1"></span>
        <span>${used} of ${a.total_leds} positions on channel 0</span>
      </div>
      <div class="cells">${cells}</div>
    </section>`;
}

// ---------------------------------------------------------------- rendering: inspector

function renderInspector() {
  const el = $('#inspector');
  if (!el) return;
  const r = rows()[state.sel];
  if (!r) {
    el.innerHTML = `<div class="label">Inspector</div><div class="muted">Select an effect to edit it.</div>`;
    return;
  }
  const m = macroOf(r.macro_text);
  const sx = sheet().protocol === 'Selectrix';
  const params = m ? paramRows(m, r.macro_text) : [];
  el.innerHTML = `
    <div style="display:flex;flex-direction:column;gap:4px">
      <span class="label">${esc(m ? (macroGroups(m)[0] || 'Effect') : 'Effect')}</span>
      <input class="input" data-bind="description" value="${esc(r.description)}" placeholder="Name / description" style="font-size:16px;font-weight:600;height:38px">
    </div>
    <div id="insp-diag"></div>

    <div class="section">
      <div class="h">Effect</div>
      <div class="effect-card">
        <div class="t"><b>${esc(m ? macroTitle(m) : 'Custom macro')}</b><button class="btn small" data-act="change">Change…</button></div>
        ${m ? `<div class="hint">${esc(mtext(m).short || '')}</div>` : ''}
      </div>
      ${params.map((p) => `<div style="display:flex;justify-content:space-between;gap:10px;font-size:12px"><span class="muted">${esc(p.label)}</span><span class="mono" style="text-align:right;overflow-wrap:anywhere">${esc(p.value)}</span></div>`).join('')}
    </div>

    <div class="section">
      <div class="h">Trigger</div>
      <div class="grid2">
        <label class="field">${sx ? 'SX channel' : sheet().protocol + ' address'}
          <input class="input mono" data-bind="address" value="${esc(r.address)}" placeholder="always on"></label>
        ${sx ? `<label class="field">Bit (1–8)<input class="input mono" data-bind="bit_pos" value="${esc(r.bit_pos)}"></label>` : `
        <label class="field">Input type
          <select class="select" data-bind="input_type">
            <option value="">—</option>
            ${INPUT_TYPES.map(([v, l]) => `<option value="${v}" ${r.input_type === v ? 'selected' : ''}>${l}</option>`).join('')}
          </select></label>`}
      </div>
      ${sx ? `<label class="field">Input type<select class="select" data-bind="input_type"><option value="">—</option>${INPUT_TYPES.map(([v, l]) => `<option value="${v}" ${r.input_type === v ? 'selected' : ''}>${l}</option>`).join('')}</select></label>` : ''}
      <div class="grid2">
        <label class="field">Start value<input class="input mono" data-bind="start_value" value="${esc(r.start_value)}" placeholder="0"></label>
        <label class="field">Group<input class="input" data-bind="filter" value="${esc(r.filter)}" placeholder="e.g. Station"></label>
      </div>
      <div class="hint">Uses ${r.in_cnt} input channel${r.in_cnt === 1 ? '' : 's'}. An empty address keeps the effect always on.</div>
    </div>

    <div class="section">
      <div class="h">Wiring</div>
      <div class="grid3">
        <label class="field">Distributor<input class="input mono" data-bind="distributor" value="${esc(r.distributor)}"></label>
        <label class="field">Plug<input class="input mono" data-bind="plug" value="${esc(r.plug)}"></label>
        <label class="field">LED channel<input class="input mono" data-bind="led_channel" value="${r.led_channel ?? ''}" placeholder="0"></label>
      </div>
    </div>

    <details class="section">
      <summary class="muted" style="cursor:pointer;font-size:12px">For experts: macro and counts</summary>
      <label class="field" style="margin-top:10px">Macro (one per line)
        <textarea class="textarea" rows="3" data-bind="macro_text" spellcheck="false">${esc(r.macro_text)}</textarea></label>
      <div class="grid3">
        <label class="field">LEDs<input class="input mono" data-bind="leds" value="${esc(r.leds)}"></label>
        <label class="field">InCnt<input class="input mono" data-bind="in_cnt" value="${r.in_cnt}"></label>
        <label class="field">Loc InCh<input class="input mono" data-bind="loc_in_ch" value="${r.loc_in_ch}"></label>
      </div>
    </details>`;
  renderDiag();
}

function renderDiag() {
  const el = $('#insp-diag');
  if (!el) return;
  const msgs = (state.analysis?.errors ?? []).filter((d) => d.row === state.sel).map((d) => d.message);
  el.innerHTML = msgs.length ? `<div class="diag">${msgs.map(esc).join('<br>')}</div>` : '';
}

/** Parameter label/value pairs of a filled macro call, for display. */
function paramRows(m, text) {
  const names = protoParams(m.prototype);
  const args = callArgs(text.split('\n')[0]);
  const out = [];
  let ai = 0;
  for (const n of names) {
    if (n === '...') {
      const rest = args.slice(ai);
      if (rest.length) out.push({ label: m.dialog === 'House' ? 'Rooms' : 'Values', value: rest.join(', ') });
      break;
    }
    const v = args[ai++];
    if (n === 'LED' || n === 'B_LED' || n === 'InCh' || v == null) continue;
    out.push({ label: paramText(cat.params.get(n)).label || n, value: v });
  }
  return out;
}

function protoParams(proto) {
  const i = proto.indexOf('(');
  if (i < 0) return [];
  const inner = proto.slice(i + 1).replace(/\)\s*$/, '');
  return inner.trim() ? inner.split(',').map((s) => s.trim()) : [];
}

function callArgs(call) {
  const i = call.indexOf('(');
  const j = call.lastIndexOf(')');
  if (i < 0 || j <= i) return [];
  const inner = call.slice(i + 1, j);
  if (!inner.trim()) return [];
  const out = [];
  let depth = 0, cur = '';
  for (const c of inner) {
    if (c === '(') depth++;
    if (c === ')') depth--;
    if (c === ',' && depth === 0) { out.push(cur.trim()); cur = ''; } else cur += c;
  }
  out.push(cur.trim());
  return out;
}

// ---------------------------------------------------------------- rendering: status bar

function renderStatus() {
  const el = $('#statusbar');
  if (!el) return;
  const a = state.analysis;
  const errs = a?.errors ?? [];
  const warns = a?.warnings ?? [];
  const first = errs[0];
  el.innerHTML = `
    ${!a ? '<span>Checking…</span>' : errs.length
      ? `<button data-act="goto-error"><span class="dot" style="background:var(--err)"></span>${errs.length} problem${errs.length === 1 ? '' : 's'} — ${esc((first.row != null ? `row ${first.row + 1}: ` : '') + first.message)}</button>`
      : '<span style="display:flex;align-items:center;gap:6px"><span class="dot" style="background:var(--ok)"></span>Ready to upload</span>'}
    ${warns.map((w) => `<span style="display:flex;align-items:center;gap:6px"><span class="dot" style="background:var(--accent)"></span>${esc(w.message)}</span>`).join('')}
    <span class="grow"></span>
    <button data-act="settings">${ICON.gear} ${esc(state.layout.board.platform)} · ${esc(state.layout.board.fqbn)}</button>
    <span class="mono">MobaLedLib ${esc(state.catalog?.lib_version || '')}</span>`;
}

// ---------------------------------------------------------------- welcome

function welcomeHtml() {
  const recent = recentList();
  const w = state.welcome || (state.welcome = { protocol: 'DCC', platform: 'AM328', name: '' });
  return `
    <div class="welcome">
      <aside>
        <div style="display:flex;align-items:center;gap:10px">${ICON.logo}<b style="font-size:17px">MobaLedLib Studio</b></div>
        <div class="navgroup">
          <div class="label" style="padding:0 8px 6px">Recent</div>
          ${recent.length ? recent.map((r) => `<button class="recent" data-act="recent" data-path="${esc(r.path)}"><b>${esc(r.name)}</b><span class="p">${esc(r.path)}</span></button>`).join('') : '<div class="hint" style="padding:0 8px">Layouts you open or save show up here. Open one with <i>File › Open…</i> or drop it onto this window.</div>'}
        </div>
        <div class="dropzone">
          <b>Coming from the Excel generator?</b>
          <span class="muted" style="line-height:1.5">In Excel, use <i>Save data to file</i>, then drop the <span class="mono">.MLL_pgf</span> file onto this window or choose <i>File › Import .MLL_pgf…</i> All sheets and effects come across.</span>
        </div>
        <span class="hint" style="margin-top:auto">MobaLedLib ${esc(state.catalog?.lib_version || '')} · ${state.catalog?.macros.length ?? 0} effects</span>
      </aside>
      <main>
        <div>
          <h1>Start a new layout</h1>
          <p class="muted" style="margin:8px 0 0;font-size:14px">Two questions, then you’re adding effects. You can change both later.</p>
        </div>
        <label class="field" style="max-width:420px">Name<input class="input" id="new-name" value="${esc(w.name)}" placeholder="e.g. Anlage Altstadt" style="height:38px;font-size:14px"></label>
        <section style="display:flex;flex-direction:column;gap:12px">
          <b>1 · How do you control your layout?</b>
          <div class="cards" role="radiogroup" aria-label="Command system">
            ${PROTOCOLS.map(([v, l, d]) => `<button class="card" role="radio" aria-checked="${w.protocol === v}" data-act="w-protocol" data-v="${v}"><b>${esc(l)}</b><span>${esc(d)}</span></button>`).join('')}
          </div>
        </section>
        <section style="display:flex;flex-direction:column;gap:12px">
          <b>2 · Which controller drives the LEDs?</b>
          <div class="cards" role="radiogroup" aria-label="Controller">
            ${PLATFORMS.map(([v, l, d]) => `<button class="card" role="radio" aria-checked="${w.platform === v}" data-act="w-platform" data-v="${v}"><b>${esc(l)}</b><span>${esc(d)}</span></button>`).join('')}
          </div>
        </section>
        <div><button class="btn primary" data-act="create" style="height:44px;padding:0 22px">Create layout</button></div>
      </main>
    </div>`;
}

// ---------------------------------------------------------------- modals

function closeModal() {
  if (state.modal?.unlisten) state.modal.unlisten();
  state.modal = null;
  renderModal();
}

function renderModal() {
  const root = $('#modal-root');
  const m = state.modal;
  if (!m) { root.innerHTML = ''; return; }
  const html = {
    picker: pickerHtml,
    upload: uploadHtml,
    header: headerHtml,
    settings: settingsHtml,
    sheet: newSheetHtml,
  }[m.type]();
  root.innerHTML = `<div class="backdrop" data-act="backdrop">${html}</div>`;
  if (m.type === 'picker') $('#picker-q')?.focus();
  if (m.type === 'upload') { const lg = $('#log'); if (lg) lg.scrollTop = lg.scrollHeight; }
}

// ----- new sheet

function newSheetHtml() {
  const m = state.modal;
  return `
    <div class="modal narrow" role="dialog" aria-modal="true" aria-label="New sheet">
      <div class="modal-head"><h2>New sheet</h2></div>
      <div style="padding:16px;display:flex;flex-direction:column;gap:14px">
        <label class="field">Name<input class="input" id="sheet-new-name" value="${esc(m.name)}"></label>
        <div class="cards" role="radiogroup" aria-label="Command system">
          ${PROTOCOLS.map(([v, l, d]) => `<button class="card" role="radio" aria-checked="${m.protocol === v}" data-act="sheet-protocol" data-v="${v}"><b>${esc(l)}</b><span>${esc(d)}</span></button>`).join('')}
        </div>
        <div class="hint">Each sheet is uploaded to its own LED Arduino.</div>
      </div>
      <div class="modal-foot">
        ${state.layout.sheets.length > 1 ? '' : ''}
        <span class="grow"></span>
        <button class="btn" data-act="close">Cancel</button>
        <button class="btn primary" data-act="sheet-create">Create sheet</button>
      </div>
    </div>`;
}

// ----- effect picker

function openPicker(editIndex = -1) {
  const m = { type: 'picker', edit: editIndex, cat: 'All', q: '', sel: null, values: {}, extra: [], error: '' };
  if (editIndex >= 0) {
    const r = rows()[editIndex];
    const def = macroOf(r.macro_text);
    if (def) selectMacro(m, def, r.macro_text);
  }
  state.modal = m;
  renderModal();
}

function selectMacro(m, def, existing) {
  m.sel = def.name;
  m.error = '';
  m.values = {};
  m.extra = [];
  const names = protoParams(def.prototype);
  const args = existing ? callArgs(existing.split('\n')[0]) : null;
  let ai = 0;
  for (const n of names) {
    if (n === '...') {
      m.extra = args ? args.slice(ai) : (def.name.startsWith('GasLight') ? ['GAS_LIGHT', 'GAS_LIGHT'] : ['ROOM_DARK', 'ROOM_BRIGHT', 'ROOM_WARM_W']);
      break;
    }
    const v = args ? args[ai] : undefined;
    ai++;
    if (n === 'LED' || n === 'B_LED' || n === 'InCh' || n === '' || n.startsWith('#')) continue;
    const p = cat.params.get(n);
    m.values[n] = v ?? (p?.default || (p?.type === 'CX' ? 'C1' : ''));
  }
}

/** [key, label, count] per category; the label is in the chosen language. */
function pickerCategories() {
  const cats = new Map([['All', ['All', 'All effects', 0]]]);
  for (const def of state.catalog.macros) {
    const k = groupKey(def);
    if (!cats.has(k)) cats.set(k, [k, macroGroups(def)[0] || k, 0]);
    cats.get(k)[2]++;
    cats.get('All')[2]++;
  }
  return [...cats.values()];
}

function pickerList(m) {
  const q = m.q.trim().toLowerCase();
  return state.catalog.macros.filter((def) => {
    if (m.cat !== 'All' && groupKey(def) !== m.cat) return false;
    if (!q) return true;
    const t = mtext(def);
    return [def.name, t.title, t.short, t.group, def.text.en?.title, def.text.de?.title].some((s) => (s || '').toLowerCase().includes(q));
  });
}

function pickerHtml() {
  const m = state.modal;
  const list = pickerList(m);
  const def = m.sel ? cat.byName.get(m.sel) : null;
  return `
    <div class="modal" role="dialog" aria-modal="true" aria-label="Choose an effect">
      <div class="modal-head">
        <h2>${m.edit >= 0 ? 'Change effect' : 'Add an effect'}</h2>
        <label class="search" style="flex:1 1 auto;max-width:520px">${ICON.search}<span class="sr-only">Search effects</span>
          <input id="picker-q" type="search" placeholder="Search: house, signal, servo, Blink…" value="${esc(m.q)}"></label>
        <span class="grow" style="flex:1"></span>
        ${langSelect()}
        <button class="btn icon" data-act="close" aria-label="Close">✕</button>
      </div>
      <div class="modal-body">
        <nav class="picker-cats" aria-label="Categories">
          ${pickerCategories().map(([k, label, n]) => `<button class="navitem" data-act="pick-cat" data-v="${esc(k)}" aria-current="${m.cat === k}"><span class="grow">${esc(label)}</span><span class="count">${n}</span></button>`).join('')}
        </nav>
        <div class="picker-list" role="listbox" aria-label="Effects">
          ${list.length ? list.map((d) => {
            const [, color] = category(d);
            return `<button class="mcard" role="option" data-act="pick" data-v="${esc(d.name)}" aria-selected="${m.sel === d.name}">
              <span class="t"><span class="dot" style="background:${color}"></span><b>${esc(macroTitle(d))}</b></span>
              <span class="d">${esc(mtext(d).short || '')}</span>
              <span class="m">${esc(d.name)}</span></button>`;
          }).join('') : '<div class="muted">No effect found.</div>'}
        </div>
        <aside class="picker-detail">${def ? pickerDetail(m, def) : '<div class="muted">Choose an effect on the left to see its settings.</div>'}</aside>
      </div>
      <div class="modal-foot">
        <span class="muted" style="font-size:12px">${list.length} of ${state.catalog.macros.length} effects</span>
        <span class="grow"></span>
        <button class="btn" data-act="close">Cancel</button>
        <button class="btn primary" data-act="pick-apply" ${def && def.dialog !== 'ColTab' ? '' : 'disabled'}>${m.edit >= 0 ? 'Apply' : 'Add to sheet'}</button>
      </div>
    </div>`;
}

function pickerDetail(m, def) {
  const t = mtext(def);
  const names = protoParams(def.prototype).filter((n) => !['LED', 'B_LED', 'InCh', '', '...'].includes(n) && !n.startsWith('#'));
  const fields = names.map((n) => {
    const p = cat.params.get(n);
    const pt = paramText(p);
    const label = pt.label && pt.label !== '=> Dialog' ? pt.label : n;
    const v = m.values[n] ?? '';
    let input;
    if (p?.type === 'CX') {
      input = `<select class="select" data-pv="${esc(n)}">${CX_VALUES.map((c) => `<option ${c === v ? 'selected' : ''}>${c}</option>`).join('')}</select>`;
    } else if (p && (p.type === 'List' || /Values:/.test(p.options))) {
      const vals = (p.options.split(';').find((s) => s.trim().startsWith('Values:')) || '').replace(/^\s*Values:/, '').split(',').map((s) => s.trim()).filter(Boolean);
      input = `<select class="select" data-pv="${esc(n)}">${(vals.includes(v) ? vals : [v, ...vals]).map((c) => `<option ${c === v ? 'selected' : ''}>${esc(c)}</option>`).join('')}</select>`;
    } else {
      input = `<input class="input mono" data-pv="${esc(n)}" value="${esc(v)}">`;
    }
    const range = p && (p.min || p.max) ? ` (${p.min || '…'} – ${p.max || '…'})` : '';
    return `<label class="field">${esc(label)}${esc(range)}${input}${pt.hint ? `<span class="hint">${esc(pt.hint.split('\n')[0])}</span>` : ''}</label>`;
  }).join('');
  const hasRest = protoParams(def.prototype).includes('...');
  const rooms = hasRest ? `
    <div class="section">
      <div class="h">${def.dialog === 'House' ? 'Rooms' : 'Lights'} — one LED each</div>
      <div class="rooms">
        ${m.extra.map((x, i) => `<div class="r"><span class="n">${i + 1}</span>
          <select class="select" data-room="${i}">${(ROOM_TYPES.includes(x) ? ROOM_TYPES : [x, ...ROOM_TYPES]).map((c) => `<option ${c === x ? 'selected' : ''}>${c}</option>`).join('')}</select>
          <button class="btn small icon" data-act="room-del" data-i="${i}" aria-label="Remove">✕</button></div>`).join('')}
      </div>
      <button class="btn small" data-act="room-add" style="align-self:flex-start">${ICON.plus} Add ${def.dialog === 'House' ? 'room' : 'light'}</button>
    </div>` : '';
  return `
    <div><span class="label">${esc(macroGroups(def).join(' › '))}</span><h3 style="margin-top:6px">${esc(macroTitle(def))}</h3></div>
    <div class="long">${esc((t.long || t.short || '').replace(/\|/g, '\n'))}</div>
    ${def.dialog === 'ColTab' ? '<div class="notice bad">This effect needs the color table editor, which MobaLedLib Studio doesn’t have yet.</div>' : ''}
    ${fields ? `<div class="section"><div class="h">Settings</div>${fields}</div>` : ''}
    ${rooms}
    ${m.error ? `<div class="diag">${esc(m.error)}</div>` : ''}
    <details><summary class="muted" style="cursor:pointer;font-size:12px">For experts: library macro</summary>
      <div class="mono" style="margin-top:8px;font-size:12px;color:var(--text-2);overflow-wrap:anywhere">${esc(def.prototype)}</div></details>`;
}

async function pickerApply() {
  const m = state.modal;
  const def = cat.byName.get(m.sel);
  if (!def) return;
  try {
    const f = await invoke('fill_macro', { name: def.name, values: m.values, extra: m.extra });
    if (m.edit >= 0) {
      const r = rows()[m.edit];
      Object.assign(r, { macro_text: f.macro_text, leds: f.leds, in_cnt: f.in_cnt, loc_in_ch: f.loc_in_ch });
      if (f.in_cnt > 0 && !r.input_type) r.input_type = f.in_cnt > 1 ? 'red' : 'on_off';
    } else {
      const at = state.sel >= 0 ? state.sel + 1 : rows().length;
      const prev = rows()[state.sel];
      const r = newRow({
        macro_text: f.macro_text, leds: f.leds, in_cnt: f.in_cnt, loc_in_ch: f.loc_in_ch,
        led_channel: prev?.led_channel ?? (parseInt(def.default_channel, 10) || 0),
        filter: state.group ?? (prev?.filter || ''),
        input_type: f.in_cnt > 0 ? (f.in_cnt > 1 ? 'red' : 'on_off') : null,
      });
      rows().splice(at, 0, r);
      state.sel = at;
    }
    closeModal();
    renderSidebar();
    changed({ inspector: true });
  } catch (e) {
    m.error = errText(e);
    renderModal();
  }
}

// ----- generated header

async function openHeader() {
  state.modal = { type: 'header', text: null, errors: null };
  renderModal();
  try {
    state.modal.text = await invoke('generate_header', { layout: state.layout, sheet: state.sheet });
  } catch (e) {
    if (state.modal) state.modal.errors = e;
  }
  if (state.modal?.type === 'header') renderModal();
}

function headerHtml() {
  const m = state.modal;
  const body = m.errors
    ? `<div style="padding:16px"><div class="notice bad" style="white-space:pre-line">${esc(errText(m.errors))}</div></div>`
    : m.text == null ? '<div style="padding:16px" class="muted">Generating…</div>'
      : `<pre class="headerview">${esc(m.text.replace(/\r\n/g, '\n'))}</pre>`;
  return `
    <div class="modal" role="dialog" aria-modal="true" aria-label="Generated code">
      <div class="modal-head"><h2>LEDs_AutoProg.h</h2><span class="muted">Generated for sheet “${esc(sheet().name)}”</span><span class="grow" style="flex:1"></span>
        <button class="btn" data-act="copy-header" ${m.text ? '' : 'disabled'}>Copy</button>
        <button class="btn icon" data-act="close" aria-label="Close">✕</button></div>
      <div class="modal-body" style="flex-direction:column">${body}</div>
    </div>`;
}

// ----- settings

function settingsHtml() {
  const l = state.layout;
  const s = l.settings;
  const presets = FQBN_PRESETS.some(([v]) => v === l.board.fqbn) ? FQBN_PRESETS : [[l.board.fqbn, l.board.fqbn], ...FQBN_PRESETS];
  return `
    <div class="modal narrow" role="dialog" aria-modal="true" aria-label="Layout settings">
      <div class="modal-head"><h2>Layout settings</h2><span class="grow" style="flex:1"></span><button class="btn icon" data-act="close" aria-label="Close">✕</button></div>
      <div style="padding:16px;display:flex;flex-direction:column;gap:18px;overflow-y:auto">
        <label class="field">Layout name<input class="input" data-set="name" value="${esc(l.name)}"></label>
        <div class="section"><div class="h">LED Arduino</div>
          <div class="grid2">
            <label class="field">Controller<select class="select" data-set="platform">${PLATFORMS.map(([v, n]) => `<option value="${v}" ${v === l.board.platform ? 'selected' : ''}>${esc(n)}</option>`).join('')}</select></label>
            <label class="field">Board (arduino-cli)<select class="select" data-set="fqbn">${presets.map(([v, n]) => `<option value="${esc(v)}" ${v === l.board.fqbn ? 'selected' : ''}>${esc(n)}</option>`).join('')}</select></label>
          </div>
        </div>
        <div class="section"><div class="h">House() timing</div>
          <div class="grid2">
            <label class="field">Min. seconds until a room changes<input class="input mono" data-set="house_min_t" value="${s.house_min_t ?? ''}" placeholder="50"></label>
            <label class="field">Max. seconds until a room changes<input class="input mono" data-set="house_max_t" value="${s.house_max_t ?? ''}" placeholder="150"></label>
          </div>
        </div>
        <div class="section"><div class="h">Command station</div>
          <div class="grid2">
            <label class="field">DCC address offset<input class="input mono" data-set="dcc_offset" value="${s.dcc_offset}"><span class="hint">4 for Roco and some others</span></label>
            <label class="field">Simulate button release<select class="select" data-set="gen_button_release_com">
              ${[[0, 'Legacy'], [1, 'Off'], [2, 'Mode 1'], [3, 'Mode 2']].map(([v, n]) => `<option value="${v}" ${v === s.gen_button_release_com ? 'selected' : ''}>${n}</option>`).join('')}</select></label>
          </div>
          <label style="display:flex;gap:10px;align-items:center"><input type="checkbox" data-set="use_spi" ${s.use_spi ? 'checked' : ''}> Use SPI between the DCC and the LED Arduino</label>
          <label style="display:flex;gap:10px;align-items:center"><input type="checkbox" data-set="color_test_mode" ${s.color_test_mode ? 'checked' : ''}> Allow the color test program to set LED colors (RS232)</label>
        </div>
        <div class="section"><div class="h">Effect texts</div>
          <div>${langSelect()}</div>
          <span class="hint">Only English and German are written by the MobaLedLib authors; the other languages are machine translations from the Excel tool.</span>
        </div>
        ${state.layout.sheets.length > 1 ? `<div class="section"><div class="h">Danger zone</div><button class="btn danger" data-act="del-sheet" style="align-self:flex-start">Delete sheet “${esc(sheet().name)}”</button></div>` : ''}
      </div>
      <div class="modal-foot"><span class="grow"></span><button class="btn primary" data-act="close">Done</button></div>
    </div>`;
}

// ----- upload

async function openUpload() {
  state.modal = { type: 'upload', status: null, ports: [], port: state.layout.board.port || '', running: false, log: '', step: -1, result: null, errors: null };
  renderModal();
  await refreshUpload();
}

async function refreshUpload() {
  const m = state.modal;
  if (!m || m.type !== 'upload') return;
  try {
    m.status = await invoke('arduino_status');
    if (m.status.cli) {
      m.ports = await invoke('list_ports');
      if (!m.port && m.ports.length) m.port = m.ports[0].address;
    }
  } catch (e) {
    m.log += errText(e) + '\n';
  }
  try {
    await invoke('generate_header', { layout: state.layout, sheet: state.sheet });
    m.errors = null;
  } catch (e) {
    m.errors = e;
  }
  if (state.modal === m) renderModal();
}

function uploadHtml() {
  const m = state.modal;
  const st = m.status;
  const l = state.layout;
  let notice = '';
  if (!st) notice = '<div class="muted">Looking for arduino-cli…</div>';
  else if (!st.cli) notice = `<div class="notice bad"><div><b>arduino-cli was not found.</b><br>MobaLedLib Studio compiles and uploads with the official <code>arduino-cli</code>. Install it (macOS: <code>brew install arduino-cli</code>, Windows/Linux: see arduino.github.io/arduino-cli), then press Refresh.</div></div>`;
  else if (!st.sketch) notice = `<div class="notice bad"><div><b>The MobaLedLib library is not installed.</b><br>Run <code>arduino-cli core install arduino:avr</code> and <code>arduino-cli lib install MobaLedLib</code> (it installs FastLED and the other dependencies), then press Refresh.</div></div>`;
  else notice = `<div class="notice ok"><div>arduino-cli ${esc(st.version)} · MobaLedLib sketch found<br><span class="mono" style="font-size:11px;opacity:.8">${esc(st.sketch)}</span></div></div>`;
  const errs = m.errors ? `<div class="notice bad" style="white-space:pre-line"><div><b>Fix these problems first:</b><br>${esc(errText(m.errors))}</div></div>` : '';
  const steps = ['Generate', 'Compile', 'Upload'].map((s, i) => {
    const cls = m.result === 'ok' && i <= (m.withUpload ? 2 : 1) ? 'done' : m.step > i ? 'done' : m.step === i ? (m.result === 'fail' ? 'failed' : 'active') : '';
    return `<div class="step ${cls}"><div class="bar"></div><b>${s}</b></div>`;
  }).join('');
  const ready = st?.cli && st?.sketch && !m.errors && !m.running;
  const presets = FQBN_PRESETS.some(([v]) => v === l.board.fqbn) ? FQBN_PRESETS : [[l.board.fqbn, l.board.fqbn], ...FQBN_PRESETS];
  return `
    <div class="modal" role="dialog" aria-modal="true" aria-label="Upload" style="width:min(900px,100%)">
      <div class="modal-head"><h2>Upload to the LED Arduino</h2><span class="muted">Sheet “${esc(sheet().name)}”</span><span class="grow" style="flex:1"></span>
        <button class="btn icon" data-act="close" aria-label="Close" ${m.running ? 'disabled' : ''}>✕</button></div>
      <div style="flex:1;min-height:0;padding:16px;display:flex;flex-direction:column;gap:14px">
        ${notice}${errs}
        <div class="grid2">
          <label class="field">Board<select class="select" id="up-fqbn" ${m.running ? 'disabled' : ''}>${presets.map(([v, n]) => `<option value="${esc(v)}" ${v === l.board.fqbn ? 'selected' : ''}>${esc(n)}</option>`).join('')}</select></label>
          <label class="field">Port
            <div style="display:flex;gap:6px">
              <select class="select" id="up-port" ${m.running ? 'disabled' : ''}>
                ${m.ports.length ? '' : '<option value="">No board found</option>'}
                ${m.ports.map((p) => `<option value="${esc(p.address)}" ${p.address === m.port ? 'selected' : ''}>${esc(p.label)}${p.board ? ' — ' + esc(p.board) : ''}</option>`).join('')}
              </select>
              <button class="btn icon" data-act="up-refresh" aria-label="Refresh" ${m.running ? 'disabled' : ''}>${ICON.refresh}</button>
            </div></label>
        </div>
        <div class="steps">${steps}</div>
        <pre class="log" id="log">${esc(m.log || 'The compiler output appears here.')}</pre>
      </div>
      <div class="modal-foot">
        <span class="hint">The DCC/Selectrix Arduino needs its own program once; v1 doesn't upload it yet.</span>
        <span class="grow"></span>
        <button class="btn" data-act="up-compile" ${ready ? '' : 'disabled'}>Compile only</button>
        <button class="btn primary" data-act="up-run" ${ready && m.port ? '' : 'disabled'}>${ICON.upload} ${m.running ? 'Working…' : 'Compile & upload'}</button>
      </div>
    </div>`;
}

async function runUpload(withUpload) {
  const m = state.modal;
  m.running = true;
  m.withUpload = withUpload;
  m.log = '';
  m.step = 0;
  m.result = null;
  if (withUpload) state.layout.board.port = m.port;
  renderModal();
  m.unlisten = await listen('build-log', (ev) => {
    const line = String(ev.payload);
    m.log += line + '\n';
    if (m.step < 1) m.step = 1;
    if (/Sketch uses|Der Sketch verwendet/.test(line) && withUpload) m.step = 2;
    if (state.modal === m) {
      const lg = $('#log');
      if (lg) { lg.textContent = m.log; lg.scrollTop = lg.scrollHeight; }
      document.querySelectorAll('.step').forEach((s, i) => { s.className = 'step ' + (m.step > i ? 'done' : m.step === i ? 'active' : ''); });
    }
  });
  try {
    await invoke('build_and_upload', { layout: state.layout, sheet: state.sheet, port: withUpload ? m.port : null });
    m.result = 'ok';
    m.step = withUpload ? 3 : 2;
    m.log += withUpload ? '\n✓ Uploaded. The LEDs should start within a few seconds.\n' : '\n✓ Compiled without errors.\n';
    toast(withUpload ? 'Uploaded to the Arduino' : 'Compiled');
  } catch (e) {
    m.result = 'fail';
    m.log += '\n✕ ' + errText(e) + '\n';
  }
  m.running = false;
  m.unlisten?.();
  m.unlisten = null;
  if (state.modal === m) renderModal();
}

// ---------------------------------------------------------------- events

document.addEventListener('click', async (ev) => {
  const t = ev.target.closest('[data-act]');
  if (!t) return;
  const i = t.dataset.i != null ? Number(t.dataset.i) : -1;
  if (t.dataset.act === 'backdrop' && ev.target !== t) return;
  await perform(t.dataset.act, t, i);
});

/** Run an action from a button (data-act) or from the native menu (menu item id). */
async function perform(act, t = null, i = -1) {
  const m = state.modal;
  switch (act) {
    // file
    case 'new': await actNew(); break;
    case 'open': await actOpen(false); break;
    case 'import': await actOpen(true); break;
    case 'save': await actSave(false); break;
    case 'saveas': await actSave(true); break;
    case 'export': await actExport(); break;
    case 'recent': await actOpenPath(t.dataset.path); break;
    case 'settings': if (state.layout) { state.modal = { type: 'settings' }; renderModal(); } break;

    // welcome
    case 'w-protocol': state.welcome.protocol = t.dataset.v; state.welcome.name = $('#new-name')?.value ?? ''; render(); break;
    case 'w-platform': state.welcome.platform = t.dataset.v; state.welcome.name = $('#new-name')?.value ?? ''; render(); break;
    case 'create': await createLayout($('#new-name').value.trim(), state.welcome.protocol, state.welcome.platform); break;

    // sheets & groups
    case 'sheet': state.sheet = i; state.sel = rows().length ? 0 : -1; state.group = null; state.analysis = null; render(); analyze(); break;
    case 'add-sheet': state.modal = { type: 'sheet', name: `Sheet ${state.layout.sheets.length + 1}`, protocol: sheet().protocol }; renderModal(); break;
    case 'sheet-protocol': m.name = $('#sheet-new-name').value; m.protocol = t.dataset.v; renderModal(); break;
    case 'sheet-create': {
      const fresh = await invoke('new_layout', { name: 'x', protocol: m.protocol, platform: state.layout.board.platform });
      const s = fresh.sheets[0];
      s.name = $('#sheet-new-name').value.trim() || s.name;
      state.layout.sheets.push(s);
      state.sheet = state.layout.sheets.length - 1;
      state.sel = 0;
      closeModal();
      render();
      changed();
      break;
    }
    case 'del-sheet':
      if (await dialog.ask(`“${sheet().name}” and its ${rows().length} effects will be removed.`, { title: 'Delete sheet?', kind: 'warning', okLabel: 'Delete', cancelLabel: 'Cancel' })) {
        state.layout.sheets.splice(state.sheet, 1);
        state.sheet = 0;
        state.sel = rows().length ? 0 : -1;
        render();
        changed();
      }
      break;
    case 'group': state.group = t.dataset.g === '' ? null : t.dataset.g; renderSidebar(); renderMain(); break;

    // rows
    case 'row': if (state.sel !== i) { state.sel = i; renderMain(); renderInspector(); } break;
    case 'toggle': rows()[i].enabled = !rows()[i].enabled; changed(); break;
    case 'add': if (state.layout) openPicker(-1); break;
    case 'change': if (rows()[state.sel]) openPicker(state.sel); break;
    case 'up': if (rows()[state.sel]) moveRow(state.sel, -1); break;
    case 'down': if (rows()[state.sel]) moveRow(state.sel, 1); break;
    case 'dup': if (rows()[state.sel]) { duplicateRow(state.sel); renderSidebar(); } break;
    case 'del': {
      const r = rows()[state.sel];
      if (!r) break;
      const m = macroOf(r.macro_text);
      const name = r.description || (m ? macroTitle(m) : r.macro_text.split('(')[0]) || 'this effect';
      if (await dialog.ask(`“${name}” will be removed from the sheet.`, { title: 'Delete effect?', kind: 'warning', okLabel: 'Delete', cancelLabel: 'Cancel' })) {
        deleteRow(state.sel);
        renderSidebar();
      }
      break;
    }
    case 'goto-error': {
      const e = state.analysis?.errors?.find((d) => d.row != null);
      if (e) { state.sel = e.row; state.group = null; renderSidebar(); renderMain(); renderInspector(); }
      break;
    }

    // picker
    case 'pick-cat': m.cat = t.dataset.v; renderModal(); break;
    case 'pick': selectMacro(m, cat.byName.get(t.dataset.v)); renderModal(); break;
    case 'room-add': m.extra.push(m.extra[m.extra.length - 1] || 'ROOM_DARK'); renderModal(); break;
    case 'room-del': m.extra.splice(i, 1); renderModal(); break;
    case 'pick-apply': await pickerApply(); break;

    // header & upload
    case 'header': if (state.layout) await openHeader(); break;
    case 'copy-header': await navigator.clipboard.writeText(m.text); toast('Copied'); break;
    case 'upload': if (state.layout) await openUpload(); break;
    case 'up-refresh': await refreshUpload(); break;
    case 'up-compile': await runUpload(false); break;
    case 'up-run': await runUpload(true); break;

    // modal basics
    case 'close': case 'backdrop': if (!m?.running) closeModal(); break;
  }
}

/** Text fields of the inspector: update the row while typing. */
function bindRow(el) {
  const r = rows()[state.sel];
  if (!r) return;
  const key = el.dataset.bind;
  let v = el.value;
  if (key === 'in_cnt' || key === 'loc_in_ch') v = Math.max(parseInt(v, 10) || 0, 0);
  else if (key === 'led_channel') v = v.trim() === '' ? null : Math.max(parseInt(v, 10) || 0, 0);
  else if (key === 'input_type') v = v || null;
  r[key] = v;
  changed();
  if (key === 'filter') renderSidebar();
}

function bindSetting(el) {
  const l = state.layout;
  const k = el.dataset.set;
  const s = l.settings;
  switch (k) {
    case 'name': l.name = el.value; renderTopbar(); break;
    case 'platform':
      l.board.platform = el.value;
      l.board.fqbn = { AM328: 'arduino:avr:nano', ESP32: 'esp32:esp32:esp32', PICO: 'rp2040:rp2040:rpipico' }[el.value];
      renderModal();
      break;
    case 'fqbn': l.board.fqbn = el.value; break;
    case 'house_min_t': case 'house_max_t': s[k] = el.value.trim() === '' ? null : Math.max(parseInt(el.value, 10) || 0, 0); break;
    case 'dcc_offset': s.dcc_offset = parseInt(el.value, 10) || 0; break;
    case 'gen_button_release_com': s.gen_button_release_com = parseInt(el.value, 10) || 0; break;
    case 'use_spi': case 'color_test_mode': s[k] = el.checked; break;
  }
  setDirty(true);
  scheduleAnalyze();
  renderStatus();
}

document.addEventListener('input', (ev) => {
  const el = ev.target;
  if (el.dataset.bind) bindRow(el);
  else if (el.dataset.set && el.type !== 'checkbox' && el.tagName !== 'SELECT') bindSetting(el);
  else if (el.id === 'query') { state.query = el.value; renderMain(); $('#query').focus(); $('#query').setSelectionRange(el.value.length, el.value.length); }
  else if (el.id === 'sheet-name') { sheet().name = el.value; setDirty(true); renderSidebar(); }
  else if (el.id === 'picker-q') {
    state.modal.q = el.value;
    renderModal();
    const q = $('#picker-q');
    q.setSelectionRange(q.value.length, q.value.length);
  } else if (el.dataset.pv) state.modal.values[el.dataset.pv] = el.value;
});

document.addEventListener('change', (ev) => {
  const el = ev.target;
  if (el.dataset.set && (el.type === 'checkbox' || el.tagName === 'SELECT')) bindSetting(el);
  else if (el.dataset.room != null) state.modal.extra[Number(el.dataset.room)] = el.value;
  else if (el.dataset.pv) state.modal.values[el.dataset.pv] = el.value;
  else if (el.id === 'sheet-protocol') { sheet().protocol = el.value; render(); changed(); }
  else if (el.id === 'up-port') state.modal.port = el.value;
  else if (el.id === 'lang') {
    state.lang = el.value;
    try { localStorage.setItem('mll.lang', state.lang); } catch { /* storage unavailable */ }
    renderModal(); renderMain(); renderInspector();
  }
  else if (el.id === 'up-fqbn') { state.layout.board.fqbn = el.value; setDirty(true); renderStatus(); }
});

// Keyboard shortcuts come from the native menu (see build_menu in main.rs).
document.addEventListener('keydown', (ev) => {
  if (ev.key === 'Escape' && state.modal && !state.modal.running) closeModal();
});

// Native apps have no browser context menu ("Reload", "Inspect"); keep it only where text can be edited.
// An effect row gets its own menu (Change, Duplicate, Move, Delete).
document.addEventListener('contextmenu', (ev) => {
  if (ev.target.closest('input, textarea, [contenteditable]')) return;
  ev.preventDefault();
  const row = ev.target.closest('.trow[data-act="row"]');
  if (!row || state.modal) return;
  const i = Number(row.dataset.i);
  if (state.sel !== i) { state.sel = i; renderMain(); renderInspector(); }
  invoke('effect_menu', { index: i, count: rows().length }).catch((e) => toast(errText(e), true));
});

listen('menu', (ev) => {
  if (state.modal?.running) return;
  perform(ev.payload);
});

listen('close-requested', async () => {
  if (await guardUnsaved()) invoke('close_window');
});

// Dropping a layout or .MLL_pgf file onto the window opens it
listen('tauri://drag-drop', (ev) => {
  const path = ev.payload?.paths?.find((p) => /\.(json|mll_pgf)$/i.test(p));
  if (path && !state.modal?.running) actOpenPath(path);
});

// ---------------------------------------------------------------- start

(async function start() {
  try {
    indexCatalog(await invoke('get_catalog'));
  } catch (e) {
    document.body.textContent = 'Could not load the macro catalog: ' + errText(e);
    return;
  }
  render();
})();
