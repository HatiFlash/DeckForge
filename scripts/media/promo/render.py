# Rendu image par image de promo.html (render(t) déterministe) → PNG/MP4.
# Usage : python render.py stills 0,2.5,5 out.png | python render.py video OUT.mp4 [début fin]
import json, os, subprocess, sys
from playwright.sync_api import sync_playwright

HERE = os.path.dirname(os.path.abspath(__file__))
TL = json.load(open(os.path.join(HERE, 'timeline.json')))
TL['version'] = os.environ.get('VERSION', '0.8.8')
open(os.path.join(HERE, 'timeline.js'), 'w').write('window.TL = ' + json.dumps(TL) + ';\n')
FPS = TL['fps']
import pathlib
FFMPEG = os.environ.get('FFMPEG', 'ffmpeg')
ANGLE = os.environ.get('ANGLE', 'swiftshader')  # 'd3d11' sur Windows avec GPU
ARGS = ['--use-gl=angle', f'--use-angle={ANGLE}', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-gpu',
        '--allow-file-access-from-files', '--force-device-scale-factor=1']


def page(p):
    b = p.chromium.launch(args=ARGS, headless=os.environ.get('HEADED') != '1')
    pg = b.new_page(viewport={'width': 1920, 'height': 1080})
    pg.on('console', lambda m: m.type in ('error', 'warning') and print('console:', m.text, file=sys.stderr))
    pg.on('pageerror', lambda e: print('pageerror:', e, file=sys.stderr))
    pg.goto(pathlib.Path(HERE, 'promo.html').as_uri())
    print('renderer:', pg.evaluate("(()=>{const g=document.createElement('canvas').getContext('webgl2');const e=g.getExtension('WEBGL_debug_renderer_info');return e?g.getParameter(e.UNMASKED_RENDERER_WEBGL):'?'})()"), flush=True)
    pg.wait_for_function('window.READY === true', timeout=120000)
    pg.wait_for_timeout(300)
    return b, pg


mode = sys.argv[1]
with sync_playwright() as p:
    b, pg = page(p)
    if mode == 'stills':
        times = [float(x) for x in sys.argv[2].split(',')]
        out = sys.argv[3]
        tmp = []
        for t in times:
            pg.evaluate(f'render({t})')
            f = f'{out}.{t:05.2f}.png'; pg.screenshot(path=f); tmp.append(f)
        cols = 3
        rows = (len(tmp) + cols - 1) // cols
        inputs = sum([['-i', f] for f in tmp], [])
        filt = ''.join(f'[{i}:v]scale=640:360,drawtext=fontfile=/usr/share/fonts/opentype/inter/Inter-SemiBold.otf:text={times[i]:.2f}s:x=10:y=10:fontsize=22:fontcolor=yellow:box=1:boxcolor=black@0.6[v{i}];' for i in range(len(tmp)))
        while len(tmp) < rows * cols:
            filt += f'color=black:s=640x360[v{len(tmp)}];'; tmp.append(None)
        layout = '|'.join(f'{(i % cols) * 640}_{(i // cols) * 360}' for i in range(rows * cols))
        filt += ''.join(f'[v{i}]' for i in range(rows * cols)) + f'xstack=inputs={rows * cols}:layout={layout}'
        subprocess.run([FFMPEG, '-y', '-loglevel', 'error', *inputs, '-filter_complex', filt, out], check=True)
    else:
        out = sys.argv[2]
        start = float(sys.argv[3]) if len(sys.argv) > 3 else 0.0
        end = float(sys.argv[4]) if len(sys.argv) > 4 else TL['duration']
        ff = subprocess.Popen([FFMPEG, '-y', '-loglevel', 'error', '-f', 'image2pipe', '-framerate', str(FPS), '-i', '-',
                               '-c:v', 'libx264', '-preset', 'slow', '-crf', '16', '-pix_fmt', 'yuv420p', out], stdin=subprocess.PIPE)
        n0, n1 = round(start * FPS), round(end * FPS)
        for i in range(n0, n1):
            pg.evaluate(f'render({i / FPS})')
            ff.stdin.write(pg.screenshot(type='png'))
            if i % 60 == 0:
                print(f'{out}: {i}/{n1}', flush=True)
        ff.stdin.close(); ff.wait()
    b.close()
