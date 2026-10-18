// MIT — DeckForge controlled witness; Elgato Stream Deck WebSocket events.
const args = process.argv.slice(2);
const arg = key => args[args.indexOf(key) + 1];
const socket = new WebSocket(`ws://127.0.0.1:${arg('-port')}`);
const settings = new Map();
const globals = new Set();
let firstContext;
const image = "<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'><rect width='16' height='16' fill='red'/></svg>";
const send = (event, context, payload) => socket.send(JSON.stringify({ event, context, payload }));
socket.addEventListener('open', () => socket.send(JSON.stringify({ event: arg('-registerEvent'), uuid: arg('-pluginUUID') })));
socket.addEventListener('message', message => {
  let data;
  try { data = JSON.parse(message.data); } catch { return; }
  if (data.event === 'didReceiveGlobalSettings') {
    if (firstContext) send('logMessage', firstContext, { message: 'global-read:' + (data.payload?.settings?.witness || 'empty') });
    if (!globals.has('global')) {
      globals.add('global');
      send('setGlobalSettings', firstContext, { witness: 'persisted' });
    }
    return;
  }
  const context = data.context;
  if (!context) return;
  if (data.event === 'willAppear') {
    firstContext ||= context;
    settings.set(context, data.payload?.settings || {});
    send('getSettings', context);
    send('getGlobalSettings', context);
  } else if (data.event === 'didReceiveSettings') {
    settings.set(context, data.payload?.settings || {});
  } else if (data.event === 'willDisappear') {
    send('logMessage', context, { message: 'willDisappear:' + context });
  } else if (data.event === 'keyDown') {
    if (data.action === 'org.deckforge.counter.write_file') {
      const folder = settings.get(context)?.output_dir;
      if (folder) require('node:fs').writeFileSync(require('node:path').join(folder, 'deckforge-witness.txt'), 'témoin');
      return;
    }
    const old = settings.get(context) || data.payload?.settings || {};
    if (old.error === 'crash') process.exit(7); // Controlled failure for the harness.
    if (old.error === 'malformed') { socket.send('{malformed'); return; }
    if (old.error === 'timeout') return;
    if (old.error === 'foreign') { send('setTitle', 'unknown-context', { title: 'INTRUS' }); return; }
    const step = Number.isSafeInteger(old.step) && old.step > 0 ? old.step : 1;
    const count = (Number.isSafeInteger(old.count) ? old.count : 0) + step;
    const updated = { ...old, count };
    settings.set(context, updated);
    send('setTitle', context, { title: String(count), target: 0 });
    send('setImage', context, { image });
    send('setState', context, { state: 1 });
    send(count === 4 ? 'showAlert' : 'showOk', context, {});
    send('setSettings', context, updated);
  }
});
