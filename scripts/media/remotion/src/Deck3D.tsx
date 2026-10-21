import React, {useLayoutEffect, useMemo} from 'react';
import {ThreeCanvas} from '@remotion/three';
import {useLoader, useThree} from '@react-three/fiber';
import {TextureLoader, SRGBColorSpace, LinearFilter, CanvasTexture, type Texture} from 'three';
import {RoundedBoxGeometry} from 'three/examples/jsm/geometries/RoundedBoxGeometry.js';
const capGeo = new RoundedBoxGeometry(1.035, 0.31, 1.02, 4, 0.12);
const bodyGeo = new RoundedBoxGeometry(5.94, 0.36, 3.9, 6, 0.30);
const baseGeo = new RoundedBoxGeometry(6.02, 0.08, 3.98, 6, 0.33);
const faceGeo = new RoundedBoxGeometry(5.78, 0.04, 3.72, 4, 0.22);
import {interpolate, staticFile, useCurrentFrame, useVideoConfig} from 'remotion';

const clamp = {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'} as const;
const keyFiles = Array.from({length: 15}, (_, i) => `keys/key-${String(i).padStart(2, '0')}.png`);
const animatedFiles = Array.from({length: 12}, (_, i) => `keys/key-09-f${String(i).padStart(2, '0')}.png`);

const CameraRig: React.FC<{phase: number}> = ({phase}) => {
  const {camera} = useThree();
  useLayoutEffect(() => {
    // Deterministic camera orbit / dolly, no realtime clock or R3F useFrame.
    camera.position.set(Math.sin(phase * 0.9) * 1.45, 6.7 + Math.cos(phase * 0.66) * 0.65, 8.8 - Math.min(phase, 1) * 0.7);
    camera.lookAt(0, 0, 0);
    camera.updateMatrixWorld();
  }, [camera, phase]);
  return null;
};

const Lcd: React.FC<{texture: Texture; mask: Texture; x: number; z: number; index: number; frame: number; offset: number}> =
  ({texture, mask, x, z, index, frame, offset}) => {
    const pulse = Math.max(0, 1 - Math.abs(frame - (offset + index * 8)) / 10);
    return (
      <group position={[x, 0.25 - pulse * 0.08, z]}>
        <mesh castShadow>
          <primitive object={capGeo} attach="geometry" />
          <meshStandardMaterial color="#131c30" metalness={0.46} roughness={0.45} />
        </mesh>
        <mesh position={[0, 0.161, 0]} rotation={[-Math.PI / 2, 0, 0]}>
          <planeGeometry args={[0.88, 0.86]} />
          <meshBasicMaterial map={texture} alphaMap={mask} transparent toneMapped={false} />
        </mesh>

      </group>
    );
  };

const Hardware: React.FC<{phase: number; offset: number}> = ({phase, offset}) => {
  const frame = useCurrentFrame();
  const mask = useMemo(() => {
    const c = document.createElement('canvas'); c.width = 256; c.height = 256;
    const ctx = c.getContext('2d')!; ctx.fillStyle = '#000'; ctx.fillRect(0, 0, 256, 256);
    ctx.fillStyle = '#fff'; ctx.beginPath(); ctx.roundRect(1, 1, 254, 254, 24); ctx.fill();
    return new CanvasTexture(c);
  }, []);
  const textures = useLoader(TextureLoader, [...keyFiles, ...animatedFiles].map(staticFile));
  for (const texture of textures) {
    texture.colorSpace = SRGBColorSpace;
    texture.minFilter = LinearFilter;
    texture.magFilter = LinearFilter;
  }
  const start = interpolate(frame, [0, 38], [-0.37, 0], clamp);
  const spin = interpolate(frame, [0, 170], [-0.32, 0.18], clamp);
  return (
    <group rotation={[start + 0.1 * Math.sin(phase * 0.7), spin + 0.09 * Math.sin(phase), 0.035 * Math.sin(phase * 0.8)]}>
      <mesh position={[0, -0.04, 0]} receiveShadow castShadow>
        <primitive object={bodyGeo} attach="geometry" />
        <meshStandardMaterial color="#101b2d" metalness={0.68} roughness={0.48} />
      </mesh>
      <mesh position={[0, -0.21, 0]}>
        <primitive object={baseGeo} attach="geometry" />
        <meshStandardMaterial color="#1e819f" metalness={0.8} roughness={0.29} emissive="#075875" emissiveIntensity={0.12} />
      </mesh>
      <mesh position={[0, 0.119, 0]}>
        <primitive object={faceGeo} attach="geometry" />
        <meshStandardMaterial color="#26324b" metalness={0.7} roughness={0.5} />
      </mesh>
      {keyFiles.map((_, i) => <Lcd key={i} index={i} mask={mask} texture={i === 9 ? textures[15 + Math.floor(frame / 3) % animatedFiles.length] : textures[i]} x={(i % 5 - 2) * 1.13} z={(Math.floor(i / 5) - 1) * 1.13} frame={frame} offset={offset} />)}
    </group>
  );
};

export const Deck3D: React.FC<{offset?: number; zoom?: number}> = ({offset = 14, zoom = 1}) => {
  const frame = useCurrentFrame();
  const {width, height, fps} = useVideoConfig();
  const phase = frame / fps;
  return (
    <div style={{position: 'absolute', inset: 0, opacity: interpolate(frame, [0, 14], [0, 1], clamp)}}>
      <ThreeCanvas width={width} height={height} camera={{position: [0, 3, 9], fov: 39, near: 0.1, far: 60}} gl={{antialias: true, powerPreference: 'high-performance'}}>
        <color attach="background" args={['#060e1b']} />
        <ambientLight intensity={1.55} />
        <hemisphereLight args={['#86dafa', '#0b1425', 2]} />
        <directionalLight position={[-3, 7, 6]} intensity={3.2} color="#a8e9ff" />
        <pointLight position={[4, 2, -4]} intensity={36} distance={15} color="#008ec7" />
        <CameraRig phase={phase} />
        <group scale={zoom} position={[0, 0.65, 0]}>
          <Hardware phase={phase} offset={offset} />
        </group>
        <mesh position={[0, -2.8, 0]} rotation={[-Math.PI / 2, 0, 0]}>
          <planeGeometry args={[200, 200]} />
          <meshStandardMaterial color="#071324" roughness={0.5} metalness={0.32} />
        </mesh>
      </ThreeCanvas>
    </div>
  );
};
