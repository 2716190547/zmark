import { mkdir } from 'node:fs/promises';
import { globSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const dist = process.env.ZMARK_HYPERFRAMES_DIST;
const [entry] = globSync('producer-*.js', { cwd: dist });
const { loadProducer } = await import(pathToFileURL(join(dist, entry)));
const { createRenderJob, executeRenderJob, resolveConfig } = await loadProducer();
const job = createRenderJob({
  format: 'mp4',
  quality: 'high',
  fps: 60,
  workers: 5,
  outputResolution: 'landscape-1440p',
  engineConfig: resolveConfig({ browserGpuMode: 'auto' })
});
await mkdir('renders', { recursive: true });
let last = -1;
await executeRenderJob(job, process.cwd(), resolve('renders/ZMark-Lightning-Draft-1440p60.mp4'), (progress, stage) => {
  const step = Math.floor(progress.progress / 5);
  if (step !== last) {
    last = step;
    console.log(`${progress.progress}% ${stage}`);
  }
});
