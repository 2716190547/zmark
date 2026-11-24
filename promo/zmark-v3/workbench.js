// The ZMark window, rebuilt from the app's layout (toolbar, Drawer, panes) and
// theme tokens. The document is the real release note for 0.2.12 and 0.2.13.
const icon = (name, label = '') => `<i class="icon" style="--icon:url(assets/icons/${name}.svg)" aria-hidden="true"></i>${label}`;

// [kind, text]: h = heading, li = list item, p = paragraph, f = fence, c = code, '' = blank
const DOC = [
  ['h', '# ZMark 发布说明'], ['', ''], ['p', '今晚发两个版本，先把要点写下来。'], ['', ''],
  ['h', '## 0.2.13'], ['li', '- 安装包和自动更新迁回源码仓库'], ['li', '- 关于页和欢迎页链接同步更新'], ['li', '- 旧仓库归档，历史版本保留'], ['', ''],
  ['h', '## 0.2.12'], ['li', '- 草稿编辑后不再误报保存错误'], ['li', '- 首次保存时再选位置'], ['', ''], ['', ''],
  ['h', '## 已知限制'], ['li', '- macOS 应用未经 Apple 公证'], ['li', '- Windows 安装包未做代码签名'], ['', ''],
  ['h', '## 下载'], ['li', '- macOS · Windows · Linux'], ['', ''],
  ['h', '## 发布流程'], ['f', '```mermaid'], ['c', 'flowchart LR'], ['c', 'A[写草稿] --> B{校对}'],
  ['c', 'B -->|修改| A'], ['c', 'B -->|通过| C[打包构建]'], ['c', 'C --> D[发布]'], ['f', '```'],
];
const LATE_LINE = 12; // typed after the first save
const LATE_TEXT = '- 保存后照常自动保存';
const LINE = { intro: 2, v13: 4, v12: 9, limits: 14, download: 18, flow: 21, fenceEnd: 28 };

const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
function lineHTML(kind, text) {
  const t = esc(text);
  if (kind === 'h') return `<b>${t}</b>`;
  if (kind === 'f') return `<em>${t}</em>`;
  if (kind === 'li') return `<em>-</em>${t.slice(1)}`;
  return t;
}

// Preview blocks carry data-ln: the source line that produces them.
const previewHTML = (late) => `
  <h1 class="pv-top" data-ln="0">ZMark 发布说明</h1>
  <p data-ln="2">今晚发两个版本，先把要点写下来。</p>
  <h2 class="pv-v13" data-ln="4">0.2.13</h2>
  <ul><li data-ln="5">安装包和自动更新迁回源码仓库</li><li data-ln="6">关于页和欢迎页链接同步更新</li><li data-ln="7">旧仓库归档，历史版本保留</li></ul>
  <h2 class="pv-v12" data-ln="9">0.2.12</h2>
  <ul><li data-ln="10">草稿编辑后不再误报保存错误</li><li data-ln="11">首次保存时再选位置</li><li class="late"${late ? '' : ' style="display:none"'}>保存后照常自动保存</li></ul>
  <h2 class="pv-limits" data-ln="14">已知限制</h2>
  <ul><li data-ln="15">macOS 应用未经 Apple 公证</li><li data-ln="16">Windows 安装包未做代码签名</li></ul>
  <h2 class="pv-download" data-ln="18">下载</h2>
  <ul><li data-ln="19">macOS · Windows · Linux</li></ul>
  <h2 class="pv-flow" data-ln="21">发布流程</h2>
  <pre class="code-grow"></pre>
  <div class="mermaid-card"><img src="assets/release-flow.svg" alt="发布流程图"><span class="expand">${icon('maximize')}</span></div>`;

