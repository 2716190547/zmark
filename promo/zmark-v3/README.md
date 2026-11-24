# ZMark《今晚要发版》

72 秒简体中文旁白宣传片，讲一位开发者在发版前夜，用 ZMark 从空白草稿写到可发布的说明。分镜和旁白稿在 [SPEC.md](SPEC.md)，另外两条故事线在 [STORYLINES.md](STORYLINES.md)，审片记录在 [REVIEW.md](REVIEW.md)。上一期《光速成稿》在 [promo/zmark-v2](../zmark-v2/)。

## 结构

工作画布 1280×720，60 fps，以 2 倍设备像素比渲染为 2560×1440。`index.html` 挂载 6 幕和母带：

| 文件 | 时间（s） | 内容 |
| --- | --- | --- |
| `compositions/night.html` | 0–14.4 | 夜里的空白文稿，3D `#` 落入，资料散乱 |
| `compositions/draft.html` | 14.4–23.0 | ZMark 亮相，`⌘N` 新建草稿并开始写 |
| `compositions/split.html` | 23.0–38.3 | `⌘2` 分屏、同步滚动、写 Mermaid 并渲染 |
| `compositions/dive.html` | 38.3–44.4 | 灯箱放大，流程图节点立起成 3D，镜头穿过节点 |
| `compositions/outline.html` | 44.4–58.5 | Drawer、`⌘⇧O` 大纲跳转、`⌘S` 首次保存、自动保存 |
| `compositions/finale.html` | 58.5–72.0 | `⌘3` 预览全文，3D 图标落成片尾 |

`workbench.js` 按应用布局重建 ZMark 窗口，演示文档是 0.2.12/0.2.13 的真实发布要点。`stage3d.js` 封装三段 Three.js 场景：每帧只由 `hf-seek` 的时间决定，配置在 `window.__hf.buildReady` 里完成。

## 编辑与导出

需要 HyperFrames 0.9.74、Node.js 22+ 和 FFmpeg。

```sh
hyperframes preview .
hyperframes check .
hyperframes render . --fps 60 --resolution 1440p --quality high -o renders/ZMark-Release-Night-1440p60.mp4
ffmpeg -i renders/ZMark-Release-Night-1440p60.mp4 -vf scale=1920:1080:flags=lanczos -c:v libx264 -crf 17 -c:a copy renders/ZMark-Release-Night-1080p60.mp4
```

渲染器只接受整数倍缩放，1080p 由 1440p 缩出。

## 改旁白或配乐

旁白每句一个文件 `assets/audio/vo/lNN.wav`。改稿后重新生成对应句子，再重混母带。Kokoro 需要 `kokoro-onnx` 和 `soundfile`，放在独立虚拟环境里：

```sh
uv venv --python 3.12 ~/.venvs/hyperframes-tts
VIRTUAL_ENV=~/.venvs/hyperframes-tts uv pip install kokoro-onnx soundfile
HYPERFRAMES_PYTHON=~/.venvs/hyperframes-tts/bin/python hyperframes tts "换个写法，打开 Zee Mark。" -v zf_xiaobei -o assets/audio/vo/l03.wav
python3 mix_audio.py /path/to/Chill-Wave.mp3
```

产品名写成 “Zee Mark”、“Mac OS”，让中文声线按英文读。句子起点在 `mix_audio.py` 的 `VO` 列表；句子变长时，同步调整对应幕的动作时间。原始配乐从 [CREDITS.md](CREDITS.md) 的链接下载，放在被忽略的 `assets/audio/raw/`。中文字体是文案子集，新增汉字后需要用完整的 Noto Sans SC 重新子集化 `assets/fonts/noto-sans-sc.woff2`。
