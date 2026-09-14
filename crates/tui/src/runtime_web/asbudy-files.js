// AsBudy 目录树面板 —— 门卫注入到官方界面侧栏。
// 官方 web 没有「列目录」的 API（/v1/files 404），门卫读文件系统提供目录树 + 文件预览。
(function () {
  var host = document.getElementById('asbudy-files');
  if (!host) return;
  host.hidden = false;   // 骨架默认 hidden（避免脚本没加载时显示「加载中…」），脚本跑起来才露出来
  var pkey = host.getAttribute('data-pkey') || '';
  var projKind = host.getAttribute('data-kind') || 'proxy';   // proxy = 系统页面 | artifacts = 工作台（看文件内容）
  var body = document.getElementById('asbudy-files-body');        // 「项目文件」卡片内容区
  var mineBody = document.getElementById('asbudy-mine-body');     // 「我的资料」卡片内容区
  var refresh = document.getElementById('asbudy-files-refresh');

  // 样式（动态注入，避免改官方 stylesheet）
  var css = [
    '#asbudy-files{margin:12px 12px 0;display:flex;flex-direction:column;gap:8px}',
    '.asb-card{border:1px solid var(--line);border-radius:8px;background:var(--surface);overflow:hidden}',
    '.asb-hd{display:flex;align-items:center;gap:6px;padding:7px 9px;font-size:12px;color:var(--text)}',
    '.asb-fold{cursor:pointer;color:var(--text-faint);width:12px;text-align:center;user-select:none;flex:none}',
    '.asb-fold:hover{color:var(--text)}',
    '.asb-title{flex:1;min-width:0;font-weight:600;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}',
    '.asb-title #asb-mine-count{font-weight:400;color:var(--text-faint)}',
    '.asb-tools{flex:none;color:var(--text-dim);display:flex;gap:10px;align-items:center}',
    '.asb-bd{padding:0 9px 9px;max-height:30vh;overflow-y:auto}',
    '.asb-card.folded .asb-bd{display:none}',
    '#asbudy-files-upload{cursor:pointer;color:var(--text-dim)}#asbudy-files-upload:hover{color:var(--text)}',
    '#asbudy-files-refresh{cursor:pointer;color:var(--text-faint)}#asbudy-files-refresh:hover{color:var(--text)}',
    '.f-node{display:flex;align-items:center;gap:6px;cursor:pointer;font-size:12px;line-height:1.7;white-space:nowrap;color:var(--text);padding:0 4px;border-radius:4px}',
    '.f-node .f-ic{flex:none;color:var(--text-soft);width:13px;text-align:center}',
    '.f-node .f-nm{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis}',
    '.f-node .f-sz{flex:none;color:var(--text-faint);font-size:10.5px}',
    '.f-node .f-tag{flex:none;color:var(--text-faint);font-size:10px;border:1px solid var(--line);border-radius:3px;padding:0 3px;line-height:1.5}',
    '.f-node .f-del{flex:none;color:#c9ccd0;padding:0 3px}',
    '.f-node .f-del:hover{color:#f85149}',
    '.f-node:hover{background:var(--hover)}',
    '.f-node.dir{color:var(--text-soft)}',
    '.f-kids{margin-left:12px;border-left:1px solid var(--line);padding-left:6px}',
    '.f-empty{font-size:12px;color:var(--text-faint);padding:0 4px}',
    '#asbudy-undo{margin:8px 12px 0;border:1px solid var(--line);border-radius:8px;padding:8px;background:var(--surface)}',
    '#asbudy-undo-body{max-height:22vh;overflow-y:auto}',
    '#asbudy-arts{margin:8px 12px 0;border:1px solid var(--line);border-radius:8px;padding:8px;background:var(--surface)}',
    '.a-head{font-size:12px;color:var(--text);font-weight:600;display:flex;justify-content:space-between;align-items:center;margin-bottom:6px}',
    '#asbudy-arts-toggle{cursor:pointer;color:var(--text-faint);font-weight:400}',
    '.a-list{max-height:22vh;overflow-y:auto}',
    '.a-item{display:flex;align-items:center;gap:8px;padding:5px 6px;border-radius:4px;cursor:pointer;font-size:12px;color:var(--text)}',
    '.a-item:hover{background:var(--hover)}',
    '.a-ic{flex:none;color:var(--text-soft)}',
    '.a-nm{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}',
    '.a-meta{flex:none;color:var(--text-faint);font-size:11px}',
    '.a-del{flex:none;color:#c9ccd0;padding:0 3px}',
    '.a-del:hover{color:#f85149}',
    '.u-item{font-size:12px;color:var(--text);padding:4px 6px;border-radius:4px;cursor:pointer;line-height:1.5}',
    '.u-item:hover{background:var(--hover)}',
    '.u-time{color:var(--text-faint);font-size:11px}'
  ].join('\n');
  var st = document.createElement('style');
  st.textContent = css;
  document.head.appendChild(st);

  // ── 两个卡片（不再用 tab 切换）：各自可折叠，折叠状态记在 localStorage（下次进来保持）──
  function foldKey(which) { return 'asbudy.fold.' + which; }
  function isFolded(which) {
    try { return localStorage.getItem(foldKey(which)) === '1'; } catch (e) { return false; }
  }
  function setFold(which, folded) {
    var card = document.getElementById('asb-card-' + which);
    var btn = document.getElementById('asb-fold-' + which);
    if (card) card.className = 'asb-card' + (folded ? ' folded' : '');
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
        h.innerHTML = '<span>做好的东西 ' + list.length + ' 个</span><span id="asbudy-arts-toggle">收起</span>';
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
        if (d.kind === 'office') showPanel(a.name, htmlBody(d.html || ''), d.download);
        else if (d.kind === 'text' || d.kind === 'svg') showPanel(a.name, textBody(d.text || ''), d.download);
        else if (d.kind === 'image') showPanel(a.name, imgBody(d.url), d.download);
        else if (d.kind === 'pdf') showPanel(a.name, pdfBody(d.url), d.download);
        else showPanel(a.name, textBody('这种格式暂时只能下下来看'), d.download);
      })
      .catch(function () { alert('读不到这个文件'); });
  }

  // 卡片一：项目里的文件（只读目录树）
  async function loadProj() {
    if (!body) return;
    body.innerHTML = '<span class="f-empty">加载中…</span>';
    try {
      var r = await fetch('/_gate/projfiles?project=' + encodeURIComponent(pkey), { credentials: 'same-origin' });
      if (!r.ok) throw 0;
      var d = await r.json();
      body.innerHTML = '';
      if (!d.files || !d.files.length) { body.innerHTML = '<span class="f-empty">（空目录）</span>'; return; }
      for (var i = 0; i < d.files.length; i++) body.appendChild(render(d.files[i], false));
    } catch (e) { body.innerHTML = '<span class="f-empty">加载失败</span>'; }
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
      if (cnt) cnt.textContent = files.length ? '（' + files.length + ' 份）' : '';
      mineBody.innerHTML = '';
      if (!files.length) { mineBody.innerHTML = '<span class="f-empty">还没传过资料 —— 点右边「+ 传资料」</span>'; return; }
      for (var i = 0; i < files.length; i++) mineBody.appendChild(render(files[i], true));
    } catch (e) { mineBody.innerHTML = '<span class="f-empty">加载失败</span>'; }
  }
  function render(f, isMine) {
    var w = document.createElement('div');
    if (f.isDir) {
      var h = document.createElement('div'); h.className = 'f-node dir';
      h.innerHTML = '<span class="f-ic">▸</span><span class="f-nm">' + aEsc(f.name) + '</span>' +
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
          if (!confirm('删掉整个文件夹「' + f.name + '」？里面的东西都会没。')) return;
          fetch('/_gate/file?name=' + encodeURIComponent(f.path), { method: 'DELETE', credentials: 'same-origin' })
            .then(function (r) { if (r.ok) loadMine(); else alert('删不掉'); });
        };
      }
      for (var i = 0; i < (f.children || []).length; i++) kids.appendChild(render(f.children[i], isMine));
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
        if (!confirm('删掉「' + f.name + '」？删了拿不回来。')) return;
        var url = isMine
          ? '/_gate/file?name=' + encodeURIComponent(f.path)
          : '/_gate/artifact?project=' + encodeURIComponent(pkey) + '&path=' + encodeURIComponent(f.path);
        fetch(url, { method: 'DELETE', credentials: 'same-origin' })
          .then(function (r) { if (r.ok) { isMine ? loadMine() : loadProj(); } else alert('删不掉'); });
      };
      w.appendChild(n);
    }
    return w;
  }

  // 点一个文件：不管是哪个 tab，都「带上」（告诉 AI 用这份）+ 弹预览
  function onPick(f, isMine) {
    // 文件池在项目目录外 → 带绝对路径（AI 能读任意路径）；项目文件用相对路径即可
    var p = isMine ? (f.abs || f.path) : f.path;
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
      if (d.kind === 'office') showPanel(f.name, htmlBody(d.html || ''), d.download);
      else if (d.kind === 'table') showPanel(f.name, tableBody(d), d.download);
      else if (d.kind === 'text' || d.kind === 'svg' || d.kind === 'code') showPanel(f.name, textBody(d.text || d.content || ''), d.download);
      else if (d.kind === 'image') showPanel(f.name, imgBody(d.url), d.download);
      else if (d.kind === 'pdf') showPanel(f.name, pdfBody(d.url), d.download);
      else if (d.kind === 'unsupported') showPanel(f.name, textBody(d.note || '这种格式暂时只能下下来看'), d.download);
      else showPanel(f.name, textBody('这种格式暂时只能下下来看'), d.download);
    } catch (e) { alert('读不到这个文件'); }
  }

  async function openFile(fp, name) {
    // 告诉「@文件」逻辑：用户点了这份文件（路径会被带到对话里）
    try { document.dispatchEvent(new CustomEvent('asbudy-file-picked', { detail: { path: fp, name: name } })); } catch (e0) {}
    try {
      var r = await fetch('/_gate/artifact/view?project=' + encodeURIComponent(pkey) + '&path=' + encodeURIComponent(fp), { credentials: 'same-origin' });
      if (!r.ok) { alert('读不到这个文件'); return; }
      var d = await r.json();
      if (d.kind === 'office') showPanel(name, htmlBody(d.html || ''), d.download);
      else if (d.kind === 'table') showPanel(name, tableBody(d), d.download);
      else if (d.kind === 'text' || d.kind === 'svg' || d.kind === 'code') showPanel(name, textBody(d.text || d.content || ''), d.download);
      else if (d.kind === 'image') showPanel(name, imgBody(d.url), d.download);
      else if (d.kind === 'pdf') showPanel(name, pdfBody(d.url), d.download);
      else if (d.kind === 'unsupported') showPanel(name, textBody(d.note || '这种格式暂时只能下下来看'), d.download);
      else showPanel(name, textBody('这种格式暂时只能下下来看'), d.download);
    } catch (e) { alert('读不到这个文件'); }
  }

  function textBody(text) {
    var pre = document.createElement('pre');
    pre.style.cssText = 'margin:0;padding:16px;overflow:auto;font:12px/1.5 ui-monospace,monospace;color:#e6edf3;white-space:pre-wrap;word-break:break-all';
    pre.textContent = text;
    return pre;
  }
  function htmlBody(html) {
    var div = document.createElement('div');
    div.style.cssText = 'margin:0;padding:16px;overflow:auto;color:#e6edf3;font-size:13px;line-height:1.6';
    div.innerHTML = html;
    return div;
  }
  function imgBody(url) {
    var img = document.createElement('img');
    img.src = url; img.style.cssText = 'max-width:100%;max-height:70vh;object-fit:contain';
    return img;
  }
  function pdfBody(url) {
    var f = document.createElement('iframe');
    f.src = url; f.style.cssText = 'width:100%;height:70vh;border:0';
    return f;
  }
  function tableBody(d) {
    var wrap = document.createElement('div');
    wrap.style.cssText = 'margin:0;padding:16px;overflow:auto;color:#e6edf3;font-size:13px;line-height:1.6';
    if (d.sheet) {
      var sh = document.createElement('div');
      sh.style.cssText = 'margin-bottom:8px;color:#8b949e;font-size:12px';
      sh.textContent = '工作表：' + d.sheet;
      wrap.appendChild(sh);
    }
    var rows = d.rows || [];
    if (!rows.length) { wrap.textContent = '（这个表格是空的）'; return wrap; }
    var maxCols = 0;
    for (var i = 0; i < rows.length; i++) if (rows[i].length > maxCols) maxCols = rows[i].length;
    var tb = document.createElement('table');
    tb.style.cssText = 'border-collapse:collapse;width:100%;font-size:12px';
    for (var r = 0; r < rows.length; r++) {
      var tr = document.createElement('tr');
      for (var c = 0; c < maxCols; c++) {
        var cell = document.createElement(r === 0 ? 'th' : 'td');
        cell.style.cssText = 'border:1px solid #30363d;padding:4px 8px;text-align:left;white-space:nowrap;' + (r === 0 ? 'background:#161b22;font-weight:600' : '');
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
    var box = document.createElement('div'); box.style.cssText = 'background:#0d1117;border:1px solid #30363d;border-radius:12px;max-width:82vw;max-height:86vh;display:flex;flex-direction:column;overflow:hidden';
    var head = document.createElement('div'); head.style.cssText = 'display:flex;justify-content:space-between;align-items:center;padding:10px 16px;border-bottom:1px solid #30363d;gap:12px';
    var t = document.createElement('span'); t.style.cssText = 'font-size:13px;color:#e6edf3'; t.textContent = name;
    var acts = document.createElement('div'); acts.style.cssText = 'display:flex;gap:8px;align-items:center';
    if (download) {
      var dl = document.createElement('a'); dl.textContent = '下载'; dl.href = download;
      dl.style.cssText = 'border:1px solid #30363d;color:#8b949e;font-size:12px;cursor:pointer;border-radius:6px;padding:3px 10px;text-decoration:none';
      acts.appendChild(dl);
    }
    var x = document.createElement('button'); x.textContent = '关闭'; x.style.cssText = 'background:none;border:1px solid #30363d;color:#8b949e;font-size:12px;cursor:pointer;border-radius:6px;padding:3px 10px';
    x.onclick = function () { d.remove(); };
    acts.appendChild(x);
    head.appendChild(t); head.appendChild(acts);
    box.appendChild(head); box.appendChild(bodyEl);
    d.appendChild(box); document.body.appendChild(d);
    d.onclick = function (e) { if (e.target === d) d.remove(); };
  }

  if (refresh) refresh.onclick = function () { loadProj(); loadMine(); };
  setFold('proj', isFolded('proj'));      // 恢复上次的折叠状态
  setFold('mine', isFolded('mine'));
  loadProj();
  loadMine();

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
        var want = '/_pv/' + encodeURIComponent(pkey) + '/';
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
      var want = '/_pv/' + encodeURIComponent(pkey) + '/';
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
    function move(ev) {
      var w = startW - (ev.clientX - startX);
      w = Math.max(260, Math.min(window.innerWidth - 420, w));
      shell.style.setProperty('--preview-width', w + 'px');
    }
    function up() {
      document.removeEventListener('mousemove', move);
      document.removeEventListener('mouseup', up);
    }
    document.addEventListener('mousemove', move);
    document.addEventListener('mouseup', up);
  });

  // 所见即所得：PC（宽屏）进来默认分栏；手机（窄屏）默认对话，点文件才开预览
  if (window.matchMedia && window.matchMedia('(min-width: 801px)').matches) {
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', function () { showPreview(); });
    else setTimeout(function () { showPreview(); }, 0);
  }

  document.addEventListener('click', function (e) {
    var t = e.target;
    if (!t) return;
    if (!t.id) return;
    if (t.id === 'preview-close') { hidePreview(); }
    else if (t.id === 'preview-sys') { showSysPreview(); }
    else if (t.id === 'preview-reveal') { showPreview(); }
    else if (t.id === 'preview-reload') { var f = frameEl(); if (f) f.src = f.src; }
    else if (t.id === 'asbudy-files-upload') { showUpMenu(t); }
    else if (t.id === 'asb-fold-proj') { setFold('proj', !isFolded('proj')); }
    else if (t.id === 'asb-fold-mine') { setFold('mine', !isFolded('mine')); }
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
    m.style.cssText = 'position:fixed;z-index:99999;background:#0d1117;border:1px solid #30363d;border-radius:8px;padding:4px;box-shadow:0 8px 24px rgba(0,0,0,.5)';
    var r = anchor.getBoundingClientRect();
    m.style.left = Math.max(8, r.left - 40) + 'px';
    m.style.top = (r.bottom + 4) + 'px';
    [['选文件（可多选）', upInput], ['选整个文件夹', dirInput]].forEach(function (it) {
      var d = document.createElement('div');
      d.textContent = it[0];
      d.style.cssText = 'padding:7px 12px;font-size:12.5px;color:#e6edf3;cursor:pointer;border-radius:5px;white-space:nowrap';
      d.onmouseenter = function () { d.style.background = '#21262d'; };
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
              if (!confirm('确定退回到这个时间点吗？之后的改动会被撤销（你填的数据都还在）。')) return;
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
})();
