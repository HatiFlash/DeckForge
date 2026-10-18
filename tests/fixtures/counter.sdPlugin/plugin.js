// MIT — DeckForge controlled witness; Elgato Stream Deck WebSocket events.
const args = process.argv.slice(2);
const arg = key => args[args.indexOf(key) + 1];
const socket = new WebSocket(`ws://127.0.0.1:${arg('-port')}`);
const settings = new Map();
const send = (event, context, payload) => socket.send(JSON.stringify({ event, context, payload }));
socket.addEventListener('open', () => socket.send(JSON.stringify({ event: arg('-registerEvent'), uuid: arg('-pluginUUID') })));
socket.addEventListener('message', message => {
  let data;
  try { data = JSON.parse(message.data); } catch { return; }
  const context = data.context;
  if (!context) return;
  if (data.event === 'willAppear') {
    settings.set(context, data.payload?.settings || {});
    send('getSettings', context);
  } else if (data.event === 'didReceiveSettings') {
    settings.set(context, data.payload?.settings || {});
  } else if (data.event === 'keyDown') {
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
    send('setSettings', context, { settings: updated });
  }
});
