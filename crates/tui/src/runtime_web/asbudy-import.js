// AsBudy：「选择项目」页上的「新建项目 / 导入已有项目」（2026-09-15 · 老板要求）
//
// 为什么单独一个文件（不塞进 asbudy-my.js）：
//   这一页是门卫原生渲染的（没有官方对话界面那套 DOM），逻辑跟那边不共用；
//   分开放也便于官方源码升级时原样保留。
//
// 导入这条线为什么这么长（对应老板 2026-09-15 的四条要求）：
//   ① 选完文件夹**先算总大小**，跟资料池可用空间比一下，不够当场说，不白等；
//   ② **分片上传**（4MB 一片）—— 原来那个整块接口会把几百 MB 读进内存，也会一断就从头来；
//   ③ **断点续传** —— 上传进度记在服务端（manifest + .part 文件），刷新/断网回来接着传；
//   ④ **后台上传** —— 传输挂在页面级，收起面板也照传，底部细条显示进度；
//   ⑤ 导入成功 → 服务端已把新项目设为当前 → 这里直接跳过去；原件默认挪进回收站（可勾选保留）。
(function () {
  var root = document.getElementById('abx-bar');
  if (!root) return;                       // 不在这一页

  var $ = function (id) { return document.getElementById(id); };
  var CHUNK = 4 * 1024 * 1024;             // 4MB
  var PENDING = 'asbudy.imp.pending';      // localStorage：没传完的活
  // 跟 m0/scripts/create-project.sh 的排除项保持一致（那边还会再排一次，这里排掉省流量）
  var SKIP = ['node_modules', '.git', '.codewhale', '__pycache__', 'venv', '.venv', 'dist', 'build'];

  var S = { files: null, root: '', dest: '', total: 0, uid: null, keep: false, busy: false };

  /* ── 样式（这一页没加载官方界面那套 CSS）── */
  document.head.appendChild((function () {
    var st = document.createElement('style');
    st.textContent = [
      '.abx-bar{display:flex;gap:10px;flex-wrap:wrap;margin:0 0 14px}',
      '.abx-btn{padding:10px 16px;border:1px solid var(--line);border-radius:8px;background:var(--surface);color:var(--text);font:inherit;font-size:.95em;cursor:pointer}',
      '.abx-btn:hover{border-color:var(--action)}',
      '.abx-btn:disabled{opacity:.45;cursor:not-allowed}',
      '.abx-btn.abx-go{background:var(--action);border-color:var(--action);color:var(--action-contrast)}',
      '.abx-btn.abx-go:hover{border-color:var(--action)}',
      '.abx-btn.abx-ghost{background:transparent}',
      '.abx-panel{margin:0 0 18px;padding:14px 16px;border:1px solid var(--line);border-radius:10px;background:var(--bg)}',
      '.abx-h2{margin:0 0 12px;font-size:1.05em}',
      '.abx-row{display:flex;align-items:center;gap:10px;margin:10px 0}',
      '.abx-row>span{flex:0 0 56px;color:var(--text-dim);font-size:.9em}',
      '.abx-row input,.abx-row select{flex:1;min-width:0;padding:9px 11px;border:1px solid var(--line);border-radius:8px;background:var(--bg);color:var(--text);font:inherit;font-size:.95em}',
      '.abx-note{margin:8px 0 0;color:var(--text-dim);font-size:.88em;line-height:1.6}',
      '.abx-note.bad{color:var(--warning)}',
      '.abx-note.good{color:var(--live)}',
      '.abx-tip{margin:0 0 12px;padding:8px 10px;border:1px solid var(--human);border-radius:8px;background:var(--human-wash);color:var(--human);font-size:.88em;line-height:1.5}',
      '.abx-check{display:flex;gap:8px;align-items:flex-start;margin:12px 0 0;color:var(--text-dim);font-size:.9em;line-height:1.5}',
      '.abx-acts{display:flex;gap:10px;margin-top:14px}',
      '.abx-msg{margin-top:10px;min-height:1.2em;color:var(--text-dim);font-size:.9em;line-height:1.5}',
      '.abx-msg.bad{color:var(--warning)}',
      '.abx-msg.good{color:var(--live)}',
      '.abx-prog{margin-top:14px}',
      '.abx-prog .bar{height:8px;border-radius:99px;background:var(--surface-raised);overflow:hidden}',
      '.abx-prog .bar>i{display:block;height:100%;width:0;background:var(--action);transition:width .25s}',
      '.abx-prog .txt{display:block;margin-top:6px;color:var(--text-dim);font-size:.85em}',
      '.abx-slim{position:fixed;left:0;right:0;bottom:0;z-index:60;padding:12px 16px 10px;background:var(--bg);border-top:1px solid var(--line);color:var(--text-dim);font-size:.88em}',
      '.abx-slim>i{position:absolute;left:0;top:0;height:3px;width:0;background:var(--action);transition:width .25s}',
    ].join('\n');
    return st;
  })());

  function api(path, opt) {
    opt = opt || {};
    return fetch(path, {
      method: opt.method || 'GET',
      credentials: 'same-origin',
      headers: opt.body ? { 'Content-Type': 'application/json' } : undefined,
      body: opt.body ? JSON.stringify(opt.body) : undefined,
    }).then(function (r) {
      return r.json().catch(function () { return {}; }).then(function (j) {
        return { ok: r.ok, status: r.status, body: j || {} };
      });
    });
  }
  function fmtMb(mb) {
    if (mb === Infinity) return '不限';
    if (mb >= 1024) return (Math.round(mb / 1024 * 10) / 10) + ' GB';
    return Math.round(mb) + ' MB';
  }
  function fail(el, text) { el.className = 'abx-msg bad'; el.textContent = text; }
  function okMsg(el, text) { el.className = 'abx-msg good'; el.textContent = text; }
  function info(el, text) { el.className = 'abx-msg'; el.textContent = text; }
  /** 把浏览器的英文报错说成人话 */
  function friendly(err) {
    var m = String((err && err.message) || err || '');
    if (/Failed to fetch|NetworkError|load failed|ERR_/i.test(m)) return '网络断了';
    return m || '导入中断';
  }
  /** 读「没传完的活」（刷新之后 S.uid 会丢，得从这儿找回来） */
  function readPending() {
    try {
      var p = JSON.parse(localStorage.getItem(PENDING) || 'null');
      return (p && p.uid && p.files) ? p : null;
    } catch (e) { return null; }
  }

  /* ── 底部细条：后台上传的进度 / 结果 ── */
  var slim = $('abx-slim'), slimBar = $('abx-slim-bar'), slimTxt = $('abx-slim-txt');
  function showSlim(pct, text) {
    slim.hidden = false;
    slimBar.style.width = (pct == null ? 0 : Math.max(0, Math.min(100, pct))) + '%';
    slimTxt.textContent = text || '';
  }
  function hideSlim() { slim.hidden = true; }

  /* ══════════ 新建项目 ══════════ */
  var newPanel = $('abx-new-panel');
  $('abx-new').onclick = function () {
    newPanel.hidden = !newPanel.hidden;
    if (!newPanel.hidden) { $('abx-imp-panel').hidden = true; $('abx-new-name').focus(); }
  };
  $('abx-new-cancel').onclick = function () { newPanel.hidden = true; };
  $('abx-new-go').onclick = function () {
    var name = ($('abx-new-name').value || '').trim();
    var msgEl = $('abx-new-msg');
    if (!name) return fail(msgEl, '给它起个名字吧');
    this.disabled = true;
    info(msgEl, '正在建，稍等（约 10~30 秒）…');
    var go = this;
    api('/_gate/projects/create', { method: 'POST', body: { name: name, template: $('abx-new-tpl').value } })
      .then(function (r) {
        go.disabled = false;
        if (!r.ok) return fail(msgEl, r.body.error || '没建成');
        okMsg(msgEl, '建好了，正在带你过去…');
        location.href = '/';            // 服务端已把它设为当前项目
      })
      .catch(function (e) { go.disabled = false; fail(msgEl, '没建成：' + e.message); });
  };

  /* 「先给我演示一个」—— 空状态页的主行动（2026-09-16 老板定 A 方案）
   * 自动建一个带示例的项目（名字/模板用默认），省掉「起名字 + 选从哪开始」两步 ——
   * NN/g 空状态规范里说的「直达关键任务的路」就是一步到位，别让新手先做选择题。 */
  var quickBtn = $('abx-quick');
  if (quickBtn) quickBtn.onclick = function () {
    var qMsg = $('abx-quick-msg');
    var old = quickBtn.textContent;
    quickBtn.disabled = true;
    quickBtn.textContent = '正在给你准备（约 10~30 秒）…';
    info(qMsg, '建好会自动带你进去。');
    api('/_gate/projects/create', { method: 'POST', body: { name: '我的第一个系统', template: 'example' } })
      .then(function (r) {
        if (!r.ok) { quickBtn.disabled = false; quickBtn.textContent = old; return fail(qMsg, (r.body && r.body.error) || '没建成'); }
        okMsg(qMsg, '建好了，正在带你过去…');
        location.href = '/';
      })
      .catch(function (e) { quickBtn.disabled = false; quickBtn.textContent = old; fail(qMsg, '没建成：' + e.message); });
  };

  /* ══════════ 导入已有项目 ══════════ */
  var impPanel = $('abx-imp-panel'), inp = $('abx-imp-input');
  var pickInfo = $('abx-imp-pickinfo'), impMsg = $('abx-imp-msg'), impGo = $('abx-imp-go');
  var progBox = $('abx-imp-prog'), progBar = $('abx-imp-bar'), progTxt = $('abx-imp-progtxt');

  $('abx-import').onclick = function () {
    impPanel.hidden = !impPanel.hidden;
    if (!impPanel.hidden) newPanel.hidden = true;
  };
  $('abx-imp-cancel').onclick = function () {
    if (S.busy) { info(impMsg, '传输在后台继续，收起不影响；传完会自己切过去。'); }
    impPanel.hidden = true;
  };
  $('abx-imp-pick').onclick = function () { inp.value = ''; inp.click(); };

  inp.onchange = function () {
    var picked = collect(inp.files || []);
    if (!picked.files.length) {
      impGo.disabled = true;
      return fail(impMsg, '这个文件夹里没找到可导入的文件（node_modules / .git 这类会自动跳过）');
    }
    // 断点接续（老板 2026-09-15 要的）：重新选的是**同一个文件夹**（路径+大小逐个对得上）
    // 就沿用上次的 uploadId，服务端那边 .part 还在，从断的地方接着传。
    // 刷新后 S.uid / S.files 都会丢 —— 所以还得从 localStorage 里的「没传完的活」里找回来。
    var prevUid = S.uid, prevRoot = S.root, prevFiles = S.files;
    var pend = readPending();
    if (pend && pend.root === picked.root && (!prevUid || !prevFiles)) {
      // 刷新回来时 S.uid / S.files 都会丢（File 对象没法持久化）——
      // 用 localStorage 里记的那份清单来核对，这才接得上上次的进度
      prevUid = prevUid || pend.uid;
      prevRoot = pend.root;
      prevFiles = pend.files.map(function (f) { return { path: f.path, size: f.size }; });
    }
    var same = !!prevUid && prevRoot === picked.root && !!prevFiles
      && prevFiles.length === picked.files.length
      && prevFiles.every(function (f, i) { return f.path === picked.files[i].path && f.size === picked.files[i].size; });
    S.files = picked.files;
    S.root = picked.root;
    S.total = picked.total;
    S.dest = picked.root;
    S.uid = same ? prevUid : null;
    if (!$('abx-imp-name').value.trim()) $('abx-imp-name').value = picked.root;
    info(impMsg, '正在核对空间…');
    precheck(S.total, S.root).then(function (c) {
      var tail = '共 ' + picked.files.length + ' 个文件 · ' + fmtMb(c.needMb);
      if (!c.enough) {
        pickInfo.className = 'abx-note bad';
        pickInfo.textContent = '空间不够：这个文件夹约 ' + fmtMb(c.needMb) + '，可用的只有 ' + fmtMb(c.availMb) + '。';
        impGo.disabled = true;
        return fail(impMsg, '先删点旧资料、或清空回收站，再来导。');
      }
      pickInfo.className = 'abx-note good';
      pickInfo.textContent = '可以导：' + tail + '，可用空间 ' + fmtMb(c.availMb) + '。';
      if (!S.uid) { okMsg(impMsg, '可以开始导入了。'); impGo.disabled = false; return; }
      // 清单一致 → 问服务端“传到哪了”
      api('/_gate/upload/status?uploadId=' + encodeURIComponent(S.uid)).then(function (st) {
        var server = (st.ok && st.body.files) || [];
        var eq = server.length === picked.files.length
          && server.every(function (a, i) { return a.path === picked.files[i].path && a.size === picked.files[i].size; });
        if (!eq) { S.uid = null; impGo.disabled = false; return okMsg(impMsg, '可以开始导入了。'); }
        S.dest = st.body.name || S.dest;
        var done = 0;
        for (var k in (st.body.received || {})) done += st.body.received[k];
        okMsg(impMsg, '接上上次的进度：已经传到 ' + fmtMb(done / 1048576) + '，点「导入」继续。');
        impGo.disabled = false;
      });
    }).catch(function (e) { fail(impMsg, '没读到空间信息：' + e.message); });
  };

  /** 把用户选的文件夹整理成上传清单：去掉最外层文件夹名（它就是目标文件夹名） */
  function collect(list) {
    var out = [], name = '', total = 0;
    for (var i = 0; i < list.length; i++) {
      var f = list[i];
      var rel = f.webkitRelativePath || f.name;
      var segs = String(rel).split('/').filter(Boolean);
      if (segs.length < 2) continue;                     // 顶层散文件：这一页只服务"整个文件夹"
      if (!name) name = segs[0];
      if (segs[0] !== name) continue;                    // 只收一个根
      var rest = segs.slice(1);
      var bad = false;
      for (var j = 0; j < rest.length; j++) if (SKIP.indexOf(rest[j]) >= 0) bad = true;
      if (bad) continue;
      if (/\.log$/i.test(rest[rest.length - 1])) continue;
      out.push({ path: rest.join('/'), size: f.size, file: f });
      total += f.size;
    }
    return { root: name, files: out, total: total };
  }

  /** 空间预检：拿资料池用量 + 目标文件夹名有没有撞车 */
  function precheck(total, destName) {
    return api('/_gate/files').then(function (r) {
      var u = (r.body && r.body.usage) || {};
      var names = (r.body && r.body.files) || [];
      var unlimited = !!u.unlimited;
      var availMb = unlimited ? Infinity : Math.max(0, (u.quotaMb || 0) - (u.totalMb || 0));
      var needMb = Math.ceil(total / 1048576);
      // 资料池里已有同名文件夹 → 目标名加后缀，别覆盖客户已有的东西
      var taken = function (n) { return names.some(function (x) { return x.name === n; }); };
      if (taken(destName)) {
        for (var n = 2; n < 60; n++) { if (!taken(destName + ' (' + n + ')')) { S.dest = destName + ' (' + n + ')'; break; } }
      }
      return { unlimited: unlimited, availMb: availMb, needMb: needMb, enough: unlimited || needMb <= availMb };
    });
  }

  /** 单片上传（带重试；服务端说进度对不上就用它的） */
  function sendChunk(uid, rel, offset, buf, tries) {
    tries = tries || 0;
    return fetch('/_gate/upload/chunk', {
      method: 'POST', credentials: 'same-origin',
      headers: { 'x-upload-id': uid, 'x-upload-path': encodeURIComponent(rel), 'x-offset': String(offset) },
      body: buf,
    }).then(function (r) {
      return r.json().catch(function () { return {}; }).then(function (j) {
        return { ok: r.ok, status: r.status, body: j || {} };
      });
    }).catch(function (e) { return { ok: false, status: 0, body: { error: String(e && e.message || e) } }; })
      .then(function (res) {
        if (res.ok) return res.body;
        if (res.status === 409 && typeof res.body.received === 'number') return { received: res.body.received };
        if (tries < 3) {
          return new Promise(function (r) { setTimeout(r, 700 * (tries + 1)); })
            .then(function () { return sendChunk(uid, rel, offset, buf, tries + 1); });
        }
        throw new Error(res.body.error || '网络中断');
      });
  }

  /** 分片上传：从服务端记录的进度接着传（这就是断点续传） */
  function uploadAll(uid, files, total) {
    return api('/_gate/upload/status?uploadId=' + encodeURIComponent(uid)).then(function (st) {
      var received = (st.ok && st.body.received) || {};
      var done = 0;
      for (var k in received) done += received[k];
      paint(done, total, '正在上传（已传 ' + fmtMb(done / 1048576) + ' / ' + fmtMb(total / 1048576) + '）');
      var chain = Promise.resolve();
      files.forEach(function (it) {
        chain = chain.then(function () {
          var off = Math.min(received[it.path] || 0, it.size);
          var step = function () {
            if (off >= it.size) return Promise.resolve();
            var end = Math.min(off + CHUNK, it.size);
            return it.file.slice(off, end).arrayBuffer().then(function (buf) {
              return sendChunk(uid, it.path, off, buf);
            }).then(function (res) {
              var prev = off;
              off = res.received;
              done += Math.max(0, off - prev);
              paint(done, total, '正在上传（已传 ' + fmtMb(done / 1048576) + ' / ' + fmtMb(total / 1048576) + '）');
              return step();
            });
          };
          return step();
        });
      });
      return chain;
    });
  }

  function paint(done, total, text) {
    var pct = total ? Math.round(done / total * 100) : 0;
    progBox.hidden = false;
    progBar.style.width = pct + '%';
    progTxt.textContent = text + '（' + pct + '%）';
    showSlim(pct, '导入中：' + text.replace(/^正在/, ''));
  }

  function savePending() {
    try {
      localStorage.setItem(PENDING, JSON.stringify({
        uid: S.uid, dest: S.dest, root: S.root, total: S.total,
        files: S.files.map(function (f) { return { path: f.path, size: f.size }; }),
        at: Date.now(),
      }));
    } catch (e) {}
  }

  impGo.onclick = function () {
    if (S.busy || !S.files) return;
    S.keep = !!$('abx-imp-keep').checked;
    var projName = ($('abx-imp-name').value || '').trim() || S.root;
    impPanel.hidden = false;
    runImport(projName);
  };

  function runImport(projName) {
    S.busy = true;
    impGo.disabled = true;
    okMsg(impMsg, '开始导入。可以收起这个面板去干别的，传输在后台继续。');
    // 先把上次的 uploadId 验一下（可能已经在服务端过期了）→ 无效就重新开传
    var p = (S.uid
      ? api('/_gate/upload/status?uploadId=' + encodeURIComponent(S.uid)).then(function (st) { if (!st.ok) S.uid = null; })
      : Promise.resolve()
    ).then(function () {
      if (S.uid) return null;
      return api('/_gate/upload/init', {
        method: 'POST',
        body: { name: S.dest, files: S.files.map(function (f) { return { path: f.path, size: f.size }; }) },
      }).then(function (r) {
        if (!r.ok) throw new Error(r.body.error || '开传失败');
        S.uid = r.body.uploadId;
        savePending();
      });
    });

    p.then(function () { return uploadAll(S.uid, S.files, S.total); })
      .then(function () {
        paint(S.total, S.total, '正在收尾（整理目录）');
        return api('/_gate/upload/finish', { method: 'POST', body: { uploadId: S.uid, name: S.dest } });
      })
      .then(function (r) {
        if (!r.ok) throw new Error(r.body.error || '上传收尾失败');
        progTxt.textContent = '正在建项目（约 30~60 秒）…';
        showSlim(100, '导入中：正在建项目…');
        return api('/_gate/projects/create', { method: 'POST', body: { name: projName, template: 'blank', from: S.dest } });
      })
      .then(function (r) {
        if (!r.ok) throw new Error(r.body.error || '建项目失败');
        if (S.keep) return null;
        // 原件挪进回收站（不是真删 —— 客户随时能还原）
        return api('/_gate/file?name=' + encodeURIComponent(S.dest), { method: 'DELETE' });
      })
      .then(function () {
        try { localStorage.removeItem(PENDING); } catch (e) {}
        okMsg(impMsg, '导入好了，正在带你过去…');
        showSlim(100, '导入好了，正在切到新项目…');
        location.href = '/';        // 服务端已把新项目设为当前
      })
      .catch(function (e) {
        S.busy = false;
        impGo.disabled = false;
        fail(impMsg, friendly(e) + ' —— 已经传的部分还在，再点一次【导入】会接着传。');
        showSlim(null, '导入中断：' + friendly(e) + '（点【导入已有项目】可继续）');
      });
  }

  /* ── 上次没传完：回来时提示一下（浏览器的安全限制：必须重新选同一个文件夹） ── */
  (function resumeHint() {
    var p = readPending();
    var raw = null;
    try { raw = localStorage.getItem(PENDING); } catch (e) {}
    if (!p) { if (raw) { try { localStorage.removeItem(PENDING); } catch (e) {} } return; }
    if (Date.now() - (p.at || 0) > 7 * 24 * 3600 * 1000) { try { localStorage.removeItem(PENDING); } catch (e) {} return; }
    impPanel.hidden = false;
    var tip = $('abx-imp-resume');
    tip.hidden = false;
    tip.textContent = '上次「' + (p.root || '') + '」传到一半。服务端还留着进度 —— 重新选同一个文件夹，会从断的地方接着传。';
    $('abx-imp-name').value = p.root || '';
    S.root = p.root || '';
    S.uid = p.uid;
    S.total = p.total || 0;
    showSlim(null, '上次的导入没传完，点「导入已有项目」继续');
  })();
})();