// opts: title, dirty, mode ('source' | 'split' | 'preview'), sidebar, lines (source lines shown),
// late (the line typed after saving), mermaid (rendered card shown), saved (file in the folder tree)
function buildWorkbench(target, opts = {}) {
  const { title = '未命名', dirty = false, mode = 'source', sidebar = false, lines = DOC.length, late = false, mermaid = true, saved = false } = opts;
  const source = DOC.map(([kind, text], i) => {
    const html = i === LATE_LINE ? (late ? lineHTML('li', LATE_TEXT) : '') : lineHTML(kind, text);
    return `<span class="ln" data-ln="${i}">${i < lines ? html : ''}</span>`;
  }).join('');
  target.innerHTML = `
    <header class="toolbar">
      <span class="traffic"><i></i><i></i><i></i></span>
      <div class="drawer-tools">
        <span class="tool drawer-toggle">${icon('panel-left')}</span>
        <div class="drawer-tabs" style="visibility:${sidebar ? 'visible' : 'hidden'}">
          <span class="tool files-tab selected">${icon('folder')}</span>
          <span class="tool outline-tab">${icon('list')}</span>
          <span class="tool">${icon('folder-plus')}</span>
        </div>
      </div>
      <span class="file-label${dirty ? ' dirty' : ''}">${title}</span>
      <div class="mode">
        <span class="mode-source${mode === 'source' ? ' selected' : ''}">${icon('code')}</span>
        <span class="mode-split${mode === 'split' ? ' selected' : ''}">${icon('columns-2')}</span>
        <span class="mode-preview${mode === 'preview' ? ' selected' : ''}">${icon('book-open')}</span>
      </div>
      <div class="tools"><span>${icon('search')}</span><span>${icon('download')}</span><span>${icon('settings')}</span></div>
    </header>
    <div class="body">
      <aside class="sidebar" data-layout-allow-overflow>
        <div class="tree">
          <div class="folder">${icon('folder-open', '发布')}</div>
          <div class="file">${icon('file', '校对清单.md')}</div>
          <div class="file saved-file${saved ? ' active' : ''}" style="display:${saved ? 'flex' : 'none'}">${icon('file', 'release.md')}</div>
          <div class="group draft-group" style="display:${saved ? 'none' : 'flex'}">草稿</div>
          <div class="file draft-file active" style="display:${saved ? 'none' : 'flex'}">${icon('file', '未命名')}</div>
        </div>
        <div class="outline">
          <div class="ol-top">ZMark 发布说明</div>
          <div class="sub ol-v13">0.2.13</div>
          <div class="sub ol-v12">0.2.12</div>
          <div class="sub ol-limits">已知限制</div>
          <div class="sub ol-download">下载</div>
          <div class="sub ol-flow">发布流程</div>
        </div>
      </aside>
      <div class="panes">
        <div class="source"><div class="scroll" data-layout-allow-overflow>${source}</div></div>
        <div class="divider"></div>
        <div class="preview"><div class="scroll" data-layout-allow-overflow>${previewHTML(late)}</div></div>
      </div>
    </div>`;
  target.style.setProperty('--cut', { source: '100%', split: '50%', preview: '0%' }[mode]);
  target.style.setProperty('--side', sidebar ? '230px' : '0px');
  target.querySelector('.mermaid-card').style.display = mermaid ? 'block' : 'none';
  target.querySelectorAll('.preview [data-ln]').forEach((el) => {
    if (Number(el.dataset.ln) >= lines) el.style.visibility = 'hidden';
  });
}

// Shows source line i (fully written) and its preview block at time `at`.
function revealLine(tl, root, i, at) {
  const [kind, text] = DOC[i];
  TEXT.set(root.querySelector('.source [data-ln="' + i + '"]'), lineHTML(kind, text), at);
  const block = root.querySelector('.preview [data-ln="' + i + '"]');
  if (block) tl.set(block, { visibility: 'visible' }, at);
}

