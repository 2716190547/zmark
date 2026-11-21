import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { globSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const english = process.argv.includes('--en');
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
const project = process.cwd();
const output = resolve('renders', english ? 'ZMark-Lightning-Draft-EN-1440p60.mp4' : 'ZMark-Lightning-Draft-1440p60.mp4');
const staging = english ? await mkdtemp(join(tmpdir(), 'zmark-promo-en-')) : null;

try {
  if (staging) {
    for (const path of ['assets', 'compositions', 'style.css', 'palettes.css', 'workbench.js']) {
      await cp(join(project, path), join(staging, path), {
        recursive: true,
        filter: (source) => source !== join(project, 'assets', 'audio', 'raw')
      });
    }
    const index = await readFile(join(project, 'index.html'), 'utf8');
    await writeFile(join(staging, 'index.html'), index
      .replace('lang="zh-CN"', 'lang="en"')
      .replace('ZMark · 光速成稿', 'ZMark · Lightning Draft'));
  }

  await mkdir('renders', { recursive: true });
  let last = -1;
  await executeRenderJob(job, staging ?? project, output, (progress, stage) => {
    const step = Math.floor(progress.progress / 5);
    if (step !== last) {
      last = step;
      console.log(`${progress.progress}% ${stage}`);
    }
  });
} finally {
  if (staging) await rm(staging, { recursive: true, force: true });
}
