// Schlanker statischer Dev-Server fuer die Browser-Vorschau des HardView-Frontends.
// Nur fuer Entwicklung/Vorschau; die echte App laeuft in Tauri (kein HTTP-Server).
const http = require('http');
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(path.join(__dirname, 'src'));
const PORT = process.env.PORT || 5599;
const HOST = '127.0.0.1'; // nur lokal, nicht im LAN erreichbar
const MIME = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.json': 'application/json', '.svg': 'image/svg+xml', '.woff2': 'font/woff2' };

http.createServer((req, res) => {
  let p;
  try {
    p = decodeURIComponent(req.url.split('?')[0]);
  } catch (e) {
    res.writeHead(400, { 'Content-Type': 'text/plain; charset=utf-8' });
    res.end('Bad Request: Malformed URI');
    return;
  }
  if (p === '/') p = '/index.html';

  const file = path.resolve(path.join(ROOT, p));
  // Pfad muss echt unterhalb von ROOT liegen (Separator erzwingen -> kein "src-tauri"-Bypass).
  if (file !== ROOT && !file.toLowerCase().startsWith((ROOT + path.sep).toLowerCase())) {
    res.writeHead(403);
    res.end('Forbidden');
    return;
  }
  fs.readFile(file, (err, data) => {
    if (err) { res.writeHead(404); res.end('Not found: ' + p); return; }
    res.writeHead(200, {
      'Content-Type': MIME[path.extname(file)] || 'application/octet-stream',
      'X-Content-Type-Options': 'nosniff',
      'X-Frame-Options': 'DENY',
      'Referrer-Policy': 'no-referrer'
    });
    res.end(data);
  });
}).listen(PORT, HOST, () => console.log('HardView Vorschau: http://localhost:' + PORT));
