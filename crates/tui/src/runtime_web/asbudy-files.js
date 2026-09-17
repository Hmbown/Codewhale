// AsBudy 目录树面板 —— 门卫注入到官方界面侧栏。
// 官方 web 没有「列目录」的 API（/v1/files 404），门卫读文件系统提供目录树 + 文件预览。
(function () {
  var host = document.getElementById('asbudy-files');
  if (!host) return;
  host.hidden = false;   // 骨架默认 hidden（避免脚本没加载时显示「加载中…」），脚本跑起来才露出来
  var pkey = host.getAttribute('data-pkey') || '';
  // 项目预览的地址由门卫给（预览子域名 + 短期票）——
  //   为什么不自拼 /_pv/：客户的内容要住到另一个门牌号下，跟控制台不同源（2026-09-16）。
  var previewUrl = host.getAttribute('data-preview-url') || '';
  var projKind = host.getAttribute('data-kind') || 'proxy';   // proxy = 系统页面 | artifacts = 工作台（看文件内容）
  var body = document.getElementById('asbudy-files-body');        // 「项目文件」卡片内容区
  var mineBody = document.getElementById('asbudy-mine-body');     // 「我的资料」卡片内容区
  var refresh = document.getElementById('asbudy-files-refresh');

  // 样式（动态注入，避免改官方 stylesheet）
  var css = [
    // 以下四块（项目文件 / 我的资料 / 回收站 / 退回）对齐官方侧栏控件语言
    // （2026-09-15 老板：「样式渲染明显跟官方的新建会话、搜索会话、最近会话不一致」）：
    //   ① 左右不缩进 —— 官方的按钮/搜索框都是贴 .rail 的 10px padding（x=10），
    //      我们原来 margin 12px 导致 x=22，左边缘和官方对不齐；
    //   ② 圆角用官方令牌 --radius-control（6px），不是自定的 8px；
    //   ③ 底色用 --well（官方搜索框同款深底）、边框透明（官方控件们就是 transparent，
    //      只靠底色区分）—— 原来我们用 --surface 亮底 + --line 实边框，像另一个系统；
    //   ④ 标题排版抄官方 .rail-section-title（13px / 700 / --text-dim / 字距 0.08em）。
    '#asbudy-files{margin:0;display:flex;flex-direction:column;gap:8px;font-size:13.5px}',
    '.asb-card{border:1px solid transparent;border-radius:var(--radius-control);background:var(--well);overflow:hidden}',
    '.asb-hd{display:flex;align-items:center;gap:6px;padding:8px 10px;font-size:13px;color:var(--text-dim)}',
    '.asb-fold{cursor:pointer;color:var(--text-faint);width:12px;text-align:center;user-select:none;flex:none}',
    '.asb-fold:hover{color:var(--text)}',
    '.asb-title{flex:1;min-width:0;font-weight:700;letter-spacing:0.08em;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}',
    '.asb-title #asb-mine-count{font-weight:400;letter-spacing:0;color:var(--text-faint)}',
    // 「看我的项目」大按钮（2026-09-16 老板：学老系统，做成的东西放左侧栏；
    // 以前只有预览栏右上角一个小按钮，客户找不到）
    '.asb-openproj{display:block;width:100%;padding:10px 12px;font:inherit;font-size:14px;font-weight:600;color:var(--action-contrast);background:var(--live);border:0;border-radius:7px;cursor:pointer;text-align:center}',
    '.asb-openproj:hover{background:var(--live)}',
    '.asb-openproj:active{transform:scale(.985)}',
    '.asb-tools{flex:none;color:var(--text-dim);display:flex;gap:10px;align-items:center}',
    '.asb-bd{padding:0 10px 10px;max-height:30vh;overflow-y:auto}',
    '.asb-card.folded .asb-bd{display:none}',
    '#asbudy-files-upload{cursor:pointer;color:var(--text-dim)}#asbudy-files-upload:hover{color:var(--text)}',
    '#asbudy-files-refresh{cursor:pointer;color:var(--text-faint)}#asbudy-files-refresh:hover{color:var(--text)}',
    '.f-node{display:flex;align-items:center;gap:6px;cursor:pointer;font-size:13.5px;line-height:1.7;white-space:nowrap;color:var(--text);padding:0 4px;border-radius:var(--radius-control)}',
    '.f-node .f-ic{flex:none;color:var(--text-soft);width:13px;text-align:center}',
    '.f-node .f-nm{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis}',
    '.f-node .f-sz{flex:none;color:var(--text-faint);font-size:12.5px}',
    // 固定文件夹的中文小注（老板 2026-09-16：客户看不懂 data / public）
    '.f-node .f-note{flex:none;color:var(--text-faint);font-size:12px;border:1px solid var(--line);border-radius:3px;padding:0 3px;line-height:1.5}',
    '.f-node .f-tag{flex:none;color:var(--text-faint);font-size:12.5px;border:1px solid var(--line);border-radius:3px;padding:0 3px;line-height:1.5}',
    '.f-node .f-del{flex:none;color:var(--text-soft);padding:0 3px}',
    '.f-node .f-del:hover{color:var(--danger)}',
    '.f-node:hover{background:var(--hover)}',
    '.f-node.dir{color:var(--text-soft)}',
    '.f-kids{margin-left:12px;border-left:1px solid var(--line);padding-left:6px}',
    '.f-empty{font-size:13.5px;color:var(--text-faint);padding:0 4px}',
    '#asbudy-undo{margin:0;border:1px solid transparent;border-radius:var(--radius-control);padding:8px 10px;background:var(--well);font-size:13.5px}',
    '#asbudy-undo-body{max-height:22vh;overflow-y:auto}',
    '#asbudy-arts{margin:8px 12px 0;border:1px solid var(--line);border-radius:8px;padding:8px;background:var(--surface);font-size:13.5px}',
    '.a-head{font-size:13.5px;color:var(--text);font-weight:600;display:flex;justify-content:space-between;align-items:center;margin-bottom:6px}',
    '#asbudy-arts-toggle{cursor:pointer;color:var(--text-faint);font-weight:400}',
    '.a-list{max-height:22vh;overflow-y:auto}',
    '.a-item{display:flex;align-items:center;gap:8px;padding:5px 6px;border-radius:4px;cursor:pointer;font-size:13.5px;color:var(--text)}',
    '.a-item:hover{background:var(--hover)}',
    '.a-ic{flex:none;color:var(--text-soft)}',
    '.a-nm{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}',
    '.a-meta{flex:none;color:var(--text-faint);font-size:12.5px}',
    '.a-del{flex:none;color:var(--text-soft);padding:0 3px}',
    '.a-del:hover{color:var(--danger)}',
    '.u-item{font-size:13.5px;color:var(--text);padding:4px 6px;border-radius:var(--radius-control);cursor:pointer;line-height:1.5}',
    '.u-item:hover{background:var(--hover)}',
    '.u-time{color:var(--text-faint);font-size:12.5px}',
    // 回收站卡片（老板 2026-09-15：从「我的资料」卡里搬出来，排在「最近会话」下面）——
    // 现在是侧栏的直接子元素，边距得自己带（以前靠 #asbudy-files 容器的边距）
    '#asbudy-recycle{margin:0}',
    // 「退回」面板头部：折叠箭头 + 标题（官方 .rail-section-title 的排版）+ （靠右的）刷新；折叠时只留头部
    '#asbudy-undo .f-head{display:flex;align-items:center;gap:6px;font-size:13px;font-weight:700;letter-spacing:0.08em;color:var(--text-dim)}',
    '#asbudy-undo .f-head #asbudy-undo-refresh{margin-left:auto;cursor:pointer;color:var(--text-faint)}',
    '#asbudy-undo.folded > #asbudy-undo-body{display:none}',
    // 手机（窄屏）：侧栏要一屏装得下。2026-09-15 实测 iPhone 视口（390×844）下
    // 侧栏内容总高 962px > 一屏 844 —— 会话列表下半截和底部「已连接」状态整块在屏幕外。
    // 卡片的内容区各自限高（超出在卡片内部滚），配合 styles.css 里窄屏 .rail 可纵向滚动兜底。
    '@media (max-width:800px){',
    '.asb-bd{max-height:18vh}',
    '#asbudy-undo-body{max-height:14vh}',
    '.a-list{max-height:14vh}',
    '}'
  ].join('\n');
  var st = document.createElement('style');
  st.textContent = css;
  document.head.appendChild(st);

  // ── 可折叠的块：折叠状态记在 localStorage（下次进来保持）──
  // 2026-09-15 老板要求：回收站 / 退回**默认折叠**（列表长、常占地方），
  // 项目文件 / 我的资料仍默认展开。默认值只在「本地没记录」时生效 ——
  // 老板自己点开过就按他的来。
  function foldKey(which) { return 'asbudy.fold.' + which; }
  var FOLD_DEFAULT = { proj: false, mine: false, recycle: true, undo: true };
  var FOLD_CARD = { proj: 'asb-card-proj', mine: 'asb-card-mine', recycle: 'asbudy-recycle', undo: 'asbudy-undo' };
  function isFolded(which) {
    var dflt = !!FOLD_DEFAULT[which];
    try { var v = localStorage.getItem(foldKey(which)); return v === null ? dflt : v === '1'; } catch (e) { return dflt; }
  }
  function setFold(which, folded) {
    var card = document.getElementById(FOLD_CARD[which] || ('asb-card-' + which));
    var btn = document.getElementById('asb-fold-' + which);
    if (card) {
      // .asb-card 系的（项目文件 / 我的资料 / 回收站）整串重设，保持原逻辑；
      // 「退回」面板有自己的 id 样式，只 toggle folded，不碰它其他 class
      if (card.classList.contains('asb-card')) card.className = 'asb-card' + (folded ? ' folded' : '');
      else card.classList.toggle('folded', folded);
    }
    if (btn) btn.textContent = folded ? '\u25b8' : '\u25be';
    try { localStorage.setItem(foldKey(which), folded ? '1' : '0'); } catch (e) {}
  }

  /* ── 做好的东西（成品列表）── 所有项目都可能有：AI 干出来的活落在这里 ── */
  // 【2026-09-14 老板定「C 方案」】不再单独显示「做好的东西」清单 ——
  // 同一批成品文件在目录树里本来就有，两处显示 = 同一个文件出现两遍，看着乱。
  // 成品文件改成在目录树里标「可下载」。这个容器保留但不挂进页面
  // （loadArts 仍可用；将来若要恢复，把 insertBefore 那行加回来即可）。
  var artsEl = document.createElement('div');
  artsEl.id = 'asbudy-arts';
  artsEl.hidden = true;

  function aEsc(s) {
    return String(s == null ? '' : s).replace(/[&<>"']/g, function (c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c];
    });
  }
  function aSize(n) {
    if (n < 1024) return n + ' B';
    if (n < 1048576) return Math.round(n / 1024) + ' KB';
    return (n / 1048576).toFixed(1) + ' MB';
  }
  function aTime(ts) {
    var t = new Date(ts);
    function p(n) { return String(n).padStart(2, '0'); }
    return p(t.getMonth() + 1) + '-' + p(t.getDate()) + ' ' + p(t.getHours()) + ':' + p(t.getMinutes());
  }
  // 成品后缀（跟后端 server.js 的 ARTIFACT_EXT 对齐）—— 目录树里给这些文件标「可下载」
  var ART_EXT = ['pptx','ppt','xlsx','xls','docx','doc','pdf','csv','md','txt','png','jpg','jpeg','svg','zip','json'];
  function isArtifact(name) {
    var e = (name.split('.').pop() || '').toLowerCase();
    return ART_EXT.indexOf(e) >= 0;
  }
  function aIcon(name) {
    var e = (name.split('.').pop() || '').toLowerCase();
    if (['xlsx', 'xls', 'csv'].indexOf(e) >= 0) return '▦';
    if (['pptx', 'ppt'].indexOf(e) >= 0) return '▣';
    if (['docx', 'doc', 'pdf'].indexOf(e) >= 0) return '▤';
    if (['png', 'jpg', 'jpeg', 'svg'].indexOf(e) >= 0) return '◨';
    return '●';
  }
  function loadArts() {
    fetch('/_gate/artifacts?project=' + encodeURIComponent(pkey), { credentials: 'same-origin' })
      .then(function (r) { return r.json(); })
      .then(function (d) {
        var list = (d && d.files) || [];
        if (!list.length) { artsEl.hidden = true; return; }
        artsEl.innerHTML = '';
        artsEl.hidden = false;
        var h = document.createElement('div');
        h.className = 'a-head';
        h.innerHTML = '<span>产出文件 ' + list.length + ' 个</span><span id="asbudy-arts-toggle">收起</span>';
        var wrap = document.createElement('div');
        wrap.className = 'a-list';
        list.forEach(function (a) {
          var it = document.createElement('div');
          it.className = 'a-item';
          it.title = a.path;
          it.innerHTML = '<span class="a-ic">' + aIcon(a.name) + '</span>' +
            '<span class="a-nm">' + aEsc(a.name) + '</span>' +
            '<span class="a-meta">' + aSize(a.size) + ' · ' + aTime(a.mtime) + '</span>' +
            '<span class="a-del" title="删掉">×</span>';
          it.onclick = function (ev) {
            if (ev.target && ev.target.className === 'a-del') return;
            openArtifact(a);
          };
          var dbtn = it.querySelector('.a-del');
          if (dbtn) {
            dbtn.onclick = function (ev) {
              ev.stopPropagation();
              if (!confirm('删掉「' + a.name + '」？删了拿不回来。')) return;
              fetch('/_gate/artifact?project=' + encodeURIComponent(pkey) + '&path=' + encodeURIComponent(a.path), { method: 'DELETE', credentials: 'same-origin' })
                .then(function (r) {
                  if (!r.ok) { alert('删不掉'); return; }
                  // loadArts();   // C 方案：成品清单不再单独显示（成品文件在目录树里标「可下载」）
                });
            };
          }
          wrap.appendChild(it);
        });
        artsEl.appendChild(h);
        artsEl.appendChild(wrap);
        h.querySelector('#asbudy-arts-toggle').onclick = function (e) {
          e.stopPropagation();
          var hid = wrap.style.display === 'none';
          wrap.style.display = hid ? '' : 'none';
          e.target.textContent = hid ? '收起' : '展开';
        };
      })
      .catch(function () { artsEl.hidden = true; });
  }
  function openArtifact(a) {
    try { document.dispatchEvent(new CustomEvent('asbudy-file-picked', { detail: { path: a.path, name: a.name } })); } catch (e0) {}
    fetch('/_gate/artifact/view?project=' + encodeURIComponent(pkey) + '&path=' + encodeURIComponent(a.path), { credentials: 'same-origin' })
      .then(function (r) { return r.ok ? r.json() : null; })
      .then(function (d) {
        if (!d) { alert('读不到这个文件'); return; }
        showData(a.name, d);      // 跟另外两条链同一套渲染（2026-09-16）
      })
      .catch(function () { alert('读不到这个文件'); });
  }

  // 卡片一：项目里的文件（只读目录树）
  var projSig = null;   // 目录树内容签名：没变化就不重绘（不闪、不丢展开状态）
  var projDirty = false; // 自上次刷新预览以来，项目文件变过没有 —— **累积**标志，不只“这一次”

  async function loadProj(force) {
    if (!body) return;
    try {
      var r = await fetch('/_gate/projfiles?project=' + encodeURIComponent(pkey), { credentials: 'same-origin' });
      if (!r.ok) throw 0;
      var d = await r.json();
      var sig = JSON.stringify(d.files || []);
      // 签名里含每个文件的 mtime/size —— 变了 = 项目里真有东西被改过。
      // ⚠️ 变化要**累积**，不能只看“这一次”：一轮里 AI 往往在干到一半就写了文件
      //   （item.completed 就把签名更新掉了），到这一轮真正结束（turn.completed）再看就“没变化”了
      //   —— 结果是永远不刷。2026-09-16 真实链路实测踩到这个，换成累积标志。
      if (projSig !== null && sig !== projSig) projDirty = true;
      if (!force && sig === projSig) return;   // 内容没变 → 什么都不做（点「刷新」走 force）
      projSig = sig;
      body.innerHTML = '';
      if (!d.files || !d.files.length) { body.innerHTML = '<span class="f-empty">（空目录）</span>'; return; }
      for (var i = 0; i < d.files.length; i++) body.appendChild(render(d.files[i], false, true));
    } catch (e) { if (projSig === null) body.innerHTML = '<span class="f-empty">加载失败</span>'; }
  }

  // 卡片二：我的资料（文件池，跨项目）
  async function loadMine() {
    if (!mineBody) return;
    try {
      var r = await fetch('/_gate/files', { credentials: 'same-origin' });
      if (!r.ok) throw 0;
      var d = await r.json();
      var files = (d && d.files) || [];
      var cnt = document.getElementById('asb-mine-count');
      var u = d.usage || null;
      var quotaTxt = (u && !u.unlimited) ? ' · 已用 ' + fmtMb(u.totalMb) + ' / ' + fmtMb(u.quotaMb) : '';
      if (cnt) cnt.textContent = files.length ? ('（' + files.length + ' 份' + quotaTxt + '）') : '';
      mineBody.innerHTML = '';
      if (!files.length) { mineBody.innerHTML = '<span class="f-empty">暂无资料</span>'; return; }
      for (var i = 0; i < files.length; i++) mineBody.appendChild(render(files[i], true));
    } catch (e) { mineBody.innerHTML = '<span class="f-empty">加载失败</span>'; }
  }
  // ── 回收站（2026-09-15 · 老板要「客户看得见、能自己捞回来」）──
  // 删掉的东西挪在这儿：资料池的来自「我的资料」，产出物的来自某个项目。
  // 能还原（挪回原位）、能彻底删（真删）。空的时候整块隐藏，不打扰。
  var recycleBox = null;

  // ── 「看我的项目」入口（2026-09-16 老板要求：做成的东西放左侧栏）──
  // 摆在整个侧栏的**最上面**，一个大大的主按钮 —— 客户最常要的就是「看我那个系统」。
  // 为什么必须显眼：以前入口只在预览栏右上角（一个小灰按钮），老板连问了两次
  //「她从哪点击查看她做的系统」。
  function ensureProjectBox() {
    if (projKind !== 'proxy') return;        // 只有「有系统的项目」才有这个入口（工作台不适用）
    var hostEl = document.getElementById('asbudy-files');
    if (!hostEl || document.getElementById('asbudy-openproj')) return;
    var box = document.createElement('div');
    box.className = 'asb-card';
    box.id = 'asbudy-openproj';
    box.innerHTML = '<div class="asb-bd"><button class="asb-openproj" id="asb-openproj" type="button">\u25b6 看我的项目</button></div>';
    hostEl.insertBefore(box, hostEl.firstChild);
  }
  function ensureRecycleBox() {
    if (recycleBox) return recycleBox;
    var mineCard = mineBody ? mineBody.closest('.asb-card') : null;
    if (!mineCard || !mineCard.parentNode) return null;
    recycleBox = document.createElement('div');
    recycleBox.className = 'asb-card';
    recycleBox.id = 'asbudy-recycle';
    recycleBox.hidden = true;
    recycleBox.innerHTML = '<div class="asb-hd"><span class="asb-fold" id="asb-fold-recycle" title="折叠 / 展开">\u25b8</span>'
      + '<span class="asb-title">回收站<span id="asb-bin-count"></span></span>'
      + '<span class="asb-tools"><span id="asbudy-recycle-refresh" style="cursor:pointer">刷新</span></span></div>'
      + '<div class="asb-bd" id="asbudy-recycle-body"></div>';
    // 位置（老板 2026-09-15）：回收站排在「最近会话」下面、「退回」上面 ——
    // 以前塞在「我的资料」卡片内部，反而把会话列表挤到了最下面
    var undoAnchor = document.getElementById('asbudy-undo');
    if (undoAnchor && undoAnchor.parentNode) undoAnchor.parentNode.insertBefore(recycleBox, undoAnchor);
    else mineCard.parentNode.insertBefore(recycleBox, mineCard.nextSibling);
    setFold('recycle', isFolded('recycle'));   // 默认折叠（老板 2026-09-15）
    var rf = recycleBox.querySelector('#asbudy-recycle-refresh');
    if (rf) rf.onclick = function () { loadRecycle(); };
    return recycleBox;
  }
  function fmtMb(v) {   // 小单位用 M，上 G 就用 G —— 别让客户读三位数的 MB
    v = Number(v) || 0;
    return v >= 1024 ? (Math.round(v / 1024 * 10) / 10) + 'G' : v + 'M';
  }
  function binTime(ts) {
    if (!ts) return '';
    var d = new Date(ts);
    function p(n) { return String(n).padStart(2, '0'); }
    return (d.getMonth() + 1) + '月' + d.getDate() + '日 ' + p(d.getHours()) + ':' + p(d.getMinutes());
  }
  async function loadRecycle() {
    var box = ensureRecycleBox();
    if (!box) return;
    var binBody = box.querySelector('#asbudy-recycle-body');
    try {
      var r = await fetch('/_gate/recycle', { credentials: 'same-origin' });
      if (!r.ok) throw 0;
      var d = await r.json();
      var items = (d && d.items) || [];
      var prjs = (d && d.projects) || [];        // 已删除的项目（接口一直在返回，之前没画）
      var cntEl = box.querySelector('#asb-bin-count');
      var total = items.length + prjs.length;
      if (cntEl) cntEl.textContent = total ? '（' + total + '）' : '';
      // 项目和文件**都空**才整块隐藏。原来只看文件，于是项目回收站里躺着东西、这块却藏着。
      if (!total) { box.hidden = true; binBody.innerHTML = ''; return; }
      box.hidden = false;
      binBody.innerHTML = '';

      // ── 块一：已删除的项目（跟文件分开摆 —— 两码事，别混一起）──
      prjs.forEach(function (p) {
        var row = document.createElement('div');
        row.className = 'f-node';
        row.title = p.deletedBy ? ('由 ' + p.deletedBy + ' 删除') : '已删除';
        row.innerHTML = '<span class="f-ic">▣</span>'
          + '<span class="f-nm">' + aEsc(p.name) + '</span>'
          + '<span class="f-sz">' + (p.sizeMb != null ? p.sizeMb + 'M · ' : '') + binTime(p.deletedAt) + '</span>'
          + '<span class="f-tag" data-act="prestore" style="cursor:pointer" title="让它回到项目列表">还原</span>'
          + '<span class="f-del" data-act="ppurge" title="彻底删除（不可恢复）">×</span>';
        row.querySelector('[data-act="prestore"]').onclick = function (ev) {
          ev.stopPropagation();
          fetch('/_gate/recycle/projects/restore', { method: 'POST', credentials: 'same-origin',
            headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ key: p.key }) })
            .then(function (rr) {
              if (rr.ok) { loadRecycle(); return; }
              return rr.json().then(function (j) { alert((j && j.error) || '还原失败'); })
                .catch(function () { alert('还原失败'); });
            });
        };
        row.querySelector('[data-act="ppurge"]').onclick = function (ev) {
          ev.stopPropagation();
          // 彻底删要手打项目名 —— 9-16 误删过一次，这层确认不能省
          var typed = prompt('彻底删除「' + p.name + '」？\n\n它的文件、资料与对话记录会一并删掉，之后无法恢复。\n想留着以后再用，请选「暂停」而不是删除。\n\n请输入项目名以确认：', '');
          if (typed !== p.name) { if (typed !== null) alert('名字不对，没有删除。'); return; }
          fetch('/_gate/recycle/projects', { method: 'DELETE', credentials: 'same-origin',
            headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ key: p.key }) })
            .then(function (rr) {
              if (rr.ok) { loadRecycle(); return; }
              return rr.json().then(function (j) { alert((j && j.error) || '删不掉'); })
                .catch(function () { alert('删不掉'); });
            });
        };
        binBody.appendChild(row);
      });

      items.forEach(function (it) {
        var row = document.createElement('div');
        row.className = 'f-node';
        row.title = it.from ? ('原位置：' + it.from) : '';
        row.innerHTML = '<span class="f-ic">↩</span>'
          + '<span class="f-nm">' + aEsc(it.name) + '</span>'
          + '<span class="f-sz">' + aEsc(it.source || '') + ' · ' + binTime(it.at) + '</span>'
          + '<span class="f-tag" data-act="restore" style="cursor:pointer" title="放回原来位置">还原</span>'
          + '<span class="f-del" data-act="purge" title="彻底删除（不可恢复）">×</span>';
        row.querySelector('[data-act="restore"]').onclick = function (ev) {
          ev.stopPropagation();
          fetch('/_gate/recycle/restore', { method: 'POST', credentials: 'same-origin',
            headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ id: it.id }) })
            .then(function (rr) {
              if (rr.ok) { loadRecycle(); loadMine(); loadProj(); return; }
              return rr.json().then(function (j) { alert((j && j.error) || '还原失败'); })
                .catch(function () { alert('还原失败'); });
            });
        };
        row.querySelector('[data-act="purge"]').onclick = function (ev) {
          ev.stopPropagation();
          if (!confirm('彻底删除「' + it.name + '」？此操作不可恢复。')) return;
          fetch('/_gate/recycle', { method: 'DELETE', credentials: 'same-origin',
            headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ id: it.id }) })
            .then(function (rr) {
              if (rr.ok) { loadRecycle(); loadMine(); return; }
              return rr.json().then(function (j) { alert((j && j.error) || '删不掉'); })
                .catch(function () { alert('删不掉'); });
            });
        };
        binBody.appendChild(row);
      });
    } catch (e) { box.hidden = true; }
  }

  // 项目里几个**固定文件夹**的中文小注（2026-09-16 老板：「data、public 这些是什么？」）——
  //   客户不懂技术，光看 `public` / `data` 一脸问号。只标**顶层**（递归下去不标，否则满屏小字），
  //   表里没有的目录不硬凑（AI 新建的目录它自己会解释）。
  var DIR_NOTES = {
    '产出': '成品',
    'public': '网页',
    'data': '数据',
    'src': '代码',
    'lib': '代码'
  };
  function render(f, isMine, top) {
    var w = document.createElement('div');
    if (f.isDir) {
      var h = document.createElement('div'); h.className = 'f-node dir';
      var note = (top && DIR_NOTES[f.name])
        ? '<span class="f-note">' + DIR_NOTES[f.name] + '</span>' : '';
      h.innerHTML = '<span class="f-ic">▸</span><span class="f-nm">' + aEsc(f.name) + '</span>' + note +
        (isMine ? '<span class="f-del" title="删掉整个文件夹">×</span>' : '');
      var kids = document.createElement('div'); kids.className = 'f-kids'; kids.style.display = 'none';
      h.onclick = function (ev) {
        if (ev.target && ev.target.className === 'f-del') return;
        var open = kids.style.display !== 'none';
        kids.style.display = open ? 'none' : 'block';
        h.querySelector('.f-ic').textContent = open ? '▸' : '▾';
      };
      var delBtn = h.querySelector('.f-del');
      if (delBtn) {
        delBtn.onclick = function (ev) {
          ev.stopPropagation();
          if (!confirm('删除文件夹「' + f.name + '」？（将移入回收站，可由 AI 协助还原）')) return;
          fetch('/_gate/file?name=' + encodeURIComponent(f.path), { method: 'DELETE', credentials: 'same-origin' })
            .then(function (r) {
              if (r.ok) { loadMine(); loadRecycle(); return; }
              return r.json().then(function (d) { alert((d && d.error) || '删不掉'); })
                .catch(function () { alert('删不掉'); });
            });
        };
      }
      for (var i = 0; i < (f.children || []).length; i++) kids.appendChild(render(f.children[i], isMine, false));
      // 空文件夹：展开后给一行「（空文件夹）」，否则点了箭头没有任何视觉反馈，用户会以为折叠坏了
      if (!(f.children || []).length) {
        var emp = document.createElement('div');
        emp.className = 'f-empty';
        emp.textContent = '（空文件夹）';
        kids.appendChild(emp);
      }
      w.appendChild(h); w.appendChild(kids);
    } else {
      var n = document.createElement('div'); n.className = 'f-node'; n.title = isMine ? (f.abs || f.path) : f.path;
      var ic = aIcon(f.name);
      var sz = f.size ? aSize(f.size) : '';
      n.innerHTML = '<span class="f-ic">' + ic + '</span>' +
        '<span class="f-nm">' + aEsc(f.name) + '</span>' +
        (isArtifact(f.name) ? '<span class="f-tag">可下载</span>' : '') +
        (sz ? '<span class="f-sz">' + sz + '</span>' : '') +
        '<span class="f-del" title="删掉这个文件">×</span>';
      n.onclick = function () { onPick(f, isMine); };
      // 删除：文件池走 /_gate/file，项目文件走 /_gate/artifact（两个后端各管各的）
      var delF = n.querySelector('.f-del');
      if (delF) delF.onclick = function (ev) {
        ev.stopPropagation();
        if (!confirm(isMine
          ? '删掉「' + f.name + '」？（将移入回收站，可由 AI 协助还原）'
          : '删掉「' + f.name + '」？（删错了可以点「退回」，但会连之后的改动一起回退）')) return;
        var url = isMine
          ? '/_gate/file?name=' + encodeURIComponent(f.path)
          : '/_gate/artifact?project=' + encodeURIComponent(pkey) + '&path=' + encodeURIComponent(f.path);
        fetch(url, { method: 'DELETE', credentials: 'same-origin' })
          .then(function (r) {
            if (r.ok) { isMine ? loadMine() : loadProj(); loadRecycle(); return; }
            // 把后端说清楚的原因透出来（比如「文件在隔离区，可以让 AI 帮你删」），别只说「删不掉」
            return r.json().then(function (d) { alert((d && d.error) || '删不掉'); })
              .catch(function () { alert('删不掉'); });
          });
      };
      w.appendChild(n);
    }
    return w;
  }

  // 点一个文件：不管是哪个 tab，都「带上」（告诉 AI 用这份）+ 弹预览
  function onPick(f, isMine) {
    // 文件池在项目目录外 → 带绝对路径（AI 能读任意路径）；项目文件用相对路径即可
    var p = isMine ? (f.abs || f.path) : f.path;
    lastPicked = { f: f, isMine: isMine };   // 右栏现在看的是它 —— 自动刷新时重拉的就是这份
    try { document.dispatchEvent(new CustomEvent('asbudy-file-picked', { detail: { path: p, name: f.name } })); } catch (e0) {}
    openPreviewFor(f, isMine);
  }

  async function openPreviewFor(f, isMine) {
    try {
      var url = isMine
        ? ('/_gate/preview?name=' + encodeURIComponent(f.path))
        : ('/_gate/artifact/view?project=' + encodeURIComponent(pkey) + '&path=' + encodeURIComponent(f.path));
      var r = await fetch(url, { credentials: 'same-origin' });
      if (!r.ok) { alert('读不到这个文件'); return; }
      var d = await r.json();
      showData(f.name, d);
    } catch (e) { alert('读不到这个文件'); }
  }

  async function openFile(fp, name) {
    // 告诉「@文件」逻辑：用户点了这份文件（路径会被带到对话里）
    try { document.dispatchEvent(new CustomEvent('asbudy-file-picked', { detail: { path: fp, name: name } })); } catch (e0) {}
    try {
      var r = await fetch('/_gate/artifact/view?project=' + encodeURIComponent(pkey) + '&path=' + encodeURIComponent(fp), { credentials: 'same-origin' });
      if (!r.ok) { alert('读不到这个文件'); return; }
      var d = await r.json();
      showData(name, d);
    } catch (e) { alert('读不到这个文件'); }
  }

  // ── 预览分派：所有链（产出物 / 我的资料 / 目录树里的成品）只走这里 ──
  // 后端两条链现在返回同一套 kind，所以渲染只写一份 —— 以前三处各写一遍 if，改一个格式要改三处。
  function showData(name, d) {
    var k = (d && d.kind) || '';
    if (k === 'web') return showPanel(name, webBody(d), d.download);   // 网页：默认渲染出页面，可切「看代码」
    if (k === 'office' || k === 'html' || k === 'markdown') return showPanel(name, htmlBody(d.html || ''), d.download);
    if (k === 'table') return showPanel(name, tableBody(d), d.download);        // 后端不再发这个 kind 了，留着兼容
    if (k === 'text' || k === 'svg' || k === 'code') return showPanel(name, textBody(d.text || d.content || ''), d.download);
    if (k === 'image') return showPanel(name, imgBody(d.url), d.download);
    if (k === 'pdf') return showPanel(name, pdfBody(d.url), d.download);
    if (k === 'unsupported') return showPanel(name, textBody(d.note || '此格式暂不支持预览，请下载查看'), d.download);
    return showPanel(name, textBody((d && d.note) || '此格式暂不支持预览，请下载查看'), d.download);   // 没得预览也要说清楚，别给空面板
  }

  // 提取出来的内容（docx 表格 / xlsx / pptx / csv / markdown）得看得像个正经文档 ——
  // 后端给的是没样式的 HTML 片段，这些规则只作用于预览里的 .pv-html，不碰官方界面。只注一次。
  function ensurePreviewCss() {
    if (document.getElementById('asbudy-preview-css')) return;
    var s = document.createElement('style');
    s.id = 'asbudy-preview-css';
    s.textContent = [
      '.pv-html h1{font-size:20px;margin:16px 0 8px;line-height:1.3}',
      '.pv-html h2{font-size:17px;margin:14px 0 6px;line-height:1.3}',
      '.pv-html h3{font-size:15px;margin:12px 0 6px;line-height:1.4}',
      '.pv-html h4,.pv-html h5,.pv-html h6{font-size:14px;margin:10px 0 4px}',
      '.pv-html p{margin:6px 0}',
      '.pv-html ul,.pv-html ol{margin:6px 0;padding-left:22px}',
      '.pv-html li{margin:2px 0}',
      '.pv-html a{color:var(--action)}',
      '.pv-html code{background:var(--surface-raised);border-radius:3px;padding:1px 4px;font-size:12.5px;font-family:ui-monospace,monospace}',
      '.pv-html pre{background:var(--bg);border:1px solid var(--line);border-radius:6px;padding:10px 12px;margin:8px 0;overflow:auto}',
      '.pv-html pre code{background:none;padding:0}',
      '.pv-html table{border-collapse:collapse;margin:10px 0;font-size:13px;max-width:100%}',
      '.pv-html th,.pv-html td{border:1px solid var(--line);padding:4px 8px;text-align:left;vertical-align:top}',
      '.pv-html th{background:var(--surface-raised);font-weight:600}',
      '.pv-html blockquote{border-left:3px solid var(--line);margin:8px 0;padding:2px 12px;color:var(--text-dim)}',
      '.pv-html hr{border:0;border-top:1px solid var(--line);margin:14px 0}',
      '.pv-html img{max-width:100%}',
      // extract.py 里的提示文字（.x-mut = 说明/截断提示；.slide = pptx 的一页）
      '.pv-html .x-mut{color:var(--text-dim);font-size:13px}',
      '.pv-html .slide{border:1px solid var(--line);border-radius:8px;padding:10px 14px;margin:10px 0}',
      '.pv-html .slide-no{color:var(--text-dim);font-size:12px;margin-bottom:6px}'
    ].join('\n');
    document.head.appendChild(s);
  }

  function textBody(text) {
    var pre = document.createElement('pre');
    pre.style.cssText = 'margin:0;padding:16px;overflow:auto;font:12px/1.5 ui-monospace,monospace;color:var(--text);white-space:pre-wrap;word-break:break-all';
    pre.textContent = text;
    return pre;
  }
  function htmlBody(html) {
    ensurePreviewCss();
    var div = document.createElement('div');
    div.className = 'pv-html';
    div.style.cssText = 'margin:0;padding:16px;overflow:auto;color:var(--text);font-size:14px;line-height:1.6';
    // 内容是后端 extract.py 生成的，里面所有文字都先 html.escape 过（markdown 也一样），
    // 所以客户文件里就算写了 <script> 也只会原样显示成文字
    div.innerHTML = html;
    return div;
  }
  // 网页（.html）——客户要的是「看到做出来的页面」，不是读源码。
  // 默认直接渲染（iframe 指 /_gate/raw 的内嵌地址）；想看代码的人切一下就行。
  // 2026-09-16 老板：对话框里做了个网页，预览里看到的却是一堆 HTML 代码。
  function webBody(d) {
    var wrap = document.createElement('div');
    wrap.style.cssText = 'display:flex;flex-direction:column;height:100%;min-height:0';
    var bar = document.createElement('div');
    bar.style.cssText = 'display:flex;gap:6px;padding:7px 12px;border-bottom:1px solid var(--line);flex:0 0 auto';
    var stage = document.createElement('div');
    stage.style.cssText = 'flex:1;min-height:0;overflow:auto';
    function mkBtn(label) {
      var b = document.createElement('button');
      b.type = 'button'; b.textContent = label;
      b.style.cssText = 'border:1px solid var(--line);background:transparent;border-radius:6px;padding:3px 10px;font-size:13px;cursor:pointer;color:var(--text)';
      return b;
    }
    var bWeb = mkBtn('看网页'), bCode = mkBtn('看代码');
    function paint(web) {
      bWeb.style.color = web ? 'var(--text)' : 'var(--text-dim)';
      bCode.style.color = web ? 'var(--text-dim)' : 'var(--text)';
      bWeb.style.borderColor = web ? 'var(--action)' : 'var(--line)';
      bCode.style.borderColor = web ? 'var(--line)' : 'var(--action)';
      stage.innerHTML = '';
      if (web) {
        var f = document.createElement('iframe');
        f.src = d.url; f.title = '网页预览';
        f.style.cssText = 'width:100%;height:100%;min-height:70vh;border:0;background:var(--action-contrast)';
        stage.appendChild(f);
      } else {
        stage.appendChild(textBody(d.text || ''));
      }
    }
    bWeb.onclick = function () { paint(true); };
    bCode.onclick = function () { paint(false); };
    paint(true);
    bar.appendChild(bWeb); bar.appendChild(bCode);
    wrap.appendChild(bar); wrap.appendChild(stage);
    return wrap;
  }
  function imgBody(url) {
    if (!url) return textBody('这个文件没有能内嵌预览的地址。');
    var wrap = document.createElement('div');
    wrap.style.cssText = 'display:flex;align-items:center;justify-content:center;min-height:160px';
    var img = document.createElement('img');
    img.src = url;
    img.alt = '';
    img.style.cssText = 'max-width:100%;max-height:70vh;object-fit:contain';
    // 图坏了/格式不认时说一句人话，不然用户只看到一个裂图标
    img.onerror = function () {
      wrap.innerHTML = '';
      var p = document.createElement('div');
      p.style.cssText = 'color:var(--text-dim);font-size:13px';
      p.textContent = '图片无法显示（文件可能损坏或格式不受支持）。请点击右上角「下载」查看。';
      wrap.appendChild(p);
    };
    wrap.appendChild(img);
    return wrap;
  }
  function pdfBody(url) {
    // 浏览器不带 PDF 阅读器时嵌进去只会是空白（navigator.pdfViewerEnabled=false）——
    // 那就别装样子，直接说清楚让他下载
    if (!url || navigator.pdfViewerEnabled === false) {
      return textBody('当前浏览器不支持内嵌 PDF 预览，请点击右上角「下载」查看。');
    }
    var f = document.createElement('iframe');
    f.src = url; f.title = 'PDF 预览';
    f.style.cssText = 'width:100%;height:70vh;border:0;background:var(--action-contrast)';
    return f;
  }
  function tableBody(d) {
    var wrap = document.createElement('div');
    wrap.style.cssText = 'margin:0;padding:16px;overflow:auto;color:var(--text);font-size:14px;line-height:1.6';
    if (d.sheet) {
      var sh = document.createElement('div');
      sh.style.cssText = 'margin-bottom:8px;color:var(--text-dim);font-size:13px';
      sh.textContent = '工作表：' + d.sheet;
      wrap.appendChild(sh);
    }
    var rows = d.rows || [];
    if (!rows.length) { wrap.textContent = '（这个表格是空的）'; return wrap; }
    var maxCols = 0;
    for (var i = 0; i < rows.length; i++) if (rows[i].length > maxCols) maxCols = rows[i].length;
    var tb = document.createElement('table');
    tb.style.cssText = 'border-collapse:collapse;width:100%;font-size:13px';
    for (var r = 0; r < rows.length; r++) {
      var tr = document.createElement('tr');
      for (var c = 0; c < maxCols; c++) {
        var cell = document.createElement(r === 0 ? 'th' : 'td');
        cell.style.cssText = 'border:1px solid var(--line);padding:4px 8px;text-align:left;white-space:nowrap;' + (r === 0 ? 'background:var(--surface-raised);font-weight:600' : '');
        cell.textContent = (rows[r][c] == null ? '' : rows[r][c]);
        tr.appendChild(cell);
      }
      tb.appendChild(tr);
    }
    wrap.appendChild(tb);
    return wrap;
  }

  function showPanel(name, bodyEl, download) {
    // 有右栏就显示在右栏（工作台看文件、客户项目也能点文件在右栏看，不弹浮层）
    var fileBox = document.getElementById('preview-file');
    var sh = shellEl();
    if (fileBox && sh) {
      showPreview();
      // 客户项目：右栏默认跑系统页面，点文件就切到「文件预览」（隐藏 iframe，给「看系统」入口切回）
      if (projKind === 'proxy') {
        var fr = frameEl();
        if (fr) fr.hidden = true;
        var sysBtn = document.getElementById('preview-sys');
        if (sysBtn) sysBtn.hidden = false;
      }
      fileBox.hidden = false;
      fileBox.innerHTML = '';
      var fh = document.createElement('div');
      fh.className = 'pv-file-head';
      // 客户项目：看文件的时候，屏幕上要让「怎么回到我的项目」摆在最显眼的位置
      //（2026-09-16 老板问：用户点了其他文件预览后，再想看她的系统从哪里点？
      //  光靠右上角那个小按钮不够显 —— 不懂电脑的人不会往那儿找）。
      if (projKind === 'proxy') {
        var back = document.createElement('span');
        back.className = 'pv-back';
        back.id = 'pv-back';
        back.textContent = '← 回到我的项目';
        fh.appendChild(back);
      }
      var nm = document.createElement('span'); nm.className = 'pv-file-name'; nm.textContent = name;
      fh.appendChild(nm);
      if (download) {
        var dl2 = document.createElement('a'); dl2.textContent = '下载'; dl2.href = download; dl2.className = 'pv-dl';
        fh.appendChild(dl2);
      }
      var wrap = document.createElement('div');
      wrap.className = 'pv-file-body';
      wrap.appendChild(bodyEl);
      fileBox.appendChild(fh);
      fileBox.appendChild(wrap);
      return;
    }
    var old = document.getElementById('asbudy-preview'); if (old) old.remove();
    var d = document.createElement('div'); d.id = 'asbudy-preview';
    d.style.cssText = 'position:fixed;inset:0;z-index:10000;background:rgba(0,0,0,.62);display:flex;align-items:center;justify-content:center';
    var box = document.createElement('div'); box.style.cssText = 'background:var(--surface);border:1px solid var(--line);border-radius:12px;max-width:82vw;max-height:86vh;display:flex;flex-direction:column;overflow:hidden';
    var head = document.createElement('div'); head.style.cssText = 'display:flex;justify-content:space-between;align-items:center;padding:10px 16px;border-bottom:1px solid var(--line);gap:12px';
    var t = document.createElement('span'); t.style.cssText = 'font-size:14px;color:var(--text)'; t.textContent = name;
    var acts = document.createElement('div'); acts.style.cssText = 'display:flex;gap:8px;align-items:center';
    if (download) {
      var dl = document.createElement('a'); dl.textContent = '下载'; dl.href = download;
      dl.style.cssText = 'border:1px solid var(--line);color:var(--text-dim);font-size:13px;cursor:pointer;border-radius:6px;padding:3px 10px;text-decoration:none';
      acts.appendChild(dl);
    }
    var x = document.createElement('button'); x.textContent = '关闭'; x.style.cssText = 'background:none;border:1px solid var(--line);color:var(--text-dim);font-size:13px;cursor:pointer;border-radius:6px;padding:3px 10px';
    x.onclick = function () { d.remove(); };
    acts.appendChild(x);
    head.appendChild(t); head.appendChild(acts);
    box.appendChild(head); box.appendChild(bodyEl);
    d.appendChild(box); document.body.appendChild(d);
    d.onclick = function (e) { if (e.target === d) d.remove(); };
  }

  if (refresh) refresh.onclick = function () { loadProj(true); loadMine(); };   // 手动刷新 = 强制重绘
  setFold('proj', isFolded('proj'));      // 恢复上次的折叠状态
  setFold('mine', isFolded('mine'));
  setFold('undo', isFolded('undo'));      // 退回面板默认折叠（老板 2026-09-15；回收站在建卡时应用）
  ensureProjectBox();   // 左栏最上面那个「▶ 看我的项目」大按钮
  loadProj();
  loadMine();
  loadRecycle();

  // 对话产出后「立即出现」（2026-09-15）：老板反馈——以前要刷新整个页面才看得到。
  // 事件由 app.mjs 广播（item.completed = 某个工具刚干完，turn.completed = 这一轮干完）。
  // 节流 700ms：一轮里工具调用很密，不节流会把 /_gate/projfiles 打密；
  // loadProj 内部还有内容签名比对 —— 没变化就不重绘，不会闪、不会丢展开状态。
  //
  // ── 右栏预览的自动刷新（2026-09-16 老板定：**变才刷**）──
  // 为什么不每轮无脑刷：AI 一轮里会改好几个文件，每改一次就重载，页面会刷花，
  //   客户正在预览页面里做的事情（填了一半的表单、滚到的位置）也会被冲掉。
  // 所以三个条件都满足才动：① 这一轮**干完了**（turn.completed）；
  //   ② **文件真的变了**（projChanged）；③ 客户没正在 iframe 里操作（不然只挂提示）。
  var lastPicked = null;         // 右栏现在看的是哪个文件（自动刷新时重拉它）
  var pendingTurnEnd = false;

  /** 在预览栏标题后面闪一句（2.6 秒后自己消失） */
  function previewNotice(text, color) {
    var head = document.querySelector('#preview-pane .preview-head');
    if (!head) return;
    var old = head.querySelector('#preview-updated');
    if (old) old.remove();
    var s = document.createElement('span');
    s.id = 'preview-updated';
    s.textContent = text;
    s.style.cssText = 'margin-left:10px;font-size:12.5px;color:' + (color || 'var(--live)');
    var title = head.querySelector('.preview-title');
    if (title) title.appendChild(s); else head.appendChild(s);
    setTimeout(function () { if (s.parentNode) s.remove(); }, 2600);
  }

  /** 把右栏正在显示的东西重新取一遍（文件预览重拉内容 / 系统页面重载 iframe） */
  function refreshCurrentPreview() {
    if (!previewOpen()) return;
    var fb = document.getElementById('preview-file');
    if (fb && !fb.hidden) {
      // 正在看某个文件：重新拉它的内容（「我的资料」在项目目录外，不跟着项目变）
      if (lastPicked && !lastPicked.isMine) { openPreviewFor(lastPicked.f, lastPicked.isMine); previewNotice('已更新'); }
      return;
    }
    var f = frameEl();
    if (!f || f.hidden) return;
    // 客户正在 iframe 里点东西/填表（焦点在页面里）→ 别把页面重载掉，挂个提示让他自己决定
    if (document.activeElement === f) { previewNotice('有更新 · 点击「刷新」查看', 'var(--human)'); return; }
    f.src = f.src;
    previewNotice('已更新');
  }

  var autoRefreshTimer = null;
  window.addEventListener('asbudy:activity', function (e) {
    var ev = e && e.detail && e.detail.event;
    if (ev !== 'item.completed' && ev !== 'turn.completed') return;
    if (ev === 'turn.completed') pendingTurnEnd = true;
    clearTimeout(autoRefreshTimer);
    autoRefreshTimer = setTimeout(function () {
      var ended = pendingTurnEnd; pendingTurnEnd = false;
      // loadProj 跑完才知道 projChanged —— 文件真的变了才去动预览
      Promise.resolve(loadProj()).then(function () {
        loadRecycle();
        if (ended && projDirty) { projDirty = false; refreshCurrentPreview(); }
      });
    }, 700);
  });

  // 右侧「预览」栏（三栏右侧；窄屏变全屏层）
  // 布局由官方前端的 .preview-pane 提供，门卫只负责：按当前项目填 iframe + 切换显隐。
  // ⚠️ 本脚本注在侧栏里，而 .preview-pane / 预览按钮在它之后才解析 → 一律惰性取 + 事件委托
  //    （不能在脚本加载时 getElementById，那时它们还是 null）。
  function shellEl() { return document.getElementById('app-shell'); }
  function frameEl() { return document.getElementById('preview-frame'); }
  function previewOpen() { var s = shellEl(); return !!(s && s.classList.contains('has-preview')); }
  function showPreview() {
    var shell = shellEl();
    var pane = document.getElementById('preview-pane');
    if (!shell || !pane) return;
    var frame = frameEl();
    var fileBox = document.getElementById('preview-file');
    if (projKind === 'proxy') {
      // 客户项目：右栏 iframe 跑系统页面
      if (frame) {
        var want = previewUrl || ('/_pv/' + encodeURIComponent(pkey) + '/');
        if (frame.getAttribute('data-pkey') !== pkey) {
          frame.src = want;
          frame.setAttribute('data-pkey', pkey);
        }
        frame.hidden = false;
      }
      if (fileBox) fileBox.hidden = true;
    } else {
      // 工作台：右栏看文件内容（点左栏文件就显示在这里）
      if (frame) { frame.hidden = true; frame.src = 'about:blank'; frame.removeAttribute('data-pkey'); }
      if (fileBox) fileBox.hidden = false;
    }
    shell.classList.add('has-preview');
  }
  function hidePreview() { var s = shellEl(); if (s) s.classList.remove('has-preview'); }

  // 客户项目：右栏从「文件预览」切回「看系统页面」（iframe）
  function showSysPreview() {
    var shell = shellEl();
    var pane = document.getElementById('preview-pane');
    if (!shell || !pane) return;
    var frame = frameEl();
    var fileBox = document.getElementById('preview-file');
    if (projKind === 'proxy' && frame) {
      var want = previewUrl || ('/_pv/' + encodeURIComponent(pkey) + '/');
      if (frame.getAttribute('data-pkey') !== pkey) {
        frame.src = want;
        frame.setAttribute('data-pkey', pkey);
      }
      frame.hidden = false;
    }
    if (fileBox) fileBox.hidden = true;
    var sysBtn = document.getElementById('preview-sys');
    if (sysBtn) sysBtn.hidden = true;
    shell.classList.add('has-preview');
    // 手机：刚才是从**侧栏里**的「▶ 看我的项目」点进来的 —— 顺手把侧栏收起来，
    //   否则它就盖在刚打开的预览上面（点完还得再点一下空白处，很别扭）。2026-09-16 老板选②。
    if (window.matchMedia('(max-width: 800px)').matches && shell.classList.contains('rail-visible')) {
      shell.classList.remove('rail-visible');
      var railBtn = document.getElementById('rail-open');
      if (railBtn) railBtn.setAttribute('aria-expanded', 'false');
    }
  }

  // 分栏拖手：拽它调预览宽度（存在 .shell 的 --preview-width 上）
  document.addEventListener('mousedown', function (e) {
    var t = e.target;
    if (!t || t.id !== 'preview-grip') return;
    e.preventDefault();
    var shell = shellEl();
    var pane = document.getElementById('preview-pane');
    if (!shell || !pane) return;
    var startX = e.clientX;
    var startW = pane.getBoundingClientRect().width;
    // ⚠️ 关键（2026-09-16 老板：向右拖动收窄不行）：
    //   往右拖时鼠标会**落进右栏的 iframe**，而 iframe 会把 mousemove 吃掉 →
    //   父页面收不到移动 → 拖动卡死（往左是进对话区，普通 DIV，所以看着“能变宽不能收窄”）。
    //   所以拖动期间先把 iframe 的鼠标事件关掉，松手再恢复。
    var frames = document.querySelectorAll('#preview-pane iframe');
    Array.prototype.forEach.call(frames, function (f) { f.style.pointerEvents = 'none'; });
    var prevUserSelect = document.body.style.userSelect;
    document.body.style.userSelect = 'none';      // 顺手别选中文字
    function stopFrames() {
      Array.prototype.forEach.call(frames, function (f) { f.style.pointerEvents = ''; });
      document.body.style.userSelect = prevUserSelect;
    }
    function move(ev) {
      var w = startW - (ev.clientX - startX);
      // 下限跟 CSS 的 minmax(260px, …) 保持一致 —— 两处不一样会让「最窄能拖到哪」对不上
      w = Math.max(260, Math.min(window.innerWidth - 420, w));
      shell.style.setProperty('--preview-width', w + 'px');
    }
    function up() {
      document.removeEventListener('mousemove', move);
      document.removeEventListener('mouseup', up);
      window.removeEventListener('blur', up);
      stopFrames();
    }
    document.addEventListener('mousemove', move);
    document.addEventListener('mouseup', up);
    window.addEventListener('blur', up);          // 拖到一半切走窗口也不能卡住
  });

  // 所见即所得：进来就分栏（PC 左右分）。
  // ⚠️ 2026-09-16 老板实际用过手机后定（当轮第二次调整）：**手机（≤800px）进来先看对话，不自动开预览** ——
  //   自动开时预览占上面 44vh，挡得难受；而当初「手机上看不到项目」的病根是**没有入口**，
  //   不是「没自动打开」。所以：PC 仍进来就分栏；手机走左侧栏顶部的「▶ 看我的项目」（入口一直在）。
  //   断点跟 styles.css 的 @media (max-width: 800px) 对齐（别各写各的）。
  var isNarrow = function () { return window.matchMedia('(max-width: 800px)').matches; };
  if (!isNarrow()) {
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', function () { showPreview(); });
    else setTimeout(function () { showPreview(); }, 0);
  }

  document.addEventListener('click', function (e) {
    var t = e.target;
    if (!t) return;
    if (!t.id) return;
    if (t.id === 'preview-close') { hidePreview(); }
    else if (t.id === 'preview-sys') { showSysPreview(); }
    else if (t.id === 'pv-back') { showSysPreview(); }   // 文件预览区里那个「← 回到我的项目」
    else if (t.id === 'asb-openproj') { showSysPreview(); }   // 左侧栏顶上那个大按钮
    else if (t.id === 'preview-reveal') { showPreview(); }
    // 「单独打开」（2026-09-16 老板：客户没法像网站一样打开自己的项目）——
    // 门卫给的地址里已经带了一张短期票，开出去就是一个能全屏用、能给同事看的页面。
    else if (t.id === 'preview-newwin') {
      var u = previewUrl || (pkey ? ('/_pv/' + encodeURIComponent(pkey) + '/') : '');
      if (u) window.open(u, '_blank', 'noopener');
    }
    else if (t.id === 'preview-reload') { var f = frameEl(); if (f) f.src = f.src; }
    else if (t.id === 'asbudy-files-upload') { showUpMenu(t); }
    else if (t.id === 'asb-fold-proj') { setFold('proj', !isFolded('proj')); }
    else if (t.id === 'asb-fold-mine') { setFold('mine', !isFolded('mine')); }
    else if (t.id === 'asb-fold-recycle') { setFold('recycle', !isFolded('recycle')); }
    else if (t.id === 'asb-fold-undo') { setFold('undo', !isFolded('undo')); }
  });

  // ── 上传到「当前项目」（目录树立刻能看到、AI 直接能读） ──
  // 两个隐藏 input：选文件（可多选）/ 选整个文件夹（保留层级）
  var upInput = document.createElement('input');
  upInput.type = 'file'; upInput.multiple = true; upInput.style.display = 'none'; upInput.id = 'asbudy-up-input';
  var dirInput = document.createElement('input');
  dirInput.type = 'file'; dirInput.webkitdirectory = true; dirInput.multiple = true; dirInput.style.display = 'none'; dirInput.id = 'asbudy-up-dir-input';
  document.body.appendChild(upInput);
  document.body.appendChild(dirInput);

  function upOne(file, rel) {
    // 传「文件池」（uploads/<user>/）：项目目录砌墙后 gate 写不进去；
    // 文件池属 gate 自己，且引擎能读（目录 755）。带绝对路径就能让 AI 用。
    return fetch('/_gate/upload', {
      method: 'POST',
      headers: { 'Content-Type': 'application/octet-stream', 'X-Filename': encodeURIComponent(rel) },
      body: file,
    }).then(function (r) { return r.ok ? { ok: true } : r.json().catch(function () { return {}; }).then(function (j) { return { ok: false, err: j.error }; }); })
      .catch(function () { return { ok: false, err: '网络错误' }; });
  }
  function doUp(files, input) {
    var arr = Array.prototype.slice.call(files || []);
    if (!arr.length) return;
    var bar = document.getElementById('asbudy-files-body');
    if (bar) bar.innerHTML = '<span class="f-empty">正在上传 ' + arr.length + ' 个…</span>';
    var done = 0, failed = 0, lastErr = '';
    var chain = Promise.resolve();
    arr.forEach(function (f) {
      chain = chain.then(function () {
        return upOne(f, f.webkitRelativePath || f.name).then(function (r) {
          if (r.ok) done++; else { failed++; lastErr = r.err || ''; }
        });
      });
    });
    chain.then(function () {
      input.value = '';
      if (failed && lastErr) alert('有 ' + failed + ' 个没传上：' + lastErr);
      setFold('mine', false);   // 传完把「我的资料」展开，让用户立刻看到传了什么
      loadMine();
    });
  }
  upInput.onchange = function () { doUp(upInput.files, upInput); };
  dirInput.onchange = function () { doUp(dirInput.files, dirInput); };

  // 「+ 上传」的下拉：选文件 / 选整个文件夹（跟老系统一致）
  function showUpMenu(anchor) {
    var old = document.getElementById('asbudy-upmenu');
    if (old) { old.remove(); return; }
    var m = document.createElement('div');
    m.id = 'asbudy-upmenu';
    m.style.cssText = 'position:fixed;z-index:99999;background:var(--surface);border:1px solid var(--line);border-radius:8px;padding:4px;box-shadow:0 8px 24px rgba(0,0,0,.5)';
    var r = anchor.getBoundingClientRect();
    m.style.left = Math.max(8, r.left - 40) + 'px';
    m.style.top = (r.bottom + 4) + 'px';
    [['选文件（可多选）', upInput], ['选整个文件夹', dirInput]].forEach(function (it) {
      var d = document.createElement('div');
      d.textContent = it[0];
      d.style.cssText = 'padding:7px 12px;font-size:13.5px;color:var(--text);cursor:pointer;border-radius:5px;white-space:nowrap';
      d.onmouseenter = function () { d.style.background = 'var(--line)'; };
      d.onmouseleave = function () { d.style.background = 'transparent'; };
      d.onclick = function () { m.remove(); it[1].click(); };
      m.appendChild(d);
    });
    document.body.appendChild(m);
    setTimeout(function () {
      document.addEventListener('click', function close(ev) {
        if (!m.contains(ev.target)) { m.remove(); document.removeEventListener('click', close); }
      });
    }, 0);
  }

  // ── 退回面板：列出可退回的时间点，点一个恢复 ──
  var undoHost = document.getElementById('asbudy-undo');
  if (undoHost) {
    undoHost.hidden = false;
    var undoBody = document.getElementById('asbudy-undo-body');
    var undoRefresh = document.getElementById('asbudy-undo-refresh');
    function fmtTime(ts) {
      var t = new Date(ts);
      function p(n) { return String(n).padStart(2, '0'); }
      return t.getFullYear() + '-' + p(t.getMonth() + 1) + '-' + p(t.getDate()) + ' ' + p(t.getHours()) + ':' + p(t.getMinutes());
    }
    async function loadUndo() {
      undoBody.innerHTML = '<span class="f-empty">加载中…</span>';
      try {
        var r = await fetch('/_gate/snapshots', { credentials: 'same-origin' });
        if (!r.ok) throw 0;
        var d = await r.json();
        undoBody.innerHTML = '';
        if (!d.snapshots || !d.snapshots.length) { undoBody.innerHTML = '<span class="f-empty">（还没有可退回的改动）</span>'; return; }
        for (var i = 0; i < d.snapshots.length; i++) {
          (function (s) {
            var it = document.createElement('div');
            it.className = 'u-item';
            var lb = document.createElement('div'); lb.textContent = s.label || '(改动前)';
            var tm = document.createElement('div'); tm.className = 'u-time'; tm.textContent = fmtTime(s.ts);
            it.appendChild(lb); it.appendChild(tm);
            it.onclick = async function () {
              if (!confirm('确认退回到该时间点？此后的改动将被撤销（已填写的数据会保留）。')) return;
              try {
                var rr = await fetch('/_gate/restore', { method: 'POST', credentials: 'same-origin', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ snapshotId: s.id }) });
                var dd = await rr.json();
                if (rr.ok) { alert('已退回'); loadUndo(); loadProj(); }
                else alert(dd.error || '退回失败');
              } catch (e) { alert('退回失败'); }
            };
            undoBody.appendChild(it);
          })(d.snapshots[i]);
        }
      } catch (e) { undoBody.innerHTML = '<span class="f-empty">加载失败</span>'; }
    }
    if (undoRefresh) undoRefresh.onclick = loadUndo;
    loadUndo();
  }

  /* 手机：点开预览 = **整屏**（styles.css 的 @media(max-width:800px)），所以得让人一眼知道怎么回去 ——
     把头部那个「收起」在手机上写成「← 回到对话」。2026-09-16 老板：预览整屏后不能让人找不到回路。 */
  (function () {
    var relabel = function () {
      var b = document.getElementById('preview-close');
      if (!b) return;
      if (window.matchMedia('(max-width: 800px)').matches) {
        if (b.textContent !== '← 回到对话') b.textContent = '← 回到对话';
      }
    };
    relabel();
    window.addEventListener('resize', relabel);
    var n = 0;
    var t = setInterval(function () { relabel(); if (++n > 20) clearInterval(t); }, 500);
  })();
})();
