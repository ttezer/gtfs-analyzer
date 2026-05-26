import { getState, setPage } from './state';
import { renderUpload } from './pages/upload';
import { renderDomain } from './pages/domain';
import { renderFix, attachFixListeners } from './pages/fix';
import { renderRules } from './pages/rules';
import { renderExport } from './pages/export';
import type { AppPage } from './state';

const NAV_LABELS: Record<Exclude<AppPage, 'upload'>, string> = {
  domain : 'Rapor',
  fix    : 'Ayrıntı ve Düzeltme',
  rules  : 'Kategori Bazlı',
  export : 'Dışa Aktar',
};

// ── Dark mode ─────────────────────────────────────────────────────────────────

function initDarkMode(): void {
  if (localStorage.getItem('gtfs-theme') === 'dark') {
    document.documentElement.classList.add('dark');
  }
}

function toggleDarkMode(): void {
  const isDark = document.documentElement.classList.toggle('dark');
  localStorage.setItem('gtfs-theme', isDark ? 'dark' : 'light');
  syncDarkButtons();
}

function syncDarkButtons(): void {
  const isDark = document.documentElement.classList.contains('dark');
  document.querySelectorAll<HTMLButtonElement>('.dark-toggle').forEach(btn => {
    btn.textContent = isDark ? '☀' : '☾';
    btn.title = isDark ? 'Açık mod' : 'Koyu mod';
  });
}

function darkToggleHtml(): string {
  const isDark = document.documentElement.classList.contains('dark');
  return `<button class="btn btn-ghost dark-toggle" title="${isDark ? 'Açık mod' : 'Koyu mod'}">${isDark ? '☀' : '☾'}</button>`;
}

// ── Uygulama render ───────────────────────────────────────────────────────────

export function renderApp(): void {
  const state = getState();
  const app = document.getElementById('app')!;

  if (state.page === 'upload' || !state.result) {
    app.innerHTML = `
      <header class="app-header">
        <h1>GTFS Analyzer</h1>
        <span style="flex:1"></span>
        ${darkToggleHtml()}
      </header>
      <main id="page-root"></main>`;
    app.querySelector<HTMLButtonElement>('.dark-toggle')!
      .addEventListener('click', toggleDarkMode);
    renderUpload(document.getElementById('page-root')!);
    return;
  }

  const navItems = (Object.keys(NAV_LABELS) as Exclude<AppPage, 'upload'>[])
    .map(page => `
      <button class="nav-btn ${state.page === page ? 'active' : ''}" data-page="${page}">
        ${NAV_LABELS[page]}
      </button>`)
    .join('');

  app.innerHTML = `
    <header class="app-header">
      <h1>GTFS Validator</h1>
      <span class="header-filename">${escHtml(state.fileName)}</span>
      <button id="btn-back" class="btn btn-ghost">← Ana Sayfa</button>
      <button id="btn-new" class="btn btn-secondary">Yeni GTFS Yükle</button>
      ${darkToggleHtml()}
    </header>
    <nav class="app-nav">${navItems}</nav>
    <main id="page-root"></main>`;

  app.querySelector('#btn-back')!.addEventListener('click', () => {
    setPage('upload');
    renderApp();
  });
  app.querySelector('#btn-new')!.addEventListener('click', () => {
    setPage('upload');
    renderApp();
  });
  app.querySelector<HTMLButtonElement>('.dark-toggle')!
    .addEventListener('click', toggleDarkMode);

  app.querySelectorAll<HTMLButtonElement>('.nav-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      setPage(btn.dataset['page'] as AppPage);
      renderApp();
    });
  });

  const pageRoot = document.getElementById('page-root')!;
  switch (state.page) {
    case 'domain': renderDomain(pageRoot, state.result); break;
    case 'fix':    renderFix(pageRoot, state.result); attachFixListeners(pageRoot, state.result, state.result.capped_totals); break;
    case 'rules':  renderRules(pageRoot, state.result); break;
    case 'export': renderExport(pageRoot, state.result, state.fileName); break;
  }
}

function escHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

initDarkMode();
renderApp();
