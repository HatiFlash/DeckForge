import {Config} from '@remotion/cli/config';

// Portable preset; the Windows render script also sets --gl=angle and --concurrency=2.
Config.setConcurrency(2);
Config.setOverwriteOutput(true);
