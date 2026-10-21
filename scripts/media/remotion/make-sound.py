"""Rebuild the synchronized soundtrack (no external samples). Python stdlib only."""
import json
from pathlib import Path
from sfx.mix import Mix
from sfx.synth import pad, envelope, whoosh, click, blip, pop, chime, impact

fps = 30
lengths = [170, 180, 240, 220, 180, 170, 202]
starts = [0]
for duration in lengths[:-1]:
    starts.append(starts[-1] + duration - 12)
duration = (sum(lengths) - 12 * 6) / fps
scenes = [f / fps for f in starts]
Path('timeline.json').write_text(json.dumps({'fps':fps,'duration':duration,'sceneStarts':scenes}, indent=2))
m = Mix(duration, seed=89)
vol = envelope([(0,0),(1,1),(duration-2,1),(duration,0)])
m.put_stereo(0, *pad(duration, [(0,[146.83,220,293.66,369.99,659.25]),(scenes[-1],[146.83,220,293.66,440,587.33,739.99])], volume=vol), gain=.028)
notes = [587.33,659.25,739.99,880,987.77]
for i in range(15):
    at = scenes[1] + (20+i*8)/fps
    m.put(at, click(1400+i%5*140,dur=.04), .18, (i%5-2)*.22)
    m.put(at, blip(notes[i%5],dur=.3,tau=.09), .07, (i%5-2)*.22)
for i,at in enumerate(scenes[1:]):
    m.put(max(0,at-.18), whoosh(.7,400,3000,peak=.5), .17, .2 if i%2 else -.2)
    m.put(at+.65, pop(900+i*70,440+i*40), .13)
for i in range(5):
    at=scenes[3]+(30+i*10)/fps
    m.put(at,click(1800+i*180,dur=.04),.16,-.2)
    m.put(at,blip(notes[i],dur=.35,tau=.1),.08,-.2)
m.put(scenes[4]+2.0,chime([880,1318.5,1760],dur=1.3,tau=.5),.12,.4)
m.put(scenes[-1]+.35,impact(),.27)
m.put(scenes[-1]+1.3,chime([587.33,880,1174.66,1479.98],dur=2.2,tau=.9),.15)
print(m.master(target_lufs=-14.0,ceiling_db=-2.0))
m.write('public/soundtrack.wav')
print('soundtrack',duration,'seconds;',m.count,'events')