// Types source line i character by character; its preview block follows live.
function typeLine(tl, root, i, at, step = 0.05) {
  const [kind, text] = DOC[i];
  const line = root.querySelector('.source [data-ln="' + i + '"]');
  const block = root.querySelector('.preview [data-ln="' + i + '"]');
  const prefix = { h: text.indexOf(' ') + 1, li: 2 }[kind] || 0;
  const chars = Array.from(text);
  chars.forEach((_, k) => {
    const t = at + k * step;
    const typed = chars.slice(0, k + 1).join('');
    TEXT.set(line, '<span class="typing">' + lineHTML(kind, typed) + '</span>', t);
    if (block && k + 1 > prefix) {
      if (k === prefix) tl.set(block, { visibility: 'visible' }, t);
      TEXT.set(block, esc(typed.slice(prefix)), t);
    }
  });
  const end = at + chars.length * step;
  TEXT.set(line, lineHTML(kind, text), end);
  return end;
}

// Text changes are drawn from the timeline's time instead of tweened: GSAP
// cannot revert a string set whose start value is empty, so a backward seek
// would leave later text on screen.
let TEXT = null;
function textTrack() {
  const tracks = new Map();
  return {
    set(el, html, at) {
      if (!tracks.has(el)) tracks.set(el, { initial: el.innerHTML, events: [] });
      tracks.get(el).events.push([at, html]);
    },
    render(t) {
      for (const [el, track] of tracks) {
        let value = track.initial, best = -Infinity;
        for (const [at, html] of track.events) if (at <= t + 1e-6 && at >= best) { best = at; value = html; }
        if (el.innerHTML !== value) el.innerHTML = value;
      }
    },
  };
}

// A scene builds after the fonts load, so measured offsets are final, then
// registers its one paused timeline.
function scene(id, build) {
  window.__hf = window.__hf || {};
  window.__hf.buildReady = window.__hf.buildReady || {};
  window.__hf.buildReady[id] = document.fonts.ready.then(() => {
    const root = document.getElementById(id).querySelector('.stage');
    const tl = gsap.timeline({ paused: true });
    const text = TEXT = textTrack();
    build({ root, tl, find: (s) => root.querySelector(s), all: (s) => root.querySelectorAll(s) });
    TEXT = null;
    tl.to({}, { duration: tl.duration() + 6, ease: 'none', onUpdate() { text.render(this.time()); } }, 0);
    text.render(0);
    window.__timelines = window.__timelines || {};
    window.__timelines[id] = tl;
  });
}

// Types text into an element, one character per step. Returns the end time.
function typeInto(tl, el, text, at, step = 0.055, wrap = (s) => esc(s)) {
  const chars = Array.from(text);
  chars.forEach((_, i) => TEXT.set(el, wrap(chars.slice(0, i + 1).join('')), at + i * step));
  return at + chars.length * step;
}

const PANE_PAD = 30;
const sourceTop = (root, line) => root.querySelector(".source [data-ln=\"" + line + "\"]").offsetTop - PANE_PAD;
const previewTop = (root, selector) => root.querySelector(".preview " + selector).offsetTop - PANE_PAD;

// Position of an element inside the 1280x720 stage, from offsets (immune to preview scaling).
function stageRect(el, stage) {
  let x = 0, y = 0, n = el;
  while (n && n !== stage) { x += n.offsetLeft; y += n.offsetTop; n = n.offsetParent; }
  return { x, y, w: el.offsetWidth, h: el.offsetHeight };
}

// A cursor click: arrive, press, release.
function click(tl, cursor, x, y, at, travel = 0.7) {
  tl.to(cursor, { x, y, duration: travel, ease: 'power3.out' }, at - travel)
    .to(cursor, { scale: 0.86, duration: 0.06, ease: 'power2.in' }, at)
    .to(cursor, { scale: 1, duration: 0.12, ease: 'power2.out' }, at + 0.06);
}

// A shortcut key cap: appear, press, leave.
function pressKey(tl, key, at, hold = 0.9) {
  tl.fromTo(key, { opacity: 0, filter: 'blur(6px)' }, { opacity: 1, filter: 'blur(0px)', duration: 0.16, ease: 'power3.out' }, at - 0.4)
    .to(key, { scale: 0.92, duration: 0.06 }, at)
    .to(key, { scale: 1, duration: 0.1 }, at + 0.06)
    .to(key, { opacity: 0, filter: 'blur(6px)', duration: 0.18, ease: 'power2.in' }, at + hold);
}
