#!/usr/bin/env bash
# Monte la vidéo de présentation de DeckForge à partir d'images clés RÉELLES de l'interface.
#
# 1. Images clés (Windows, dépôt DeckForge) : rendu hors écran du vrai code d'interface egui,
#    profil « DeckForge Demo / README », 1920 × 1080 :
#      $env:DECKFORGE_VIDEO_OUTPUT="C:\chemin\frames"
#      cargo test --locked --bin deckforge -- demo_video_keyframes --ignored --test-threads=1
# 2. Montage (Linux/macOS/WSL, FFmpeg ≥ 6 avec libx264 et drawtext) :
#      scripts/media/build-demo-video.sh <dossier-frames> docs/media/deckforge-demo.mp4
#
# Aucune musique, aucune image générée par IA : seulement l'interface, le logo du dépôt et du texte.
set -euo pipefail
FRAMES=${1:?dossier des images clés}
OUT=${2:-docs/media/deckforge-demo.mp4}
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
LOGO="$ROOT/res/branding/deckforge-logo.png"
FONT_BOLD=${FONT_BOLD:-$(fc-match -f '%{file}' 'Inter:style=SemiBold')}
FONT=${FONT:-$(fc-match -f '%{file}' 'Inter:style=Regular')}
VERSION=${VERSION:-$(grep -m1 '^version' "$ROOT/Cargo.toml" | cut -d'"' -f2)}
FPS=30
HOLD=3.4      # durée d'une scène
FADE=0.5      # fondu enchaîné
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
BG="0x0B1220"

# Texte des légendes passé par fichier : apostrophes et deux-points sans échappement fragile.
txt() { local f; f=$(mktemp -p "$WORK" txt.XXXX); printf '%s' "$1" > "$f"; printf '%s' "$f"; }

# Carte titre / fin : logo et texte, apparition douce.
card() { # nom durée titre sous-titre
  local name=$1 dur=$2 title sub
  title=$(txt "$3"); sub=$(txt "$4")
  ffmpeg -loglevel error -y -f lavfi -i "color=c=$BG:s=1920x1080:r=$FPS:d=$dur" -framerate $FPS -loop 1 -t "$dur" -i "$LOGO" \
    -filter_complex "[1:v]scale=200:200,format=rgba,fade=in:st=0.2:d=0.8:alpha=1[logo];\
[0:v][logo]overlay=(W-w)/2:300[b];\
[b]drawtext=fontfile=$FONT_BOLD:expansion=none:textfile=$title:fontsize=84:fontcolor=0xF2F5FA:x=(w-tw)/2:y=560:alpha='min(1,max(0,(t-0.5)/0.8))',\
drawtext=fontfile=$FONT:expansion=none:textfile=$sub:fontsize=38:fontcolor=0x9FB0CC:x=(w-tw)/2:y=680:alpha='min(1,max(0,(t-0.8)/0.8))',format=yuv420p" \
    -r $FPS -c:v libx264 -preset slow -crf 18 -pix_fmt yuv420p "$WORK/$name.mp4"
}

# Scène : image clé, très léger rapprochement (2 %) et légende en bas à gauche.
scene() { # nom image légende
  local caption; caption=$(txt "$3")
  ffmpeg -loglevel error -y -framerate $FPS -loop 1 -t "$HOLD" -i "$2" \
    -vf "scale=3840:2160:flags=lanczos,zoompan=z='1+0.02*on/($HOLD*$FPS)':x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':d=1:s=1920x1080:fps=$FPS,\
drawtext=fontfile=$FONT_BOLD:expansion=none:textfile=$caption:fontsize=40:fontcolor=0xF2F5FA:box=1:boxcolor=0x0B1220@0.82:boxborderw=22:x=72:y=h-130:alpha='min(1,max(0,(t-0.3)/0.5))',format=yuv420p" \
    -r $FPS -c:v libx264 -preset slow -crf 18 -pix_fmt yuv420p "$WORK/$1.mp4"
}

card intro 3.5 "DeckForge" "Le centre de contrôle open source du FIFINE D6"
scene s1 "$FRAMES/01-main.png" "Les 15 touches du D6 · profil de démonstration"
scene s2 "$FRAMES/02-page-2.png" "Pages et dossiers"
scene s3 "$FRAMES/03-photo-key.png" "Images, GIF et vidéos sur les touches"
scene s4 "$FRAMES/04-workshop.png" "Atelier de blocs : une macro en 5 étapes"
scene s5 "$FRAMES/05-block-added.png" "Ajouter un bloc…"
scene s6 "$FRAMES/06-block-edited.png" "…puis le régler"
scene s7 "$FRAMES/07-assistant.png" "Assistant local : aperçu avant validation"
scene s8 "$FRAMES/08-media-crop.png" "Recadrage pour l'écran 126 × 126 de la touche"
card outro 4 "github.com/HatiFlash/DeckForge" "Version $VERSION · Windows 10/11 · MIT"

# Fondus enchaînés entre tous les segments.
inputs=(); filter=""; prev="0:v"; offset=0
segments=(intro s1 s2 s3 s4 s5 s6 s7 s8 outro)
for i in "${!segments[@]}"; do inputs+=(-i "$WORK/${segments[$i]}.mp4"); done
dur() { ffprobe -v error -show_entries format=duration -of csv=p=0 "$WORK/$1.mp4"; }
calc() { awk "BEGIN { print $1 }"; }
offset=$(calc "$(dur intro) - $FADE")
for ((i = 1; i < ${#segments[@]}; i++)); do
  label="x$i"
  filter+="[$prev][$i:v]xfade=transition=fade:duration=$FADE:offset=$offset[$label];"
  prev=$label
  offset=$(calc "$offset + $(dur "${segments[$i]}") - $FADE")
done
mkdir -p "$(dirname "$OUT")"
ffmpeg -loglevel error -y "${inputs[@]}" -filter_complex "${filter%;}" -map "[$prev]" \
  -c:v libx264 -preset slow -crf 20 -profile:v high -pix_fmt yuv420p -movflags +faststart -r $FPS "$OUT"
# Vignette : première scène de l'interface.
ffmpeg -loglevel error -y -ss 4.5 -i "$OUT" -frames:v 1 -vf scale=1280:-1 "${OUT%.mp4}-poster.png"
ffprobe -v error -show_entries format=duration:stream=width,height,codec_name,r_frame_rate -of compact "$OUT"
