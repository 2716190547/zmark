# ZMark《光速成稿》

35 秒简体中文产品宣传片。可在 HyperFrames 编辑四个子合成，成片规格为 2560×1440、60 fps、H.264 和立体声 AAC。上一版《写下去，读得顺》保留在 [promo/zmark](../zmark/)。

## 编辑与导出

在本目录使用 HyperFrames 0.9.74、Node.js 22+、FFmpeg 和 Chromium。

```sh
hyperframes preview .
hyperframes check .
hyperframes snapshot . --review
hyperframes render . --fps 60 --resolution 1440p --quality high \
  -o renders/ZMark-Lightning-Draft-1440p60.mp4
```

`index.html` 挂载四幕和母带，`compositions/` 保存逐幕时间轴，`workbench.js` 保存演示文稿和共享界面，`palettes.css` 保存从 ZMark 主题源码取出的色值。1280×720 的工作画布以 2 倍设备像素比直接渲染至 2560×1440。中文字体是本片文案子集，修改文字时应确认新字符在字体中。

如果本机 CLI 等待下载托管 Chromium，可以设置 `HYPERFRAMES_BROWSER_PATH` 指向已有的 Chromium，再用随工程提供的渲染脚本。此脚本使用 HyperFrames 0.9.74 的内置 Producer。

```sh
export HYPERFRAMES_BROWSER_PATH=/path/to/chrome-headless-shell
ZMARK_HYPERFRAMES_DIST="$(npm root -g)/hyperframes/dist" node render.mjs
```

重新混音时，从 [音乐来源](CREDITS.md) 下载原始 MP3，然后运行 `python3 mix_audio.py /path/to/Galactic-Rap.mp3`。原生 Mermaid 图由 ZMark 的 `zmark_core::mermaid::render_to_svg_themed` 生成，源图在 `assets/workflow.mmd`。
