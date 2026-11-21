const copy = (zh, en) => document.documentElement.lang === "en" ? en : zh;
const icon = (name, label = "") => `<i class="icon" style="--icon:url(assets/icons/${name}.svg)" aria-hidden="true"></i>${label}`;

const sourceBlocks = [
  `<b># ZMark</b><br><br>${copy('星图发布手记', 'Release Notes')}<br><br>${copy('初稿、校对与发布。', 'Draft, review, publish.')}`,
  `<b>## ${copy('版本元数据', 'Version Metadata')}</b><br><br><em>\x60\x60\x60rust</em><br><strong>let</strong> version = <q>"0.2.11"</q>;<br><strong>let</strong> format = <q>"Markdown"</q>;<br><em>\x60\x60\x60</em>`,
  `<b>## ${copy('发布流程', 'Release Workflow')}</b><br><br><em>\x60\x60\x60mermaid</em><br>flowchart LR<br>A[Draft] --&gt; B{Review}<br>B --&gt;|Approved| C[Publish]<br>B --&gt;|Changes| D[Revise]<br>D --&gt; A<br><em>\x60\x60\x60</em>`
];

const previewBlocks = [
  `<h2>ZMark</h2><p class="doc-subtitle">${copy('星图发布手记', 'Release Notes')}</p><p>${copy('初稿、校对与发布。', 'Draft, review, publish.')}</p>`,
  `<h3>${copy('版本元数据', 'Version Metadata')}</h3><div class="code-card"><small>RUST</small><code><strong>let</strong> version = <q>"0.2.11"</q>;<br><strong>let</strong> format = <q>"Markdown"</q>;</code></div>`,
  `<h3>${copy('发布流程', 'Release Workflow')}</h3><div class="mermaid-card"><img src="assets/workflow.svg" alt="${copy('发布流程图', 'Release workflow diagram')}"><span class="expand">${icon('maximize')}</span></div>`
];

function buildWorkbench(target, withSidebar = false) {
  const panel = (blocks) => blocks.map((html) => `<section>${html}</section>`).join("");
  target.innerHTML = `
    <header class="toolbar">
      <span class="traffic"><i></i><i></i><i></i></span>
      <div class="drawer-tools">
        <span class="tool sidebar-tool" title="${copy('侧栏', 'Sidebar')}">${icon('panel-left')}</span>
        ${withSidebar ? `<div class="drawer-tabs" style="visibility:hidden">
          <span class="tool files-tab selected" title="${copy('文件', 'Files')}">${icon('folder')}</span>
          <span class="tool outline-tab" title="${copy('大纲', 'Outline')}">${icon('list')}</span>
          <span class="tool" title="${copy('打开文件或文件夹', 'Open file or folder')}">${icon('folder-plus')}</span>
        </div>` : ""}
      </div>
      <span class="file-label">${copy('星图发布手记.md', 'Release Notes.md')}</span>
      <div class="mode"><span class="selected source-tool">${icon('code')}</span><span class="split-tool">${icon('columns-2')}</span><span>${icon('book-open')}</span></div>
      <div class="tools"><span>${icon('search')}</span><span>${icon('download')}</span><span>${icon('settings')}</span></div>
    </header>
    <div class="body">
      ${withSidebar ? `<aside class="sidebar" data-layout-allow-overflow style="visibility:hidden">
        <div class="file-search">${icon('search', copy('搜索文件...', 'Search files...'))}</div>
        <div class="tree">
          <div class="folder">${icon('folder-open', copy('星图发布', 'Release'))}</div>
          <div class="file active">${icon('file', copy('星图发布手记.md', 'Release Notes.md'))}</div>
          <div class="file">${icon('file', copy('校对清单.md', 'Review Checklist.md'))}</div>
          <div class="file">${icon('file', copy('版本记录.md', 'Changelog.md'))}</div>
        </div>
        <div class="outline"><div class="active">ZMark</div><div class="file">${copy('版本元数据', 'Version Metadata')}</div><div class="file flow-link">${copy('发布流程', 'Release Workflow')}</div></div>
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
  scene.querySelectorAll('[data-en]').forEach((element) => {
    element.textContent = copy(element.textContent, element.dataset.en);
  });
  const timeline = gsap.timeline({ paused: true });
  window.__timelines[id] = timeline;
  return { scene, timeline, find: (selector) => scene.querySelector(selector) };
}
