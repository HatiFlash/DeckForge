import React from 'react';
import {loadFont} from '@remotion/fonts';
import {AbsoluteFill, Easing, Img, interpolate, staticFile, useCurrentFrame, useVideoConfig, delayRender, continueRender, cancelRender} from 'remotion';
import {Deck3D} from './Deck3D';

const fontsHandle = delayRender("Chargement des polices NovyDeck");
Promise.all([
  loadFont({family: 'Novy Inter', url: staticFile('fonts/Inter-Regular.otf'), weight: '400'}),
  loadFont({family: 'Novy Inter', url: staticFile('fonts/Inter-Medium.otf'), weight: '500'}),
  loadFont({family: 'Novy Inter', url: staticFile('fonts/Inter-SemiBold.otf'), weight: '600'}),
  loadFont({family: 'Novy Inter', url: staticFile('fonts/InterDisplay-Bold.otf'), weight: '700'}),
]).then(() => continueRender(fontsHandle)).catch(cancelRender);

const ease = Easing.bezier(0.16, 1, 0.3, 1);
const bound = {extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: ease} as const;
const cyan = '#62def5';
const background: React.CSSProperties = {background: 'radial-gradient(ellipse at 58% 36%, #163553 0%, #071525 42%, #050b15 82%)', color: '#e9f5fc', fontFamily: 'Novy Inter, sans-serif', overflow: 'hidden'};

const Atmosphere: React.FC = () => {
  const f = useCurrentFrame();
  return <AbsoluteFill style={{pointerEvents: 'none', background: `radial-gradient(circle at ${48 + 8 * Math.sin(f / 72)}% 58%, rgba(28,177,224,.12),transparent 54%), repeating-linear-gradient(0deg,transparent 0px,transparent 86px,rgba(115,214,249,.025) 87px,transparent 88px)`, boxShadow: 'inset 0 0 170px 70px #030811'}} />;
};

const Label: React.FC<{children: React.ReactNode; delay?: number}> = ({children, delay = 0}) => {
  const f = useCurrentFrame();
  return <div style={{fontSize: 27, fontWeight: 600, color: cyan, letterSpacing: 5, textTransform: 'uppercase', opacity: interpolate(f, [delay, delay + 14], [0, 1], bound), translate: `0 ${interpolate(f, [delay, delay + 20], [14, 0], bound)}px`}}>{children}</div>;
};

const Headline: React.FC<{children: React.ReactNode; delay?: number; size?: number}> = ({children, delay = 10, size = 82}) => {
  const f = useCurrentFrame();
  return <div style={{fontSize: size, fontWeight: 700, letterSpacing: -3.5, lineHeight: 1.1, opacity: interpolate(f, [delay, delay + 19], [0, 1], bound), translate: `0 ${interpolate(f, [delay, delay + 28], [60, 0], bound)}px`, textShadow: '0 14px 40px #010b17'}}>{children}</div>;
};

const Subline: React.FC<{children: React.ReactNode; delay?: number}> = ({children, delay = 24}) => {
  const f = useCurrentFrame();
  return <div style={{fontSize: 33, fontWeight: 400, color: '#bed4e2', lineHeight: 1.4, opacity: interpolate(f, [delay, delay + 20], [0, 1], bound), translate: `0 ${interpolate(f, [delay, delay + 24], [25, 0], bound)}px`}}>{children}</div>;
};

