import React from 'react';
import {Composition} from 'remotion';
import {NovyDeckDemo} from './NovyDeckDemo';

export const RemotionRoot: React.FC = () => <Composition
  id="NovyDeckDemo"
  component={NovyDeckDemo}
  fps={30}
  durationInFrames={1290}
  width={1920}
  height={1080}
/>;
