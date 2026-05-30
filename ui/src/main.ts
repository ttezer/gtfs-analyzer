import { getState, setPage } from './state';
import { renderUpload } from './pages/upload';
import { renderDomain } from './pages/domain';
import { renderFix, attachFixListeners } from './pages/fix';
import { renderRules } from './pages/rules';
import { renderExport } from './pages/export';
import { getLocale, setLocale, t } from './i18n';
import type { AppPage } from './state';

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
    btn.title = isDark ? t('theme.light') : t('theme.dark');
  });
}

function darkToggleHtml(): string {
  const isDark = document.documentElement.classList.contains('dark');
  return `<button class="btn btn-ghost dark-toggle" title="${isDark ? t('theme.light') : t('theme.dark')}">${isDark ? '☀' : '☾'}</button>`;
}

// ── Language flags ────────────────────────────────────────────────────────────

function langFlagsHtml(): string {
  const cur = getLocale();
  const langs = [
    { lang: 'tr', label: 'TR', title: 'Türkçe' },
    { lang: 'en', label: 'EN', title: 'English' },
    { lang: 'ja', label: '日本語', title: '日本語' },
  ];
  return `<div class="lang-flags">${langs.map(f =>
    `<button class="lang-flag${cur === f.lang ? ' active' : ''}" data-lang="${f.lang}" title="${f.title}">${f.label}</button>`
  ).join('')}</div>`;
}

// ── Uygulama render ───────────────────────────────────────────────────────────

export function renderApp(): void {
  const state = getState();
  const app = document.getElementById('app')!;

  if (state.page === 'upload' || !state.result) {
    app.innerHTML = `
      <header class="app-header">
        <h1>${t('header.title')}</h1>
        <span style="flex:1"></span>
        ${langFlagsHtml()}
        ${darkToggleHtml()}
      </header>
      <main id="page-root"></main>`;
    app.querySelector<HTMLButtonElement>('.dark-toggle')!
      .addEventListener('click', toggleDarkMode);
    app.querySelectorAll<HTMLButtonElement>('.lang-flag').forEach(btn => {
      btn.addEventListener('click', () => {
        setLocale(btn.dataset['lang'] as 'tr' | 'en' | 'ja');
        if (document.querySelector('#drop-zone.loading')) {
          // ZIP yükleme aktifken tam render yapma
          document.querySelectorAll<HTMLButtonElement>('.lang-flag').forEach(b => {
            b.classList.toggle('active', b.dataset['lang'] === btn.dataset['lang']);
          });
          return;
        }
        renderApp();
      });
    });
    renderUpload(document.getElementById('page-root')!);
    return;
  }

  const NAV_PAGES: Exclude<AppPage, 'upload'>[] = ['domain', 'fix', 'rules', 'export'];
  const NAV_KEYS: Record<Exclude<AppPage, 'upload'>, string> = {
    domain : 'nav.report',
    fix    : 'nav.fix',
    rules  : 'nav.rules',
    export : 'nav.export',
  };

  const navItems = NAV_PAGES
    .map(page => `
      <button class="nav-btn ${state.page === page ? 'active' : ''}" data-page="${page}">
        ${t(NAV_KEYS[page])}
      </button>`)
    .join('');

  app.innerHTML = `
    <header class="app-header">
      <h1>${t('header.title')}</h1>
      <span class="header-filename">${escHtml(state.fileName)}</span>
      <button id="btn-back" class="btn btn-ghost">${t('header.back')}</button>
      <button id="btn-new" class="btn btn-secondary">${t('header.new')}</button>
      ${langFlagsHtml()}
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
  app.querySelectorAll<HTMLButtonElement>('.lang-flag').forEach(btn => {
    btn.addEventListener('click', () => {
      setLocale(btn.dataset['lang'] as 'tr' | 'en' | 'ja');
      renderApp();
    });
  });

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
