'use strict';

// The about window (opened from the menu, see show_about in src-tauri/src/main.rs).

const { invoke } = window.__TAURI__.core;

invoke('about_info').then((i) => {
  document.getElementById('version').textContent = `Version ${i.version} · MobaLedLib ${i.lib_version}`;
});

document.addEventListener('click', (ev) => {
  const url = ev.target.closest('[data-url]')?.dataset.url;
  if (!url) return;
  // If the browser can't be opened, show the address so it can be typed in
  window.__TAURI__.opener.openUrl(url).catch(() => {
    document.getElementById('link-error').textContent = url;
  });
});

document.addEventListener('keydown', (ev) => {
  if (ev.key === 'Escape') window.__TAURI__.window.getCurrentWindow().close();
});

document.addEventListener('contextmenu', (ev) => ev.preventDefault());
