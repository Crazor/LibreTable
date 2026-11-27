// SPDX-License-Identifier: MPL-2.0
//
// Dev multiclient: a thin client of the dev server's WebSocket protocol v1
// (docs/adr/0010-websocket-protocol.md, docs/design/server.md). It renders
// the table models and prompts the server sends and forwards choices; it
// never builds moves itself. Plain JavaScript, no build step (ADR-0005).
'use strict';

const PROTOCOL = 1;

let socket = null;
let welcome = null;
let messages = {};
let joined = null;
let state = null;
let wantedMatch = matchFromHash();

// ---------------------------------------------------------------- text

// Formats a message ID. The server sends every message formatted without
// arguments, with `{$name}` left in place; this fills them in.
function t(id, args = {}) {
  const text = Object.prototype.hasOwnProperty.call(messages, id) ? messages[id] : id;
  return text.replace(/\{\$([A-Za-z0-9_-]+)\}/g, (placeholder, name) =>
    Object.prototype.hasOwnProperty.call(args, name) ? String(args[name]) : placeholder);
}

function applyTexts() {
  for (const node of document.querySelectorAll('[data-t]')) {
    node.textContent = t(node.dataset.t);
  }
  for (const node of document.querySelectorAll('[data-t-placeholder]')) {
    node.placeholder = t(node.dataset.tPlaceholder);
  }
  document.documentElement.lang = welcome ? welcome.locale : 'en';
}

// ---------------------------------------------------------------- DOM

function $(id) {
  return document.getElementById(id);
}

function el(tag, props = {}, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) {
    if (key === 'class') node.className = value;
    else if (key.startsWith('on')) node.addEventListener(key.slice(2), value);
    else if (key in node) node[key] = value;
    else node.setAttribute(key, value);
  }
  for (const child of children.flat()) {
    if (child !== null && child !== undefined) {
      node.append(child instanceof Node ? child : document.createTextNode(String(child)));
    }
  }
  return node;
}

function showError(text) {
  const box = $('error');
  box.textContent = t('server-page-error', { message: text });
  box.hidden = false;
}

function clearError() {
  $('error').hidden = true;
}

// ---------------------------------------------------------------- protocol

function send(type, fields = {}) {
  if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({ v: PROTOCOL, type, ...fields }));
  }
}

function connect() {
  const scheme = location.protocol === 'https:' ? 'wss' : 'ws';
  socket = new WebSocket(`${scheme}://${location.host}/ws`);
  socket.addEventListener('open', () => {
    $('status').dataset.t = 'server-page-connected';
    send('hello', { locale: storedLocale() || navigator.language || 'en' });
  });
  socket.addEventListener('close', () => {
    $('status').dataset.t = 'server-page-disconnected';
    applyTexts();
  });
  socket.addEventListener('message', (event) => {
    let message;
    try {
      message = JSON.parse(event.data);
    } catch (error) {
      showError(String(error));
      return;
    }
    receive(message);
  });
}

function receive(message) {
  if (message.v !== PROTOCOL) {
    showError(`protocol ${message.v}`);
    return;
  }
  switch (message.type) {
    case 'welcome':
      welcome = message;
      messages = message.messages;
      setupForms();
      applyTexts();
      if (state) render();
      if (wantedMatch !== null && !joined) send('join', { match: wantedMatch });
      break;
    case 'joined':
      joined = message;
      wantedMatch = message.match;
      history.replaceState(null, '', `#match=${message.match}`);
      break;
    case 'state':
      state = message;
      render();
      break;
    case 'events':
      // Dev builds only; the record and the tables already show the
      // result, so the events go to the console for inspection.
      console.debug('events', message);
      break;
    case 'error':
      showError(errorText(message));
      break;
    default:
      console.warn('unknown message', message);
  }
}

function errorText(error) {
  const args = { ...(error.args || {}) };
  if (error.reason) args.reason = t(error.reason);
  return t(error.code, args);
}

function matchFromHash() {
  const found = /match=(\d+)/.exec(location.hash);
  return found ? Number(found[1]) : null;
}

function storedLocale() {
  try {
    return localStorage.getItem('locale');
  } catch (error) {
    return null;
  }
}

// ---------------------------------------------------------------- forms

function setupForms() {
  const locale = $('locale');
  locale.replaceChildren(...welcome.locales.map((tag) =>
    el('option', { value: tag, selected: tag === welcome.locale }, tag)));

  const game = $('game');
  const previous = game.value;
  game.replaceChildren(...welcome.games.map((info) =>
    el('option', { value: info.id }, t(info.id))));
  if (previous) game.value = previous;
  if (!previous) fillVariant();
}

function fillVariant() {
  const info = welcome.games.find((entry) => entry.id === $('game').value);
  $('variant').value = info ? JSON.stringify(info.variant) : '';
}