const Frame: React.FC<{src: string; width?: number; height?: number; right?: number; top?: number; delay?: number}> = ({src, width = 1290, height = 784, right = 70, top = 160, delay = 8}) => {
  const f = useCurrentFrame();
  return <div style={{position: 'absolute', right, top, width, height, borderRadius: 22, padding: 10, background: 'linear-gradient(135deg,#4598bd,#182b42 18%,#091420 85%,#3d98b9)', boxShadow: '0 42px 100px #000b, 0 0 80px #119fd336', border: '1px solid #6accf250', opacity: interpolate(f, [delay, delay + 22], [0, 1], bound), translate: `${interpolate(f, [delay, delay + 34], [125, 0], bound)}px ${interpolate(f, [delay, delay + 34], [45, 0], bound)}px`, rotate: `${interpolate(f, [delay, delay + 44], [2.5, 0], bound)}deg`}}>
    <div style={{width:'100%', height:'100%', overflow:'hidden', borderRadius:13, position:'relative'}}>
      <Img src={staticFile(`shots/${src}.png`)} style={{height: '100%', width: '100%', objectFit: 'contain', background: '#09111d', transformOrigin: src === 'assistant' ? '90% 48%' : src === 'block-editor' ? '30% 35%' : '50% 45%', transform: `scale(${interpolate(f,[60,160],[1,src === 'assistant' ? 1.42 : src === 'block-editor' ? 1.22 : 1.06],bound)})`}} />
      {src === 'block-editor' && [0,1,2,3,4].map((i) => <div key={i} style={{position:'absolute',left:'17.7%',top:`${18.5+i*7.4}%`,width:'33%',height:'5%',border:`2px solid ${cyan}`,borderRadius:9,opacity:interpolate(f,[30+i*10,40+i*10,105,125],[0,.75,.75,0],bound),boxShadow:'0 0 18px #22c3ee55'}} />)}
      <div style={{position:'absolute',right:22,bottom:22,padding:'10px 18px',borderRadius:40,background:'#071525ed',border:'1px solid #62def555',color:cyan,fontSize:21,fontWeight:600,opacity:interpolate(f,[45,65],[0,1],bound)}}>{src === 'assistant' ? 'Aperçu · aucune exécution' : src === 'block-editor' ? '5 blocs · simulation disponible' : src === 'media-crop' ? 'Recadrage non destructif' : 'Profil démo · 15 touches réelles'}</div>
    </div>
  </div>;
};

const Index: React.FC<{n: string; label: string}> = ({n, label}) => <div style={{position: 'absolute', left: 86, bottom: 66, fontSize: 23, letterSpacing: 3, color: '#8aaaba'}}>{n} <span style={{color: cyan, margin: '0 12px'}}>—</span> {label.toUpperCase()}</div>;
const Stage: React.FC<{children: React.ReactNode}> = ({children}) => <AbsoluteFill style={background}>{children}<Atmosphere /><div style={{position: 'absolute', left: 86, right: 86, top: 74, height: 1, background: 'linear-gradient(90deg,#6de2f570,transparent 66%)'}} /></AbsoluteFill>;

export const Intro: React.FC = () => {
  const f = useCurrentFrame();
  return <Stage>
    <Deck3D offset={26} zoom={1.16} /><Img src={staticFile("logo.png")} style={{position:"absolute",right:94,top:100,width:180,height:180,borderRadius:24,opacity:interpolate(f,[8,32],[0,1],bound)}} />
    <div style={{position: 'absolute', left: 92, top: 110, padding: '14px 23px', border: '1px solid #56c8e277', borderRadius: 9, background: '#05182ad9', opacity: interpolate(f, [18, 34], [0, 1], bound)}}><Label delay={22}>NOUVEAU NOM · MÊME LIBERTÉ</Label></div>
    <div style={{position: 'absolute', left: 90, top: 790, maxWidth: 1500}}><Headline delay={35} size={142}><span style={{color: cyan}}>Novy</span>Deck</Headline><Subline delay={65}>Ton pavé. Tes règles. · FIFINE D6</Subline></div>
  </Stage>;
};

export const Keys: React.FC = () => <Stage>
  <Deck3D offset={20} zoom={1.24} />
  <div style={{position: 'absolute', left: 90, top: 118, maxWidth: 1720}}><Label>01 · MATÉRIEL</Label><Headline delay={14} size={76}>15 touches LCD. <span style={{color: cyan}}>À ton image.</span></Headline></div>
  <div style={{position: 'absolute', right: 88, bottom: 110, width: 465, borderLeft: `3px solid ${cyan}`, paddingLeft: 28, background: '#051322b3'}}><Subline delay={65}>Une interface visuelle sur chaque touche du D6.</Subline></div>
  <Index n="01 / 06" label="Le matériel" />
</Stage>;

