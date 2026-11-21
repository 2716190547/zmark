# ZMark 宣传片

《写下去，读得顺》是面向官网和 GitHub 的 30 秒简体中文横版短片。暖纸色、墨色与品牌橙贯穿全片。轻音乐与操作音效为本片制作。

| 时间 | 画面 |
| --- | --- |
| 0–5 秒 | Markdown 草稿与排版稿入场 |
| 5–15 秒 | 用《星图发布手记》展示分屏和同步滚动 |
| 15–23 秒 | 同一张 Mermaid 流程图从预览放大到灯箱 |
| 23–30 秒 | ZMark 图标、支持平台和 GitHub 项目地址 |

分屏与灯箱取自 ZMark 0.2.11 的真实窗口截图。分屏滚动用起止画面的两栏长图合成，镜头转场和图表放大由 HyperFrames 渲染。

## 编辑和导出

在本目录运行以下命令。HyperFrames 0.9.74 需要 Node.js 22+、FFmpeg 和可用的 Headless Chrome。

```sh
npm ci
hyperframes preview
hyperframes check .
hyperframes render . --fps 60 --quality high -o renders/ZMark-promo-final.mp4
```

`index.html` 定义四幕和音轨，`style.css` 定义排版与颜色，`motion.js` 定义 GSAP 时间轴。Noto Sans SC 与 Noto Serif SC 的本地字体只包含本片所用字符。修改文案后可更新相应字体子集。

本次 CLI 的托管 Chrome 下载耗时较长。成片用 HyperFrames 0.9.74 内置的 Producer 和本机 Playwright Chromium 完成渲染。可在本机用下面的命令复现这一路径。设置 `HYPERFRAMES_BROWSER_PATH` 指向已有的 `chrome-headless-shell` 后运行。

```sh
HYPERFRAMES_INSTALL_ROOT="$(npm root -g)" node --input-type=module <<'JS'
import { mkdir } from 'node:fs/promises';
import { globSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const dist = join(process.env.HYPERFRAMES_INSTALL_ROOT, 'hyperframes', 'dist');
const [entry] = globSync('producer-*.js', { cwd: dist });
const { loadProducer } = await import(pathToFileURL(join(dist, entry)));
const { createRenderJob, executeRenderJob } = await loadProducer();
const job = createRenderJob({ format: 'mp4', quality: 'high', fps: 60, workers: 4 });
await mkdir('renders', { recursive: true });
await executeRenderJob(job, process.cwd(), resolve('renders/ZMark-promo-final.mp4'));
JS
```

视频使用 H.264 和立体声 AAC。本片采用单一时间轴完成跨幕动画，`hyperframes check` 会提示将四个场景拆成子合成。检查通过后，从导出文件抽查转场和片尾，并用 FFmpeg 全片解码。