$('locale').addEventListener('change', (event) => {
  try {
    localStorage.setItem('locale', event.target.value);
  } catch (error) {
    // Remembering the locale is a convenience only.
  }
  send('hello', { locale: event.target.value });
});

$('game').addEventListener('change', fillVariant);

$('create-form').addEventListener('submit', (event) => {
  event.preventDefault();
  clearError();
  const fields = { game: $('game').value, seats: Number($('seats').value) };
  const seed = $('seed').value.trim();
  if (seed !== '') fields.seed = Number(seed);
  const variant = $('variant').value.trim();
  if (variant !== '') {
    try {
      fields.variant = JSON.parse(variant);
    } catch (error) {
      showError(String(error));
      return;
    }
  }
  send('create', fields);
});

$('join-form').addEventListener('submit', (event) => {
  event.preventDefault();
  clearError();
  send('join', { match: Number($('match-id').value) });
});

// ---------------------------------------------------------------- log stepping

function seek(position) {
  clearError();
  if (position === null) send('seek', {});
  else send('seek', { position: Math.max(0, Math.min(state.length, position)) });
}

$('first').addEventListener('click', () => seek(0));
$('previous').addEventListener('click', () => seek(state.position - 1));
$('next').addEventListener('click', () => seek(state.position + 1));
$('last').addEventListener('click', () => seek(state.length));
$('live').addEventListener('click', () => seek(null));
$('scrub').addEventListener('input', (event) => seek(Number(event.target.value)));

$('download').addEventListener('click', () => {
  if (!state || !state.record) return;
  const blob = new Blob([JSON.stringify(state.record, null, 2)], { type: 'application/json' });
  const link = el('a', { href: URL.createObjectURL(blob), download: `match-${state.match}.json` });
  link.click();
  URL.revokeObjectURL(link.href);
});

// ---------------------------------------------------------------- rendering

function observerTitle(observer) {
  if (observer === 'spectator') return t('server-page-spectator');
  if (observer === 'omniscient') return t('server-page-omniscient');
  return t('server-page-seat', { seat: observer.seat });
}

function findComponent(table, id) {
  for (const zone of table.zones) {
    const component = zone.components.find((item) => item.id === id);
    if (component) return { zone, component };
  }
  return null;
}

function faceText(face) {
  if (face === 'hidden') return t('server-page-face-down');
  const attributes = face.shown.attributes.map((pair) => `${pair.name}: ${pair.value}`);
  // Face keys are not message IDs yet (docs/design/localization.md).
  return attributes.length ? `${face.shown.key} (${attributes.join(', ')})` : face.shown.key;
}

function choiceText(choice, table) {
  if ('option' in choice) return t(choice.option);
  if ('number' in choice) return String(choice.number);
  if ('component' in choice) {
    const found = table && findComponent(table, choice.component);
    return found ? `${faceText(found.component.face)} #${choice.component}` : `#${choice.component}`;
  }
  if ('zone' in choice) {
    const zone = table && table.zones.find((item) => item.id === choice.zone);
    return zone ? t(zone.label) : `[${choice.zone}]`;
  }
  if ('subset' in choice) {
    return `{${choice.subset.map((id) => choiceText({ component: id }, table)).join(', ')}}`;
  }
  return JSON.stringify(choice);
}

function sameChoice(a, b) {
  return JSON.stringify(a) === JSON.stringify(b);
}

function indicatorValue(value) {
  if ('number' in value) return String(value.number);
  if ('seat' in value) return t('server-page-seat', { seat: value.seat });
  if ('message' in value) return t(value.message);
  return JSON.stringify(value);
}

function renderTable(table, prompt) {
  const selectable = prompt ? prompt.selectable : [];
  const choose = (choice) => send('choose', { seat: prompt.seat, choice });
  const isSelectable = (choice) => selectable.some((item) => sameChoice(item, choice));

  const indicators = el('ul', { class: 'indicators' }, table.indicators.map((indicator) => {
    const owner = indicator.owner === null ? '' : ` (${t('server-page-owner', { seat: indicator.owner })})`;
    return el('li', {}, `${t(indicator.label)}${owner}: ${indicatorValue(indicator.value)}`);
  }));

  const zones = table.zones.map((zone) => {
    const zoneChoice = { zone: zone.id };
    const owner = zone.owner === null ? '' : ` — ${t('server-page-owner', { seat: zone.owner })}`;
    const title = isSelectable(zoneChoice)
      ? el('button', { type: 'button', class: 'selectable', onclick: () => choose(zoneChoice) }, t(zone.label))
      : el('span', {}, t(zone.label));
    const components = zone.components.length === 0
      ? [el('span', { class: 'empty' }, t('server-page-empty'))]
      : zone.components.map((component) => {
        const choice = { component: component.id };
        const text = faceText(component.face);
        const hidden = component.face === 'hidden' ? ' hidden-face' : '';
        return isSelectable(choice)
          ? el('button', { type: 'button', class: `component selectable${hidden}`, onclick: () => choose(choice) }, text)
          : el('span', { class: `component${hidden}`, title: `#${component.id} ${component.kind}` }, text);
      });
    return el('div', { class: `zone zone-${zone.kind}` },
      el('div', { class: 'zone-title' }, title, owner),
      el('div', { class: 'components' }, components));
  });

  return el('div', { class: 'table' }, indicators, zones);
}

