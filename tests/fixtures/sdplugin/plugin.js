const args = process.argv.slice(2);
const value = k => args[args.indexOf(k) + 1];
const ws = new WebSocket(`ws://127.0.0.1:${value('-port')}`);
let n = 0;
ws.addEventListener('open', () => ws.send(JSON.stringify({event: value('-registerEvent'), uuid: value('-pluginUUID')})));
ws.addEventListener('message', m => {
 const data = JSON.parse(m.data);
 if (data.event !== 'keyDown') return;
 n++;
 const context = data.context;
 for (const payload of [
   {event:'setTitle',context,payload:{title:`OK ${n}`,target:0}},
   {event:'setImage',context,payload:{image:'data:image/svg+xml;base64,'+btoa('<svg xmlns="http://www.w3.org/2000/svg"><rect width="100" height="100" fill="red"/></svg>'),target:0}},
   {event:'setSettings',context,payload:{settings:{presses:n}}},
   {event:'showOk',context}
 ]) ws.send(JSON.stringify(payload));
});
