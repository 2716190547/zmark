# ZMark《今晚要发版》

58.8 秒无旁白宣传片，讲一位开发者在发版前夜，用 ZMark 从空白草稿写到可发布的说明。故事由每个镜头的一句短标题和画面推进，配乐与音效卡在 128 BPM 的小节线上。分镜在 [SPEC.md](SPEC.md)（r3 为当前版本），另外两条故事线在 [STORYLINES.md](STORYLINES.md)，审片记录在 [REVIEW.md](REVIEW.md)。上一期《光速成稿》在 [promo/zmark-v2](../zmark-v2/)。

## 结构

工作画布 1280×720，60 fps，以 2 倍设备像素比渲染为 2560×1440。`index.html` 挂载 6 幕和母带：

| 文件 | 时间（s） | 内容 |
| --- | --- | --- |
| `compositions/night.html` | 0–14.0 | 3D 符号风暴，大 `#` 在 1.81 s 砸进空白文稿；资料散乱，原文淹没 |
| `compositions/draft.html` | 14.0–21.5 | drop 上 ZMark 亮相，`⌘N` 新建草稿 |
| `compositions/split.html` | 21.5–30.875 | `⌘2` 分屏同步滚动，写 Mermaid 并渲染 |
| `compositions/dive.html` | 30.875–37.25 | 灯箱放大，流程图节点立起成 3D，镜头穿行后回到 300% |
| `compositions/outline.html` | 37.25–44.0 | `⌘⇧O` 大纲跳转、`⌘S` 首次保存、自动保存 |
| `compositions/finale.html` | 44.0–58.8 | `⌘3` 预览全文，页面碎成全片元素并汇聚成 3D 图标，片尾地址 |

`workbench.js` 按应用布局重建 ZMark 窗口和镜头标题，演示文档是 0.2.12/0.2.13 的真实发布要点。`stage3d.js` 封装三段 Three.js 场景：烤漆材质、影棚环境光和中性色调映射；每帧只由 `hf-seek` 的时间决定，配置在 `window.__hf.buildReady` 里完成。

## 编辑与导出

需要 HyperFrames 0.9.74、Node.js 22+ 和 FFmpeg。

```sh
hyperframes preview .
hyperframes check .
hyperframes render . --fps 60 --resolution 1440p --quality high -o renders/ZMark-Release-Night-1440p60.mp4
ffmpeg -i renders/ZMark-Release-Night-1440p60.mp4 -vf scale=1920:1080:flags=lanczos -c:v libx264 -crf 17 -c:a copy renders/ZMark-Release-Night-1080p60.mp4
```

渲染器只接受整数倍缩放，1080p 由 1440p 缩出。

## 改配乐或音效

从 [CREDITS.md](CREDITS.md) 的链接下载原始 MP3，放在被忽略的 `assets/audio/raw/`，然后运行 `python3 mix_audio.py assets/audio/raw/EDM-Detection-Mode.mp3`。曲目拼接点和每个音效的时间都在 `mix_audio.py` 顶部，与各幕时间轴使用同一套根时间；改动画时间后要同步调整。中文字体是文案子集，新增汉字后需要用完整的 Noto Sans SC 重新子集化 `assets/fonts/noto-sans-sc.woff2`。