function renderPrompt(prompt, table) {
  const made = prompt.made.length
    ? prompt.made.map((choice) => choiceText(choice, table)).join(' → ')
    : t('server-page-nothing-chosen');
  const buttons = prompt.selectable.map((choice) => el('button', {
    type: 'button',
    class: 'selectable',
    onclick: () => send('choose', { seat: prompt.seat, choice }),
  }, choiceText(choice, table)));
  return el('div', { class: 'prompt' },
    el('p', {}, `${t('server-page-chosen')} ${made}`),
    prompt.complete
      ? el('p', {}, t('server-page-complete'))
      : el('div', { class: 'choices' }, el('span', {}, t('server-page-choose')), buttons),
    el('div', { class: 'actions' },
      el('button', {
        type: 'button',
        disabled: prompt.made.length === 0,
        onclick: () => send('undo', { seat: prompt.seat }),
      }, t('server-page-undo')),
      el('button', {
        type: 'button',
        class: 'primary',
        disabled: !prompt.complete,
        onclick: () => send('submit', { seat: prompt.seat }),
      }, t('server-page-submit'))));
}

function renderSeatControls(seat) {
  const driver = state.drivers[seat];
  const select = el('select', {
    disabled: !state.live,
    onchange: (event) => send('set-driver', { seat, driver: event.target.value }),
  },
  el('option', { value: 'human', selected: driver === 'human' }, t('server-page-driver-human')),
  el('option', { value: 'bot', selected: driver === 'bot' }, t('server-page-driver-bot')));
  const acting = state.pending.seats.includes(seat);
  return el('div', { class: 'seat-controls' },
    el('label', {}, `${t('server-page-driver')} `, select),
    el('span', { class: acting ? 'badge acting' : 'badge' },
      acting ? t('server-page-to-act') : t('server-page-waiting')));
}

function render() {
  if (!state || !joined) return;
  $('match').hidden = false;
  $('match-title').textContent = t('server-page-match', { match: state.match, game: t(joined.game) });

  $('scrub').max = String(state.length);
  $('scrub').value = String(state.position);
  $('position').textContent = t('server-page-position', { position: state.position, length: state.length });
  $('first').disabled = state.position === 0;
  $('previous').disabled = state.position === 0;
  $('next').disabled = state.position >= state.length;
  $('last').disabled = state.position >= state.length && !state.live;
  $('live').disabled = state.live;
  $('history').hidden = state.live;

  const views = state.views.map((view) => {
    const seat = typeof view.observer === 'object' ? view.observer.seat : null;
    const prompt = seat === null ? null : state.prompts.find((item) => item.seat === seat) || null;
    return el('section', { class: `view${seat === null ? ' observer' : ''}` },
      el('h3', {}, observerTitle(view.observer)),
      seat === null ? null : renderSeatControls(seat),
      renderTable(view.table, prompt),
      prompt ? renderPrompt(prompt, view.table) : null);
  });
  $('views').replaceChildren(...views);

  $('outcome').hidden = !state.outcome;
  if (state.outcome) {
    $('outcome-list').replaceChildren(...state.outcome.seats.map((result, seat) =>
      el('li', {}, t('server-page-outcome-seat', { seat, rank: result.rank, score: result.score }))));
  }

  const tableOf = (seat) => {
    const view = state.views.find((item) => typeof item.observer === 'object' && item.observer.seat === seat);
    return view ? view.table : null;
  };
  const start = el('li', { class: state.position === 0 ? 'current' : '' },
    el('button', { type: 'button', class: 'link', onclick: () => seek(0) }, t('server-page-log-start')));
  const entries = state.log.map((entry, index) => {
    const choices = entry.choices
      ? entry.choices.map((choice) => choiceText(choice, tableOf(entry.seat))).join(' → ')
      : t('server-page-log-hidden');
    const text = t('server-page-log-entry', { number: index + 1, seat: entry.seat, choices });
    return el('li', { class: state.position === index + 1 ? 'current' : '' },
      el('button', { type: 'button', class: 'link', onclick: () => seek(index + 1) }, text));
  });
  $('log-list').replaceChildren(start, ...entries);

  $('record').hidden = !state.record;
  if (state.record) $('record-json').textContent = JSON.stringify(state.record, null, 2);
  $('dev-badge').hidden = !(welcome && welcome.dev);
}

connect();
