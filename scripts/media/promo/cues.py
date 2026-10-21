"""Bande-son procédurale de la présentation DeckForge (aucun échantillon externe).

    python3 -m sfx cues.py
"""

from sfx.synth import blip, chime, click, envelope, impact, pad, pop, riser, whoosh

TIMELINE = "timeline.json"
OUT = "out/sfx.wav"
TARGET_LUFS = -14.0

# Ré majeur : Dmaj9 puis D6/9 au moment héros.
BED_A = [146.83, 220.0, 293.66, 369.99, 659.25]
BED_B = [146.83, 220.0, 293.66, 440.0, 587.33, 739.99]
SCALE = [587.33, 659.25, 739.99, 880.0, 987.77, 1174.66, 1318.5, 1479.98]


def score(mix, tl):
    s1, s2, s3, s4, s5, s6, s7 = (tl[k] for k in ("s1", "s2", "s3", "s4", "s5", "s6", "s7"))
    dur = tl["duration"]
    put = mix.put
    hit = s7["impact"]

    vol = envelope([(0, 0), (1.5, 0.9), (s5["typeStart"], 0.9), (s5["typeStart"] + 0.3, 0.6),
                    (s5["typeEnd"] + 0.4, 0.6), (s5["typeEnd"] + 0.9, 0.95), (hit - 0.4, 1.0),
                    (hit, 1.25), (dur - 1.4, 1.2), (dur, 0)])
    mix.put_stereo(0, *pad(dur, [(0, BED_A), (hit, BED_B)], volume=vol), gain=0.03)

    # S1 : les touches s'allument en vague, titre, accroche
    for i in range(7):
        put(s1["keysOn"] + i * 0.085, blip(SCALE[i % len(SCALE)] / 2, dur=0.3, tau=0.07), 0.07, -0.6 + i * 0.2)
    put(s1["title"] - 0.05, whoosh(0.6, 400, 2600, peak=0.6), 0.14, -0.4)
    put(s1["title"] + 0.35, blip(146.83, dur=0.8, tau=0.25), 0.3)
    put(s1["tagline"], pop(900, 450), 0.18, -0.4)
    put(s1["exit"], whoosh(0.9, 3000, 500, peak=0.4), 0.18, 0.3)

    # S2 : 15 pressions, une note par touche (pan selon la colonne)
    for i in range(15):
        t = s2["pressStart"] + i * s2["pressStep"]
        pan = -0.6 + (i % 5) * 0.3
        put(t, click(1700 + (i % 5) * 120, dur=0.04), 0.2, pan)
        put(t + 0.01, blip(SCALE[i % len(SCALE)], dur=0.35, tau=0.09), 0.09, pan)
    put(s2["caption1"], pop(820, 400), 0.14, -0.5)
    put(s2["exit"], whoosh(1.1, 300, 3600, peak=0.7, q=0.7), 0.26, 0.4)

    # S3 : arrivée sur l'application, pastilles
    put(s3["arrive"] + 0.4, chime([587.33, 880.0, 1174.66], dur=1.4, tau=0.5), 0.1)
    for i, p in enumerate(s3["pills"]):
        put(p, pop(1100 + i * 160, 520 + i * 70), 0.24, [-0.6, 0.0, 0.6, 0.2][i])
    put(s3["exit"], whoosh(1.0, 500, 3800, peak=0.65, q=0.7), 0.24, 0.4)

    # S4 : atelier, balayage des cinq blocs
    put(s4["caption"], pop(820, 400), 0.14, -0.5)
    for i in range(5):
        t = s4["sweep"] + i * s4["sweepStep"]
        put(t, click(2000 + i * 150, dur=0.035), 0.16, -0.3)
        put(t + 0.01, blip(SCALE[i + 1], dur=0.4, tau=0.1), 0.1, -0.3)
    put(s4["pill"], pop(1300, 650), 0.22, 0.5)
    put(s4["exit"], whoosh(1.0, 500, 3800, peak=0.65, q=0.7), 0.24, 0.4)

    # S5 : saisie, réponse de l'assistant
    put(s5["caption"], pop(820, 400), 0.14, -0.5)
    sentence = "ouvre brave s'il est pas ouvert, sinon un nouvel onglet"
    mix.typing(s5["typeStart"], s5["typeEnd"], len(sentence), gain=(0.12, 0.2))
    put(s5["card"], chime([880.0, 1318.5, 1760.0], dur=1.4, tau=0.5), 0.13, 0.4)
    put(s5["pill"], pop(1400, 700), 0.2, 0.5)
    put(s5["exit"], whoosh(1.4, 3600, 400, peak=0.5, q=0.7), 0.22)

    # S6 : vue d'ensemble, pastilles
    put(s6["pull"] + 0.4, pop(700, 350), 0.14)
    for i in range(6):
        put(s6["chips"] + i * s6["chipStep"], pop(1000 + i * 130, 500 + i * 60), 0.2, -0.5 + (i % 3) * 0.5)

    # S7 : retour sur le pavé, impact unique, carte de fin
    put(s7["fly"], riser(hit - s7["fly"], 220, 4200), 0.24)
    put(hit - 0.01, impact(), 0.36)
    put(hit, whoosh(0.8, 5000, 300, peak=0.15, q=0.6), 0.14)
    put(s7["logo"], chime([587.33, 880.0, 1174.66, 1479.98], dur=2.2, tau=0.9), 0.16)
    put(s7["url"], pop(1100, 600), 0.1, 0.0)
