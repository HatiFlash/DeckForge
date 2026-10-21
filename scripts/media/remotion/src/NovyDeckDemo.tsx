import React from 'react';
import {Audio} from '@remotion/media';
import {TransitionSeries, linearTiming} from '@remotion/transitions';
import {fade} from '@remotion/transitions/fade';
import {slide} from '@remotion/transitions/slide';
import {interpolate, staticFile, useCurrentFrame, useVideoConfig} from 'remotion';
import {Intro, Keys, Dashboard, Editor, Assistant, Media, Outro} from './Scenes';

export const NovyDeckDemo: React.FC = () => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const t = linearTiming({durationInFrames: Math.round(0.4 * fps)});
  return <>
    <TransitionSeries>
      <TransitionSeries.Sequence name="Ouverture 3D" durationInFrames={170}><Intro /></TransitionSeries.Sequence>
      <TransitionSeries.Transition presentation={fade()} timing={t} />
      <TransitionSeries.Sequence name="Touches LCD" durationInFrames={180}><Keys /></TransitionSeries.Sequence>
      <TransitionSeries.Transition presentation={slide({direction: 'from-right'})} timing={t} />
      <TransitionSeries.Sequence name="Application" durationInFrames={240}><Dashboard /></TransitionSeries.Sequence>
      <TransitionSeries.Transition presentation={fade()} timing={t} />
      <TransitionSeries.Sequence name="Éditeur de blocs" durationInFrames={220}><Editor /></TransitionSeries.Sequence>
      <TransitionSeries.Transition presentation={slide({direction: 'from-right'})} timing={t} />
      <TransitionSeries.Sequence name="Assistant" durationInFrames={180}><Assistant /></TransitionSeries.Sequence>
      <TransitionSeries.Transition presentation={fade()} timing={t} />
      <TransitionSeries.Sequence name="Recadrage média" durationInFrames={170}><Media /></TransitionSeries.Sequence>
      <TransitionSeries.Transition presentation={fade()} timing={t} />
      <TransitionSeries.Sequence name="Signature NovyDeck" durationInFrames={202}><Outro /></TransitionSeries.Sequence>
    </TransitionSeries>
    <Audio name="Bande-son instrumentale originale" src={staticFile('soundtrack.wav')} durationInFrames={43 * fps} volume={interpolate(frame, [0, 20, 41 * fps, 43 * fps], [0, 1, 1, 0], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'})} />
  </>;
};
