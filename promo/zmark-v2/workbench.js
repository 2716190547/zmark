const icon = (name, label = "") => `<i class="icon" style="--icon:url(assets/icons/${name}.svg)" aria-hidden="true"></i>${label}`;

const sourceBlocks = [
  '<b># ZMark</b><br><br>星图发布手记<br><br>初稿、校对与发布。',
  '<b>## 版本元数据</b><br><br><em>\x60\x60\x60rust</em><br><strong>let</strong> version = <q>"0.2.11"</q>;<br><strong>let</strong> format = <q>"Markdown"</q>;<br><em>\x60\x60\x60</em>',
  '<b>## 发布流程</b><br><br><em>\x60\x60\x60mermaid</em><br>flowchart LR<br>A[Draft] --&gt; B{Review}<br>B --&gt;|Approved| C[Publish]<br>B --&gt;|Changes| D[Revise]<br>D --&gt; A<br><em>\x60\x60\x60</em>'
];

const previewBlocks = [
  '<h2>ZMark</h2><p class="doc-subtitle">星图发布手记</p><p>初稿、校对与发布。</p>',
  '<h3>版本元数据</h3><div class="code-card"><small>RUST</small><code><strong>let</strong> version = <q>"0.2.11"</q>;<br><strong>let</strong> format = <q>"Markdown"</q>;</code></div>',
  '<h3>发布流程</h3><div class="mermaid-card"><img src="assets/workflow.svg" alt="发布流程图"><span class="expand">' + icon('maximize') + '</span></div>'
];

function buildWorkbench(target, withSidebar = false) {
  const panel = (blocks) => blocks.map((html) => `<section>${html}</section>`).join("");
  target.innerHTML = `
    <header class="toolbar">
      <span class="traffic"><i></i><i></i><i></i></span>
      <div class="drawer-tools">
        <span class="tool sidebar-tool" title="侧栏">${icon('panel-left')}</span>
        ${withSidebar ? `<div class="drawer-tabs" style="visibility:hidden">
          <span class="tool files-tab selected" title="文件">${icon('folder')}</span>
          <span class="tool outline-tab" title="大纲">${icon('list')}</span>
          <span class="tool" title="打开文件或文件夹">${icon('folder-plus')}</span>
        </div>` : ""}
      </div>
      <span class="file-label">星图发布手记.md</span>
      <div class="mode"><span class="selected source-tool">${icon('code')}</span><span class="split-tool">${icon('columns-2')}</span><span>${icon('book-open')}</span></div>
      <div class="tools"><span>${icon('search')}</span><span>${icon('download')}</span><span>${icon('settings')}</span></div>
    </header>
    <div class="body">
      ${withSidebar ? `<aside class="sidebar" data-layout-allow-overflow style="visibility:hidden">
        <div class="file-search">${icon('search', '搜索文件...')}</div>
        <div class="tree">
          <div class="folder">${icon('folder-open', '星图发布')}</div>
          <div class="file active">${icon('file', '星图发布手记.md')}</div>
          <div class="file">${icon('file', '校对清单.md')}</div>
          <div class="file">${icon('file', '版本记录.md')}</div>
        </div>
        <div class="outline"><div class="active">ZMark</div><div class="file">版本元数据</div><div class="file flow-link">发布流程</div></div>
      </aside>` : ""}
      <div class="panes">
        <div class="source"><div class="pane-content" data-layout-allow-overflow>${panel(sourceBlocks)}</div></div>
        <div class="divider"></div>
        <div class="preview"><div class="pane-content" data-layout-allow-overflow>${panel(previewBlocks)}</div></div>
      </div>
    </div>`;
}

function sceneTimeline(id) {
  const host = document.getElementById(id);
  const scene = host.querySelector('.shot') || host;
  const timeline = gsap.timeline({ paused: true });
  window.__timelines[id] = timeline;
  return { scene, timeline, find: (selector) => scene.querySelector(selector) };
}