export const Dashboard: React.FC = () => <Stage>
  <div style={{position: 'absolute', left: 86, top: 170, width: 420}}><Label>02 · L'APPLICATION</Label><Headline delay={12} size={75}>Tout<br />sous les<br /><span style={{color: cyan}}>doigts.</span></Headline><Subline delay={43}>Configure ton pavé. Profils, pages et actions réunis.</Subline></div>
  <Frame src="main" width={1310} height={820} right={72} top={134} delay={13} />
  <Index n="02 / 06" label="Vue principale" />
</Stage>;

export const Editor: React.FC = () => <Stage>
  <div style={{position: 'absolute', left: 86, top: 168, width: 442}}><Label>03 · CRÉATION</Label><Headline delay={10} size={72}>Composez<br />vos <span style={{color: cyan}}>actions.</span></Headline><Subline delay={39}>L'éditeur de blocs, visuel et précis.</Subline></div>
  <Frame src="block-editor" width={1320} height={815} right={70} top={145} delay={12} />
  <Index n="03 / 06" label="Éditeur de blocs" />
</Stage>;

export const Assistant: React.FC = () => <Stage>
  <div style={{position: 'absolute', left: 86, top: 150, width: 450}}><Label>04 · ASSISTANCE</Label><Headline delay={11} size={73}>Vos idées.<br /><span style={{color: cyan}}>Moins de<br />friction.</span></Headline><Subline delay={41}>Décris une action. Vérifie les blocs avant de valider.</Subline></div>
  <Frame src="assistant" width={1300} height={800} right={76} top={160} delay={12} />
  <Index n="04 / 06" label="Assistant" />
</Stage>;

export const Media: React.FC = () => <Stage>
  <div style={{position: 'absolute', left: 86, top: 150, width: 455}}><Label>05 · MÉDIAS</Label><Headline delay={9} size={71}>Tes médias,<br /><span style={{color: cyan}}>à sa place.</span></Headline><Subline delay={38}>Cadrez vos contenus pour les écrans LCD.</Subline></div>
  <Frame src="media-crop" width={1295} height={784} right={80} top={170} delay={11} />
  <Index n="05 / 06" label="Médias" />
</Stage>;

export const Outro: React.FC = () => {
  const f = useCurrentFrame();
  return <Stage>
    <div style={{position: 'absolute', left: 70, top: 90, width: 790, height: 790, borderRadius: '50%', background: 'radial-gradient(circle,#1cbbe158 0%,#005f9340 34%,transparent 70%)', filter: 'blur(45px)', scale: interpolate(f, [0, 110], [0.6, 1.15], bound)}} />
    <div style={{position: 'absolute', left: 178, top: 136, width: 654, height: 654, borderRadius: 38, border: '1px solid #52ddfa42', boxShadow: '0 0 95px #08bde23c', overflow: 'hidden', opacity: interpolate(f, [10, 32], [0, 1], bound), scale: interpolate(f, [10, 65], [0.76, 1], bound)}}>
      <Img src={staticFile('logo.png')} style={{width: '100%', height: '100%', objectFit: 'cover'}} />
    </div>
    <div style={{position: 'absolute', left: 920, top: 245}}><Label delay={18}>FIFINE D6 · CRÉEZ À VOTRE RYTHME</Label><Headline delay={37} size={130}><span style={{color: cyan}}>Novy</span>Deck</Headline><Subline delay={65}>Ton pavé. Tes règles.</Subline><div style={{fontSize: 37, letterSpacing: 6, color: '#86d5e9', marginTop: 96, opacity: interpolate(f, [88, 110], [0, 1], bound)}}>VERSION 0.8.9</div><div style={{fontSize:28,color:"#c4dfed",marginTop:30,opacity:interpolate(f,[75,95],[0,1],bound)}}>github.com/HatiFlash/NovyDeck</div></div>
    <div style={{position: 'absolute', right: 95, bottom: 86, width: 520, height: 2, background: 'linear-gradient(90deg,#5de1f5,transparent)', scale: interpolate(f, [75, 122], [0, 1], bound), transformOrigin: 'left'}} />
  </Stage>;
};
