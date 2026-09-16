/* AsBudy「我的」菜单 + 员工管理（门卫注入 · 2026-09-14）
 * 为什么在门卫：官方界面没有多租户/账号概念，这些是「多租户外壳」的东西。
 * 入口：点侧栏左上角 logo →「我的」菜单。
 * 后端依赖：/_gate/whoami、/_gate/staff（GET/POST/DELETE）、/_gate/staff/grant、
 *           /_gate/projects、/_gate/users（仅管理员）、/_gate/password
 * 三层账号：admin（平台）→ customer（客户老板）→ staff（客户员工）
 */
(function () {
  var ME = null;

  function api(p, opt) {
    return fetch(p, Object.assign({ headers: { 'Content-Type': 'application/json' } }, opt || {}))
      .then(function (r) { return r.json().catch(function () { return {}; }).then(function (j) { return { ok: r.ok, code: r.status, body: j }; }); });
  }
  function esc(s) {
    return String(s == null ? '' : s).replace(/[&<>"']/g, function (c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c];
    });
  }

  /* ── 样式 ── */
  var st = document.createElement('style');
  st.textContent = [
    // 「我的」入口 = 侧栏左上角那个 logo。它的按钮化样式写在 styles.css 里
    // （带 [data-asbudy-my] 前缀，只有脚本真绑上点击才生效）。
    // 2026-09-15：右下角角标、旁边「我的」文字胶囊 —— 都做过，老板说画蛇添足，已拿掉。
    '#asbudy-layer{position:fixed;inset:0;background:rgba(2,7,17,.72);z-index:99999;display:flex;align-items:center;justify-content:center}',
    '.ab-box{background:#0d1117;border:1px solid #30363d;border-radius:12px;width:420px;max-width:90vw;max-height:86vh;display:flex;flex-direction:column;overflow:hidden;box-shadow:0 12px 48px rgba(0,0,0,.6)}',
    '.ab-head{display:flex;justify-content:space-between;align-items:center;padding:12px 18px;border-bottom:1px solid #30363d;gap:14px}',
    '.ab-title{font-size:15px;color:#e6edf3;font-weight:600}',
    '.ab-x{color:#8b949e;cursor:pointer;font-size:14px;border:1px solid #30363d;border-radius:6px;padding:3px 10px;background:transparent}',
    '.ab-x:hover{color:#e6edf3;border-color:#8b949e}',
    '.ab-body{padding:14px 18px;overflow:auto}',
    '.ab-menu-item{display:block;width:100%;text-align:left;padding:12px 14px;border:1px solid #21262d;border-radius:8px;margin-bottom:8px;cursor:pointer;background:transparent;color:#e6edf3;font-size:14px}',
    '.ab-menu-item:hover{border-color:#58a6ff;background:#58a6ff0d}',
    '.ab-menu-item small{display:block;color:#8b949e;font-size:13.5px;margin-top:3px}',
    '.ab-card{border:1px solid #21262d;border-radius:9px;padding:11px 13px;margin-bottom:9px}',
    '.ab-card-top{display:flex;justify-content:space-between;align-items:center;gap:10px}',
    '.ab-n{color:#e6edf3;font-size:14px}',
    '.ab-s{color:#8b949e;font-size:13.5px;margin-top:3px;line-height:1.5}',
    '.ab-btn{background:#238636;color:#fff;border:none;border-radius:7px;padding:7px 14px;font-size:14px;cursor:pointer}',
    '.ab-btn:hover{background:#2ea043}',
    '.ab-btn.ghost{background:transparent;color:#8b949e;border:1px solid #30363d}',
    '.ab-btn.ghost:hover{color:#e6edf3;border-color:#8b949e}',
    '.ab-btn.danger{background:#8b2c2c}.ab-btn.danger:hover{background:#a33}',
    '.ab-btn.sm{padding:4px 10px;font-size:13.5px}',
    '.ab-row{display:flex;gap:8px;align-items:center;margin-bottom:10px}',
    '.ab-row>label{color:#8b949e;font-size:14px;min-width:70px}',
    '.ab-input{flex:1;background:#010409;border:1px solid #30363d;border-radius:7px;color:#e6edf3;padding:7px 10px;font-size:14px;box-sizing:border-box}',
    '.ab-input:focus{outline:none;border-color:#58a6ff}',
    '.ab-chk{display:flex;align-items:center;gap:8px;padding:6px 0;color:#e6edf3;font-size:14px;cursor:pointer}',
    '.ab-tip{color:#8b949e;font-size:13.5px;line-height:1.6;margin-bottom:12px}',
    '.ab-msg{font-size:14px;margin-top:10px;min-height:16px}',
    '.ab-msg.err{color:#f85149}.ab-msg.ok{color:#3fb950}',
    '.ab-chip{display:flex;align-items:center;gap:8px;margin:0 0 8px;padding:6px 10px;border:1px solid #3b7ddd66;background:#3b7ddd14;border-radius:8px;font-size:13.5px;color:#e6edf3;width:fit-content}',
    '.ab-chip b{color:#58a6ff;font-weight:600}',
    '.ab-chip-x{cursor:pointer;color:#8b949e;padding:0 4px;font-size:15px;line-height:1}',
    '.ab-chip-x:hover{color:#f85149}',
    '.fact-chip[data-asbudy-model]{cursor:pointer}',
    '.fact-chip[data-asbudy-model] strong{text-decoration:underline;text-underline-offset:2px;text-decoration-style:dotted}',
    '.fact-chip[data-asbudy-model]:hover strong{color:#58a6ff}',
    '#asbudy-tick{font-size:13.5px;color:#8b949e;padding:0 0 6px 2px}',
    '#asbudy-msgbar{display:flex;flex-wrap:wrap;align-items:center;gap:8px;padding:0 0 6px 2px}',
    '#asbudy-msgbar button{font:inherit;font-size:13.5px;color:#8b949e;background:transparent;border:1px solid #30363d;border-radius:6px;padding:3px 10px;cursor:pointer}',
    '#asbudy-msgbar button:hover{color:#e6edf3}',
  ].join('\n');
  document.head.appendChild(st);

  /* ── 让「我的 → 高级设置」那几个开关在网页版**真的生效**（2026-09-16 老板：「1~5 修成真的」）
   * 为什么以前是空开关：这些偏好官方只喂**终端界面**（TUI）—— settings.toml 里 34 项，
   *   官方网页前端（app.mjs）一个都没读（`settings`/`show_thinking`/`calm_mode`/`cost_currency` 命中 0 次）
   *   → 客户取消勾选、选了美元，界面毫无变化（老板原话：「点了没用比没有更坏」）。
   * 做法：不碰官方代码，把值挂到 <html> 的 data-* 上，由下面几条 CSS 决定显隐；
   *   值本身仍存在引擎里（GET/POST /v1/config），与终端界面看到的一致。
   * ⚠️ 依赖官方 DOM 结构（`article.reasoning` / `.receipt`）—— 官方改结构要跟着改（升级检查清单里有）。
   */
  var DISPLAY = { show_thinking: true, thinking_default_expanded: false, show_tool_details: false, calm_mode: false, cost_currency: 'usd' };

  var stD = document.createElement('style');
  stD.textContent = [
    'html[data-ab-think="off"] article.reasoning{display:none!important}',
    'html[data-ab-calm="on"] article.reasoning{display:none!important}',
    'html[data-ab-tools="off"] .receipt details{display:none!important}',
    'html[data-ab-calm="on"] .receipt details{display:none!important}',
  ].join('\n');
  document.head.appendChild(stD);

  function openReasoning() {
    var list = document.querySelectorAll('article.reasoning details:not([open])');
    for (var i = 0; i < list.length; i++) list[i].open = true;
  }
  var reasonObserver = null;
  function applyDisplayPrefs() {
    var h = document.documentElement;
    h.setAttribute('data-ab-think', DISPLAY.show_thinking ? 'on' : 'off');
    h.setAttribute('data-ab-tools', DISPLAY.show_tool_details ? 'on' : 'off');
    h.setAttribute('data-ab-calm', DISPLAY.calm_mode ? 'on' : 'off');
    if (!DISPLAY.thinking_default_expanded) return;
    openReasoning();
    // 流式追加出来的新思考卡片也要默认展开。只在开关打开时才观察，平时零开销。
    if (!reasonObserver && window.MutationObserver) {
      var pending = false;
      reasonObserver = new MutationObserver(function () {
        if (pending) return;
        pending = true;
        requestAnimationFrame(function () { pending = false; openReasoning(); });
      });
      reasonObserver.observe(document.body, { childList: true, subtree: true });
    }
  }

  api('/v1/config').then(function (r) {
    if (!r.ok) return;                 // 引擎没起 / 读不到 → 用默认（显示思考），不打扰客户
    var c = r.body || {};
    DISPLAY.show_thinking = c.show_thinking !== false;
    DISPLAY.thinking_default_expanded = c.thinking_default_expanded === true;
    DISPLAY.show_tool_details = c.show_tool_details === true;
    DISPLAY.calm_mode = c.calm_mode === true;
    DISPLAY.cost_currency = c.cost_currency === 'cny' ? 'cny' : 'usd';
    applyDisplayPrefs();
  });

  /* ── 浮层 ── */
  function closeLayer() { var o = document.getElementById('asbudy-layer'); if (o) o.remove(); }
  function openLayer(title, build) {
    closeLayer();
    var L = document.createElement('div'); L.id = 'asbudy-layer';
    var box = document.createElement('div'); box.className = 'ab-box';
    var head = document.createElement('div'); head.className = 'ab-head';
    var t = document.createElement('span'); t.className = 'ab-title'; t.textContent = title;
    var x = document.createElement('button'); x.className = 'ab-x'; x.textContent = '关闭'; x.type = 'button';
    head.appendChild(t); head.appendChild(x);
    var body = document.createElement('div'); body.className = 'ab-body';
    box.appendChild(head); box.appendChild(body); L.appendChild(box);
    L.addEventListener('click', function (e) { if (e.target === L) closeLayer(); });
    x.addEventListener('click', closeLayer);
    document.body.appendChild(L);
    build(body);
    return body;
  }
  function msg(el, text, ok) {
    el.className = 'ab-msg ' + (ok ? 'ok' : 'err');
    el.textContent = text;
  }

  /* 当前会话 id —— 自己猴补 fetch 捕获（官方每次选中/新建会话都会 GET /v1/threads/{id}）。
   * ⚠️ 不能复用文件后面那个 LAST_THREAD：它在**另一个 IIFE** 里，作用域不共享（2026-09-15 踩过）。 */
  var MODEL_THREAD = '';
  (function () {
    var prev = window.fetch;
    window.fetch = function (url, opt) {
      var before = MODEL_THREAD;
      var u = '';
      try {
        u = typeof url === 'string' ? url : ((url && url.url) || '');
        var m = u.match(/\/v1\/threads\/([^\/?]+)/);
        if (m && m[1] && m[1] !== 'summary') MODEL_THREAD = m[1];
      } catch (e) {}
      var p = prev.apply(this, arguments);
      // 换了对话 → 把上一条的提示撒掉（**不拿旧结论去猜新对话**，零误报的第一条）
      try { if (MODEL_THREAD !== before) hideFixBar(); } catch (e) {}
      // ★ 只认【引擎给的权威终态】：这一轮 status = "failed" 才算「这条对话出问题了」。
      //   （2026-09-16 老板方案 B：**去掉碰运气式的黄条**，只在真的发不出消息时才给一句话。）
      //   为什么不再看 HTTP 状态码、不再 grep 错误关键词 —— 两次误报都出在那两处：
      //     ① 点「停止」后那条 turn 卡在「正在停止」，紧接着再发消息 → 前端走「插话」(steer)
      //        → 引擎回 400（“is stopping and cannot be steered”）→ 旧规则把任意 /turns 的
      //        4xx 当“坏了” → 误报（2026-09-16 实测复现）；
      //     ② 事件流里任一次工具失败 / 网络抖动都带 error 关键词 → 也误报。
      //   引擎的终态是它自己下的判断：停止=interrupted、正常=completed、真出事=failed。
      //   所以这里改成**解析 SSE 帧、只看 turn.completed 里的 payload.turn.status**（问引擎要真相）。
      //   副本读一份，不影响官方前端。
      try {
        if (p && p.then && /\/v1\/threads\/[^/?]+\/events/.test(u)) {
          p.then(function (r) {
            if (!r || !r.body || typeof r.clone !== 'function') return;
            var copy;
            try { copy = r.clone(); } catch (e) { return; }   // 已被读过就 clone 不了 —— 放过去
            try {
              var rd = copy.body.getReader();
              var dec = new TextDecoder();
              var buf = '';
              var onFrame = function (frame) {
                if (frame.indexOf('turn.completed') < 0) return;
                var lines = frame.split('\n');
                var data = '';
                for (var i = 0; i < lines.length; i++) {
                  if (lines[i].indexOf('data:') === 0) { data = lines[i].slice(5).trim(); break; }
                }
                if (!data) return;
                var o;
                try { o = JSON.parse(data); } catch (e) { return; }
                var st = o && o.payload && o.payload.turn && o.payload.turn.status;
                if (String(st) === 'failed') showFixBar();
              };
              var pump = function () {
                rd.read().then(function (x) {
                  if (x.done) return;
                  buf += dec.decode(x.value, { stream: true });
                  if (buf.length > 1000000) buf = buf.slice(-100000);   // 异常大帧兜底（SSE 帧本应很小）
                  var idx;
                  while ((idx = buf.indexOf('\n\n')) >= 0) {
                    onFrame(buf.slice(0, idx));
                    buf = buf.slice(idx + 2);
                  }
                  pump();
                }).catch(function () {});
              };
              pump();
            } catch (e) {}
          }).catch(function () {});
        }
      } catch (e) {}
      return p;
    };
  })();

  /* ── 对话坏了 → 给一个自己就能点的「修好」──
   * 为什么不做成自动静默修：换过去之后历史不在（实测），客户得知道
   * 「刚才那条对话不在了、这是新的一条」——矞着换过去比报错更吓人。
   * ⚠️ 什么时候弹（2026-09-16 定稿 · 老板方案 B）：**只在引擎说这一轮真 failed 时** ——
   *   看事件流里 turn.completed 帧的 payload.turn.status === "failed"（引擎的权威终态）。
   *   试过、都已撤掉的判定（都会误报）：① 拦 HTTP 状态码（实测失败是 201，拦不到；反过来
   *   「插话被拒」的 400 又被误当成坏了）② 扫事件流里的 error 关键词（工具瞬时失败/网络抖动
   *   都带关键词）③ 定时主动问门卫（靠猜 status）。详见档案 §8.7 85。 */
  function showFixBar() {
    if (document.getElementById('asbudy-fixbar')) return;
    var st = document.createElement('style');
    st.textContent = '#asbudy-fixbar{position:fixed;left:50%;transform:translateX(-50%);top:64px;z-index:99998;'
      + 'display:flex;align-items:center;gap:12px;max-width:92vw;padding:10px 16px;border-radius:10px;'
      + 'background:#3a2a10;border:1px solid #9e6a03;color:#e6edf3;font-size:13.5px;line-height:1.5;'
      + 'box-shadow:0 8px 28px rgba(0,0,0,.55)}'
      + '#asbudy-fixbar button{flex:none;font:inherit;font-size:13.5px;font-weight:600;color:#fff;background:#238636;'
      + 'border:0;border-radius:7px;padding:7px 13px;cursor:pointer}'
      + '#asbudy-fixbar button:hover{background:#2ea043}'
      + '#asbudy-fixbar button:disabled{opacity:.6;cursor:default}';
    document.head.appendChild(st);
    var bar = document.createElement('div');
    bar.id = 'asbudy-fixbar';
    bar.innerHTML = '<span>这条对话出了一点问题（有一处记录没写完）。这个项目的文件、代码、数据都没事。</span>'
      + '<button type="button" id="asbudy-fixbar-go">换一条干净的对话，继续</button>';
    document.body.appendChild(bar);
    var btn = bar.querySelector('#asbudy-fixbar-go');
    btn.onclick = function () {
      if (!MODEL_THREAD) { btn.textContent = '先随便说一句再来'; return; }
      btn.disabled = true;
      btn.textContent = '正在修…';
      fetch('/_gate/thread-repair', {
        method: 'POST', credentials: 'same-origin',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ thread: MODEL_THREAD }),
      }).then(function (r) { return r.json().catch(function () { return {}; }); }).then(function (j) {
        if (!j || !j.ok) {
          btn.disabled = false;
          btn.textContent = '没修成，再试一次';
          alert((j && j.error) || '没修成');
          return;
        }
        if (j.needRepair === false) {
          btn.disabled = false;
          btn.textContent = '再试一次';
          alert('这条对话本身没问题 —— 可能是网络抖了一下，直接再发一次就行');
          return;
        }
        location.href = '/';    // 刷新 → 打开的就是刚换出来的那条干净对话
      }).catch(function (e) {
        btn.disabled = false;
        btn.textContent = '没修成，再试一次';
        alert('没修成：' + ((e && e.message) || e));
      });
    };
  }

  function hideFixBar() {
    var b = document.getElementById('asbudy-fixbar');
    if (b) b.remove();
  }

  // 触发点只剩一个（2026-09-16 定稿）：引擎的 turn 终态 = failed（在上面 window.fetch 包装的
  //   onFrame 里）。停止=interrupted、插话被拒=HTTP 400（不是 turn 终态）、工具瞬时失败但
  //   AI 继续=completed —— 都不弹。换对话时清提示（hideFixBar）。

  /* ── 模型小标签可点（对话区上方的「模型: xxx」——比藏在菜单里好找）──
   * ⚠️ 2026-09-15 重写：以前写死三个名字写进 m0/.env，而**没有任何在跑的代码读那份 .env**
   *（读它的 currentModel() → runChange() → /_gate/change，只有已下线的旧界面在调）→ 假按钮。
   * 现在走官方两条：① 目录 `GET /v1/providers` + `/v1/providers/{id}/models`（分页字段 nextCursor）
   * ② 换模型 `PATCH /v1/threads/{id} {model}` —— **线程级，当前会话下一轮就生效**
   *（实测：PATCH 成 deepseek-v4-pro 后发消息，引擎回 effective_model=deepseek-v4-pro）。
   * 列表**不写死**：只列引擎里配了密钥的提供商（credentialState=configured），
   * 以后配了 claude / kimi 的 key 就自动出现在这里。
   */
  function loadProviderModels(pid) {
    var out = [];
    function step(cursor) {
      var q = '?limit=100' + (cursor ? '&cursor=' + encodeURIComponent(cursor) : '');
      return api('/v1/providers/' + encodeURIComponent(pid) + '/models' + q).then(function (r) {
        var b = r.body || {};
        if (String(b.provider || '') !== pid || !Array.isArray(b.models)) return out;
        out = out.concat(b.models);
        var next = typeof b.nextCursor === 'string' ? b.nextCursor.trim() : '';
        if (next && out.length < 500) return step(next);
        return out;
      });
    }
    return step('');
  }
  /** 换完把顶部那个「模型」小标签就地改掉（官方那块是只读渲染的，不会自己刷） */
  function paintModelChip(model) {
    var chips = document.querySelectorAll('#session-facts .fact-chip[data-asbudy-model] strong');
    for (var i = 0; i < chips.length; i++) chips[i].textContent = model;
  }
  function openModelPicker() {
    var tid = MODEL_THREAD;
    if (!tid) { alert('先在左边点开一个会话，再换模型'); return; }
    api('/v1/providers').then(function (r) {
      var all = (r.body && r.body.providers) || [];
      var ready = all.filter(function (p) { return p && p.id && p.credentialState === 'configured'; });
      var cur = (r.body && r.body.current) || '';
      if (!ready.length) { alert('引擎里还没有配好密钥的模型提供商'); return; }
      api('/v1/threads/' + encodeURIComponent(tid)).then(function (tr) {
        var body = tr.body || {};
        var th = body.thread || body;          // ⚠️ 详情接口把 thread 包在 .thread 里（列表才是裸数组）
        var thProvider = String(th.model_provider_id || th.model_provider || '');
        var thModel = String(th.model || '');
        openLayer('换个模型', function (body) {
          body.innerHTML = '<div class="ab-tip" id="mp-tip">选中的模型，这一轮对话就开始用。</div>' +
            '<div id="mp-list"><div class="ab-tip">正在读模型目录…</div></div>' +
            '<div class="ab-msg" id="mp-msg"></div>';
          var list = body.querySelector('#mp-list');
          var msgEl = body.querySelector('#mp-msg');
          Promise.all(ready.map(function (p) {
            return loadProviderModels(p.id).then(function (models) { return { p: p, models: models }; });
          })).then(function (groups) {
            groups.sort(function (a, b) { return (b.p.id === cur ? 1 : 0) - (a.p.id === cur ? 1 : 0); });
            list.innerHTML = groups.map(function (g) {
              var same = !thProvider || g.p.id === thProvider;
              var head = '<div style="color:#8b949e;font-size:13px;margin:12px 0 6px">' +
                esc(g.p.display_name || g.p.id) + (g.p.id === cur ? '（默认）' : '') + '</div>';
              if (!g.models.length) return head + '<div class="ab-tip">这个提供商没有模型目录</div>';
              return head + g.models.map(function (m) {
                var on = m.id === thModel;
                var sub = [];
                if (!same) sub.push('换提供商要新建会话');
                if (m.image_input === 'supported') sub.push('可看图');
                return '<button class="ab-menu-item" data-m="' + esc(m.id) + '" data-p="' + esc(g.p.id) + '"' +
                  (same ? '' : ' style="opacity:.6"') + '>' + esc(m.id) + (on ? '（当前）' : '') +
                  '<small style="' + (on ? 'color:#58a6ff' : '') + '">' + esc(sub.join(' · ')) + '</small></button>';
              }).join('');
            }).join('');
            list.querySelectorAll('button[data-m]').forEach(function (b) {
              b.onclick = function () {
                var mid = b.getAttribute('data-m');
                var pid = b.getAttribute('data-p');
                if (thProvider && pid !== thProvider) { msg(msgEl, '换提供商要新建会话（当前会话在 ' + thProvider + '）', false); return; }
                api('/v1/threads/' + encodeURIComponent(tid), { method: 'PATCH', body: JSON.stringify({ model: mid }) })
                  .then(function (r2) {
                    if (!r2.ok) { msg(msgEl, (r2.body && r2.body.error) || '换不了', false); return; }
                    paintModelChip(mid);
                    msg(msgEl, '换好了 —— 这一轮就用「' + mid + '」', true);
                    setTimeout(closeLayer, 900);
                  });
              };
            });
          }).catch(function (e) { msg(msgEl, '读模型目录失败：' + ((e && e.message) || e), false); });
        });
      });
    });
  }
  /* ── Provider 小标签：官方显示的是厂商 id（anthropic / moonshot…），换成官方给的友好名 ── */
  var PROVIDER_NAMES = null;
  function loadProviderNames() {
    if (PROVIDER_NAMES) return Promise.resolve(PROVIDER_NAMES);
    return api('/v1/providers').then(function (r) {
      var m = {};
      ((r.body && r.body.providers) || []).forEach(function (p) {
        if (p && p.id) m[p.id] = p.display_name || p.id;
      });
      PROVIDER_NAMES = m;
      return m;
    }).catch(function () { PROVIDER_NAMES = {}; return PROVIDER_NAMES; });
  }
  function bindProviderChip() {
    var chip = document.querySelector('#session-facts .fact-chip[data-fact="provider"] strong');
    if (!chip) return;
    var id = String(chip.textContent || '').trim();
    if (!id) return;
    loadProviderNames().then(function (m) {
      var nice = m[id];
      if (nice && nice !== id && chip.textContent !== nice) {
        chip.title = id;
        chip.textContent = nice;
      }
    });
  }
  function bindModelChip() {
    var chips = document.querySelectorAll('#session-facts .fact-chip');
    for (var i = 0; i < chips.length; i++) {
      var c = chips[i];
      if (c.getAttribute('data-asbudy-model')) continue;
      if (String(c.textContent || '').indexOf('模型') !== 0) continue;
      c.setAttribute('data-asbudy-model', '1');
      c.title = '点这里换模型';
      c.addEventListener('click', openModelPicker);
    }
  }
  // 会话详情是官方动态重建的 → 盯着它
  (function () {
    function watch() {
      var facts = document.getElementById('session-facts');
      if (!facts) return false;
      bindModelChip();
      bindProviderChip();
      new MutationObserver(function () { bindModelChip(); bindProviderChip(); }).observe(facts, { childList: true, subtree: true });
      return true;
    }
    if (!watch()) {
      var n = 0;
      var tm = setInterval(function () { if (watch() || ++n > 60) clearInterval(tm); }, 400);
    }
  })();

  /* ── PWA：把 Service Worker 注册上（资源早就有，一直没注册 → 「装到桌面」是半拉子）── */
  if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('/sw.js').catch(function () { /* 注册失败不影响使用 */ });
  }

  /* ── PWA：浏览器说可以装时记下来，菜单里给个入口 ── */
  var installEvt = null;
  window.addEventListener('beforeinstallprompt', function (e) { e.preventDefault(); installEvt = e; });

  /* ── 平台协助授权（2026-09-15 · 档案附三 §13 原则②）──
     客户本人的开关：开了，平台管理员才能「以你的视角」进来看 / 干活；随时可关。
     为什么必须由客户点：原则是「可见默认关、要有授权」—— 管理员不能自己给自己授权。 */
  function openConsent() {
    api('/_gate/consent').then(function (r) {
      var st = r.body || {};
      openLayer('平台协助', function (body) {
        var on = !!st.granted;
        var html = '<div class="ab-tip">平台要替你排查问题时，需要先「以你的视角」进来（看到的就是你这个界面）。<br>' +
          '<b>只有你能开这个开关，管理员不能替你开</b>；开了随时可以关。</div>';
        html += '<div style="margin:12px 0 14px;padding:10px 12px;border:1px solid ' + (on ? '#3fb950' : '#30363d') +
          ';border-radius:8px">当前状态：<b style="color:' + (on ? '#3fb950' : '#8b949e') + '">' +
          (on ? '已开启' : '未开启') + '</b>' +
          (on && st.until ? '<div style="color:#8b949e;font-size:13.5px;margin-top:4px">有效期到 ' +
            esc(new Date(st.until).toLocaleString()) + '</div>' : '') + '</div>';
        if (on) {
          html += '<button class="ab-menu-item" id="ab-c-off">关闭平台协助<small>关掉后，平台管理员立刻进不来</small></button>';
        } else {
          html += '<button class="ab-menu-item" id="ab-c-on24">开启 24 小时<small>够排查一次问题</small></button>' +
                  '<button class="ab-menu-item" id="ab-c-on168">开启 7 天<small>长期协助（随时可关）</small></button>';
        }
        html += '<div class="ab-tip" id="ab-c-msg" style="margin-top:10px"></div>';
        body.innerHTML = html;
        var m = body.querySelector('#ab-c-msg');
        function post(data) {
          m.textContent = '正在提交…';
          api('/_gate/consent', { method: 'POST', body: JSON.stringify(data) }).then(function (rr) {
            if (!rr.ok) { m.textContent = (rr.body && rr.body.error) || '没成功，再试一次'; return; }
            m.textContent = data.revoke ? '已关闭。' : '已开启。';
            setTimeout(openConsent, 500);   // 重开一层刷新状态（openLayer 会先关旧的）
          });
        }
        var b24 = body.querySelector('#ab-c-on24');
        if (b24) b24.onclick = function () { post({ hours: 24 }); };
        var b168 = body.querySelector('#ab-c-on168');
        if (b168) b168.onclick = function () { post({ hours: 168 }); };
        var bOff = body.querySelector('#ab-c-off');
        if (bOff) bOff.onclick = function () { post({ revoke: true }); };
      });
    });
  }

  /* ── 「我的」菜单 ── */
  function openMyMenu() {
    var role = ME ? ME.role : 'customer';
    openLayer('我的', function (body) {
      var html = '<div class="ab-tip">' + esc(ME ? (ME.name || ME.user) : '') +
        (ME && ME.user ? ' · ' + esc(ME.user) : '') +
        '（' + (role === 'admin' ? '管理员' : role === 'staff' ? '员工' : '客户老板') + '）</div>';
      if (role === 'admin' || role === 'customer') {
        html += '<button class="ab-menu-item" id="ab-m-staff">员工管理<small>给员工建账号、分配可看项目、设项目额度</small></button>';
      }
      if (role === 'admin') {
        html += '<button class="ab-menu-item" id="ab-m-users">客户管理<small>建客户账号、把项目转给客户</small></button>';
      }
      html += '<button class="ab-menu-item" id="ab-m-proj">项目管理<small>暂停（停引擎、省内存）/ 恢复 / 删除</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-auto">定时任务<small>让 AI 按点自己干活（每天 / 每周 / 每月）</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-mem">AI 的记忆<small>它自己记下来的事 —— 你能看，也能清空</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-skills">它会做什么<small>它已经学会的本事 —— 做 PPT / 表格 / 文档 / PDF / 图表…</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-space">空间<small>磁盘用量、每个项目占多少 / 上限多少</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-adv">高级设置<small>接上你自己的代码仓库 / 模型服务 / 只看不改 / 花了多少</small></button>';
      if (role === 'customer') {
        html += '<button class="ab-menu-item" id="ab-m-consent">平台协助<small>让 AsBudy 平台协助你排查问题（只有你能开，随时可关）</small></button>';
      }
      html += '<button class="ab-menu-item" id="ab-m-account">我的账号<small>我自己的名字、登录账号和归属</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-pw">修改密码<small>改自己的登录密码</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-logout">退出登录<small>退出当前账号</small></button>';
      if (installEvt) html += '<button class="ab-menu-item" id="ab-m-install">装到桌面<small>把这个页面装成桌面应用</small></button>';
      body.innerHTML = html;
      abTipPanel(body, 'my-menu', '这里改一次，<b>你所有项目都跟着变</b>（以后新建的也是）。');
      var bStaff = body.querySelector('#ab-m-staff');
      if (bStaff) bStaff.onclick = function () { openStaff(role === 'admin' ? 'admin' : ME.user); };
      var bUsers = body.querySelector('#ab-m-users');
      if (bUsers) bUsers.onclick = openUsers;
      body.querySelector('#ab-m-proj').onclick = openProjects;
      body.querySelector('#ab-m-auto').onclick = openAuto;
      body.querySelector('#ab-m-mem').onclick = openMemory;
      body.querySelector('#ab-m-skills').onclick = openSkills;
      var bSpace = body.querySelector('#ab-m-space');
      if (bSpace) bSpace.onclick = openSpace;
      var bAdv = body.querySelector('#ab-m-adv');
      if (bAdv) bAdv.onclick = openAdvanced;
      var bConsent = body.querySelector('#ab-m-consent');
      if (bConsent) bConsent.onclick = openConsent;
      body.querySelector('#ab-m-account').onclick = openAccount;
      body.querySelector('#ab-m-pw').onclick = openPassword;
      var bIns = body.querySelector('#ab-m-install');
      if (bIns) bIns.onclick = function () { if (installEvt) { installEvt.prompt(); installEvt = null; closeLayer(); } };
      body.querySelector('#ab-m-logout').onclick = function () {
        if (!confirm('退出登录？')) return;
        fetch('/_gate/logout', { method: 'POST', credentials: 'same-origin' })
          .then(function () { location.replace('/login.html'); })
          .catch(function () { location.replace('/login.html'); });
      };

      // 定时任务：有跑完还没看过的新结果 → 直接在菜单项上提醒
      // （老板 2026-09-16：「跑完提醒就在定时任务界面提醒即可」—— 不往外发通知）
      api('/v1/automations').then(function (r) {
        var list = Array.isArray(r.body) ? r.body : [];
        if (!list.length) return;
        var seen = abSeen(), fresh = 0, left = list.length;
        list.forEach(function (a) {
          api('/v1/automations/' + encodeURIComponent(a.id) + '/runs').then(function (rr) {
            var runs = abRunsOf(rr).slice().sort(abByNewest);
            var last = runs[runs.length - 1];
            if (last && last.created_at && (!seen[a.id] || String(last.created_at) > String(seen[a.id]))) fresh++;
            if (--left === 0 && fresh) {
              var it = body.querySelector('#ab-m-auto');
              if (it) {
                var s = it.querySelector('small');
                if (s) s.textContent = '有 ' + fresh + ' 条跑完还没看的结果';
                it.style.borderColor = '#3fb950';
              }
            }
          });
        });
      });
    });
  }

  /* ── 界面版本标记（2026-09-16）─────────────────────────────────────────
   * 为什么要有这个东西：老板报「AI 回复字很大」，我在服务器上用真浏览器量了是正常的，
   * 两边来回扯不清 —— 根子在于**没法确认他浏览器里跑的到底是哪一版界面**
   * （缓存 / 装成桌面应用 / 没刷新，这些都看不出来）。
   * 侧栏底部显示一行带时间的版本号：他一看就知道自己加载的是不是最新，我们也别再猜。
   * ⚠️ 改完界面文件顺手把 UI_BUILD 改掉（就下面这一行）。
   */
  var UI_BUILD = '2026-09-16 16:58';
  function abStampVersion() {
    var foot = document.querySelector('.rail-footer') || document.querySelector('.rail');
    if (!foot || document.getElementById('asbudy-version')) return;
    var el = document.createElement('div');
    el.id = 'asbudy-version';
    el.textContent = '界面版本 ' + UI_BUILD;
    el.style.cssText = 'font-size:12px;opacity:.6;margin-top:8px;letter-spacing:.02em';
    el.title = '这是你浏览器里这份界面的版本时间。若不是最新的，按 Ctrl+Shift+R 强制刷新。';
    foot.appendChild(el);
  }
  setTimeout(abStampVersion, 1500);
  setInterval(abStampVersion, 5000);

  /* ── 员工管理 ── */
  function openStaff(owner) {
    // 管理员点进来 = 管平台自己的员工。要看某个客户的员工，「客户管理」那张卡上就有「看员工」
    // （2026-09-16 老板：别再弹一层「先选归属」）
    if (!owner && ME && ME.role === 'admin') owner = 'admin';
    var q = owner ? ('?owner=' + encodeURIComponent(owner)) : '';
    Promise.all([api('/_gate/projects'), api('/_gate/staff' + q)]).then(function (rs) {
      var projects = (rs[0].body && rs[0].body.projects) || [];
      var staff = (rs[1].body && rs[1].body.staff) || [];
      var title = '员工管理' + (owner === 'admin'
        ? ' · 平台自己的'
        : (owner && ME && owner !== ME.user ? ' · ' + esc(owner) + ' 的员工' : ''));
      var tip = owner === 'admin'
        ? '给平台自己的员工建账号、设「最多能建几个项目」、勾选「能看能操作哪几个项目」。<br>要看某个客户的员工 → 回「我的 → 客户管理」，在那家客户上点「看员工」。'
        : '给员工建账号、设「最多能建几个项目」、勾选「能看能操作哪几个项目」。<br>员工自己建的项目归<b>你名下</b>（你可见可管）；删员工时项目转回你名下，<b>不删项目</b>。';
      openLayer(title, function (body) {
        body.innerHTML =
          '<div class="ab-tip">' + tip + '</div>' +
          '<button class="ab-btn" id="ab-add" type="button">+ 添加员工</button>' +
          '<div id="ab-list" style="margin-top:14px"></div>';
        var listEl = body.querySelector('#ab-list');
        // 表单保存后重开面板（表单里已把浮层关掉，刷底下的 DOM 是刷不到的）
        function reopen() { openStaff(owner); }
        body.querySelector('#ab-add').onclick = function () { openStaffForm(null, owner, projects, staff, reopen); };
        refresh();
        function refresh() {
          api('/_gate/staff' + q).then(function (r) {
            staff = (r.body && r.body.staff) || [];
            if (!staff.length) { listEl.innerHTML = '<div class="ab-tip">还没有员工。</div>'; return; }
            listEl.innerHTML = '';
            staff.forEach(function (s) {
              var card = document.createElement('div'); card.className = 'ab-card';
              var granted = (s.grants || []).map(function (k) {
                var p = projects.filter(function (x) { return x.key === k; })[0];
                return p ? p.name : k;
              });
              card.innerHTML =
                '<div class="ab-card-top"><div><div class="ab-n">' + esc(s.name || s.user) + '</div>' +
                '<div class="ab-s">登录账号：' + esc(s.user) + ' ｜ 项目额度：' + esc(s.quota) + ' 个' +
                ' ｜ 资料空间：' + (s.quotaMb ? esc(s.quotaMb) + ' MB' : '默认') +
                (s.projects && s.projects.length ? ' ｜ 已自建：' + esc(s.projects.join('、')) : '') + '<br>' +
                '可看项目：' + (granted.length ? esc(granted.join('、')) : '<span style="color:#d29922">未分配</span>') + '</div></div></div>';
              var acts = document.createElement('div');
              acts.style.cssText = 'display:flex;gap:7px;margin-top:10px;flex-wrap:wrap';
              var bEdit = document.createElement('button'); bEdit.className = 'ab-btn ghost sm'; bEdit.type = 'button'; bEdit.textContent = '编辑';
              bEdit.onclick = function () { openStaffForm(s, owner, projects, staff, reopen); };
              var bDel = document.createElement('button'); bDel.className = 'ab-btn danger sm'; bDel.type = 'button'; bDel.textContent = '删除';
              bDel.onclick = function () {
                if (!confirm('删掉员工「' + (s.name || s.user) + '」？\nta 建的项目会转回你名下（项目本身不删）。')) return;
                api('/_gate/staff', { method: 'DELETE', body: JSON.stringify({ user: s.user }) }).then(function (r) {
                  if (r.ok) { refresh(); } else { alert(r.body.error || '删除失败'); }
                });
              };
              // 「进 ta 的视角」—— 下级的东西不并排铺在我这儿（附三 §13 原则④）
              var bView = document.createElement('button'); bView.className = 'ab-btn ghost sm'; bView.type = 'button'; bView.textContent = '进 ta 的视角';
              bView.onclick = function () { location.href = '/view-as?as=' + encodeURIComponent(s.user); };
              acts.appendChild(bView);
              acts.appendChild(bEdit); acts.appendChild(bDel);
              card.appendChild(acts);
              listEl.appendChild(card);
            });
          });
        }
      });
    });
  }

  /* ── 员工表单（新建 / 编辑） ── */
  function openStaffForm(rec, owner, projects, staff, onDone) {
    var isNew = !rec;
    openLayer(isNew ? '添加员工' : '编辑员工 · ' + esc(rec.name || rec.user), function (body) {
      body.innerHTML =
        (isNew
          ? '<div class="ab-row"><label>登录账号</label><input class="ab-input" id="f-user" placeholder="字母数字，2~32 位"></div>'
          : '<div class="ab-row"><label>登录账号</label><input class="ab-input" id="f-user" value="' + esc(rec.user) + '" disabled></div>') +
        '<div class="ab-row"><label>名字</label><input class="ab-input" id="f-name" placeholder="显示用，如「小王」" value="' + (isNew ? '' : esc(rec.name || '')) + '"></div>' +
        '<div class="ab-row"><label>密码</label><input class="ab-input" id="f-pw" type="password" placeholder="' + (isNew ? '至少 8 位' : '留空 = 不改') + '"></div>' +
        '<div class="ab-row"><label>项目额度</label><input class="ab-input" id="f-quota" type="number" min="0" max="50" value="' + (isNew ? 2 : esc(rec.quota)) + '" style="max-width:110px"><span style="color:#8b949e;font-size:13.5px">最多能自己建几个项目</span></div>' +
        '<div class="ab-row"><label>资料空间</label><input class="ab-input" id="f-quotamb" type="number" min="0" placeholder="MB，留空 = 默认" value="' + (isNew || !rec.quotaMb ? '' : esc(rec.quotaMb)) + '" style="max-width:130px"><span style="color:#8b949e;font-size:13.5px">ta 能上传多少资料（不能超过你自己的）</span></div>' +
        '<div style="margin:12px 0 6px;color:#e6edf3;font-size:14px">能看能操作的项目' + (isNew ? '（建完再分配也行）' : '') + '</div>' +
        '<div id="f-projs">' + (projects.length
          ? projects.map(function (p) {
              var on = !isNew && (rec.grants || []).indexOf(p.key) >= 0;
              return '<label class="ab-chk"><input type="checkbox" value="' + esc(p.key) + '"' + (on ? ' checked' : '') + '> ' + esc(p.name) + ' <span style="color:#8b949e;font-size:13.5px">（' + esc(p.key) + '）</span></label>';
            }).join('')
          : '<div class="ab-tip">你名下还没有项目。</div>') + '</div>' +
        '<div style="display:flex;gap:8px;margin-top:16px"><button class="ab-btn" id="f-save" type="button">保存</button>' +
        '<button class="ab-btn ghost" id="f-cancel" type="button">取消</button></div>' +
        '<div class="ab-msg" id="f-msg"></div>';

      var msgEl = body.querySelector('#f-msg');
      body.querySelector('#f-cancel').onclick = closeLayer;
      body.querySelector('#f-save').onclick = function () {
        var uname = isNew ? body.querySelector('#f-user').value.trim() : rec.user;
        var name = body.querySelector('#f-name').value.trim();
        var pw = body.querySelector('#f-pw').value;
        var quota = parseInt(body.querySelector('#f-quota').value, 10);
        var quotaMbRaw = body.querySelector('#f-quotamb').value.trim();
        var grants = [];
        body.querySelectorAll('#f-projs input[type=checkbox]').forEach(function (c) { if (c.checked) grants.push(c.value); });
        if (!isNew && !pw && !name) { /* 允许只改配额/授权 */ }
        var payload = { user: uname, displayName: name, quota: quota };
        payload.quotaMb = quotaMbRaw === '' ? 0 : parseInt(quotaMbRaw, 10);   // 0 = 用默认（P2）
        if (owner) payload.owner = owner;
        if (pw) payload.password = pw;
        api('/_gate/staff', { method: 'POST', body: JSON.stringify(payload) }).then(function (r) {
          if (!r.ok) { msg(r.body.error || '保存失败', msgEl); return; }
          api('/_gate/staff/grant', { method: 'POST', body: JSON.stringify({ user: uname, grants: grants }) }).then(function (r2) {
            if (!r2.ok) { msg(r2.body.error || '分配项目失败', msgEl); return; }
            closeLayer();
            if (onDone) onDone();
          });
        });
      };
    });
  }

  /* ── 客户管理（仅管理员） ── */
  function fmtSpace(mb) {
    mb = Number(mb) || 0;
    if (mb >= 1024) return (Math.round(mb / 1024 * 10) / 10) + 'G';
    return mb + 'M';
  }
  /** 给一个客户设「能传多少资料」（2026-09-15 老板要：管理员要在前端就能分配） */
  function openSpaceForm(u, onDone) {
    openLayer('设资料空间 —— ' + (u.name || u.user), function (body) {
      body.innerHTML =
        '<div class="ab-tip">客户上传的资料、以及他回收站里占的空间，加起来不能超过这个数。<br>填写单位是 G；填 0 = 不限制。</div>' +
        '<div class="ab-row"><label>资料空间</label><input class="ab-input" id="sp-g" type="number" min="0" step="0.5" value="'
          + (u.quotaMb ? (Math.round(u.quotaMb / 1024 * 10) / 10) : 1) + '" style="max-width:110px">'
          + '<span style="color:#8b949e;font-size:13.5px">G（当前已用 ' + fmtSpace(u.spaceMb || 0) + '）</span></div>' +
        '<div style="display:flex;gap:8px;margin-top:16px"><button class="ab-btn" id="sp-save" type="button">保存</button>' +
        '<button class="ab-btn ghost" id="sp-cancel" type="button">取消</button></div><div class="ab-msg" id="sp-msg"></div>';
      var msgEl = body.querySelector('#sp-msg');
      body.querySelector('#sp-cancel').onclick = closeLayer;
      body.querySelector('#sp-save').onclick = function () {
        var g = Number(body.querySelector('#sp-g').value || 0);
        if (!(g >= 0)) { msg('填个 0 或正数', msgEl); return; }
        api('/_gate/users', { method: 'POST', body: JSON.stringify({ user: u.user, quotaMb: Math.round(g * 1024) }) })
          .then(function (r) {
            if (!r.ok) { msg((r.body && r.body.error) || '保存失败', msgEl); return; }
            closeLayer(); if (onDone) onDone();
          });
      };
    });
  }
  function openUsers() {
    api('/_gate/users').then(function (r) {
      if (!r.ok) { alert(r.body.error || '打不开'); return; }
      // 只列客户（员工不是“客户”，归到员工管理里看）—— 2026-09-15 P2
      var users = ((r.body && r.body.users) || []).filter(function (u) { return (u.role || 'customer') === 'customer'; });
      openLayer('客户管理', function (body) {
        body.innerHTML =
          '<div class="ab-tip">客户账号 = 一个客户公司。客户老板登录后能自己给员工建账号、分项目。<br>把项目转给客户：在项目上设归属（管理员）。</div>' +
          '<button class="ab-btn" id="ab-add-cust" type="button">+ 添加客户</button>' +
          '<div id="ab-ulist" style="margin-top:14px"></div>';
        var listEl = body.querySelector('#ab-ulist');
        users.forEach(function (u) {
          var card = document.createElement('div'); card.className = 'ab-card';
          card.innerHTML = '<div class="ab-card-top"><div><div class="ab-n">' + esc(u.name || u.user) + '</div>' +
            '<div class="ab-s">账号：' + esc(u.user) + ' ｜ 名下项目：' + ((u.projects && u.projects.length) ? esc(u.projects.join('、')) : '无')
              + ' ｜ 资料已用 ' + fmtSpace(u.spaceMb || 0) + ' / ' + (u.quotaMb ? fmtSpace(u.quotaMb) : '默认 1G') + '</div></div></div>';
          var acts = document.createElement('div'); acts.style.cssText = 'display:flex;gap:7px;margin-top:10px';
          var bSpace = document.createElement('button'); bSpace.className = 'ab-btn ghost sm'; bSpace.type = 'button'; bSpace.textContent = '设资料空间';
          bSpace.onclick = function () { openSpaceForm(u, openUsers); };
          acts.appendChild(bSpace);
          var bStaff = document.createElement('button'); bStaff.className = 'ab-btn ghost sm'; bStaff.type = 'button'; bStaff.textContent = '看员工';
          bStaff.onclick = function () { openStaff(u.user); };
          acts.appendChild(bStaff);
          var bDel = document.createElement('button'); bDel.className = 'ab-btn danger sm'; bDel.type = 'button'; bDel.textContent = '删除';
          bDel.onclick = function () {
            var owns = (u.projects && u.projects.length) ? '（名下还有项目：' + u.projects.join('、') + '，要先转走）' : '';
            if (!confirm('删掉客户「' + (u.name || u.user) + '」？' + owns)) return;
            api('/_gate/users', { method: 'DELETE', body: JSON.stringify({ user: u.user }) }).then(function (r2) {
              if (!r2.ok) { alert((r2.body && r2.body.error) || '删不掉'); return; }
              openUsers();
            });
          };
          acts.appendChild(bDel);
          card.appendChild(acts);
          listEl.appendChild(card);
        });
        body.querySelector('#ab-add-cust').onclick = function () { openCustomerForm(function () { openUsers(); }); };
      });
    });
  }
  function openCustomerForm(onDone) {
    openLayer('添加客户', function (body) {
      body.innerHTML =
        '<div class="ab-row"><label>登录账号</label><input class="ab-input" id="c-user" placeholder="字母数字，2~32 位"></div>' +
        '<div class="ab-row"><label>公司名</label><input class="ab-input" id="c-name" placeholder="显示用，如「XX 公司」"></div>' +
        '<div class="ab-row"><label>密码</label><input class="ab-input" id="c-pw" type="password" placeholder="至少 8 位"></div>' +
        '<div class="ab-row"><label>资料空间</label><input class="ab-input" id="c-space" type="number" min="0" step="0.5" value="1" style="max-width:110px"><span style="color:#8b949e;font-size:13.5px">G，0 = 不限（客户能传多少资料）</span></div>' +
        '<div style="display:flex;gap:8px;margin-top:16px"><button class="ab-btn" id="c-save" type="button">保存</button>' +
        '<button class="ab-btn ghost" id="c-cancel" type="button">取消</button></div><div class="ab-msg" id="c-msg"></div>';
      var msgEl = body.querySelector('#c-msg');
      body.querySelector('#c-cancel').onclick = closeLayer;
      body.querySelector('#c-save').onclick = function () {
        var payload = {
          user: body.querySelector('#c-user').value.trim(),
          displayName: body.querySelector('#c-name').value.trim(),
          password: body.querySelector('#c-pw').value,
          quotaMb: Math.round(Number(body.querySelector('#c-space').value || 0) * 1024),
        };
        api('/_gate/users', { method: 'POST', body: JSON.stringify(payload) }).then(function (r) {
          if (!r.ok) { msg(r.body.error || '保存失败', msgEl); return; }
          closeLayer(); if (onDone) onDone();
        });
      };
    });
  }

  /* ── 导入已有项目（把客户现有的代码 / 系统搬进来）── */

  /* ── 新建项目 ── */

  /* ── 项目管理（暂停 / 恢复：停引擎 = 释放内存） ── */
  function openProjects() {
    // 管理员：项目可转给某个客户；先把客户名单拉回来再开面板
    var isAdmin = !!(ME && ME.role === 'admin');
    var ownersP = isAdmin
      ? api('/_gate/users').then(function (r) { return (r.body && r.body.users) || []; }).catch(function () { return []; })
      : Promise.resolve([]);
    ownersP.then(function (owners) { openProjectsWith(owners); });
  }
  function openProjectsWith(owners) {
    var isAdmin = !!(ME && ME.role === 'admin');
    openLayer('项目管理', function (body) {
      body.innerHTML = '<div class="ab-tip">「暂停」= 停掉引擎、释放内存（每个项目约 50~270MB），暂停期间别人访问会看到提示而不是偷偷启动；要用时点「恢复」（几秒）。</div><div id="ab-plist">加载中…</div>';
      function refresh() {
        api('/_gate/projects').then(function (r) {
          var list = (r.body && r.body.projects) || [];
          var el = body.querySelector('#ab-plist');
          if (!el) return;
          if (!list.length) { el.innerHTML = '<div class="ab-tip">还没有项目。</div>'; return; }
          el.innerHTML = '';
          list.forEach(function (p) {
            var card = document.createElement('div'); card.className = 'ab-card';
            card.innerHTML = '<div class="ab-card-top"><div><div class="ab-n">' + esc(p.name) + '</div>' +
              '<div class="ab-s">' + (p.paused
                ? '<span style="color:#d29922">已暂停（不占内存）</span>'
                : '<span style="color:#3fb950">运行中</span>') +
              (p.editable ? '' : ' ｜ 这个项目没有独立引擎') + '</div></div></div>';
            var acts = document.createElement('div'); acts.style.cssText = 'display:flex;gap:7px;margin-top:10px;flex-wrap:wrap';
            if (p.editable) {
              var b = document.createElement('button');
              b.className = 'ab-btn sm ' + (p.paused ? '' : 'ghost');
              b.type = 'button';
              b.textContent = p.paused ? '恢复' : '暂停（省内存）';
              b.onclick = function () {
                b.disabled = true;
                b.textContent = p.paused ? '恢复中…' : '暂停中…';
                api('/_gate/projects/' + (p.paused ? 'resume' : 'pause'), { method: 'POST', body: JSON.stringify({ key: p.key }) })
                  .then(function (r2) {
                    if (!r2.ok) { alert((r2.body && r2.body.error) || '操作失败'); }
                    refresh();
                  });
              };
              acts.appendChild(b);
            }
            // 工作台不给删（2026-09-16 老板定 A）：它是平台给你的干活入口、不是客户的项目，
            //   删了连里面所有产出文件（PPT/Excel）一起没，也没地方补。后端同时拦着，两层。
            if (p.workbench) {
              var wbHint = document.createElement('div');
              wbHint.className = 'ab-s';
              wbHint.style.cssText = 'align-self:center;color:#8b949e';
              wbHint.textContent = '这是你的工作台，不能删（想省内存可以暂停）';
              acts.appendChild(wbHint);
            } else {
            var bDel = document.createElement('button');
            bDel.className = 'ab-btn danger sm';
            bDel.type = 'button';
            bDel.textContent = '彻底删除';
            bDel.onclick = function () {
              // 2026-09-15：以前这里写的是「默认只下线，要真删得去服务器跑 --purge」——
              // 客户根本做不到，点了删除文件还在。现在点它就是真删（走后端 purge）。
              if (!confirm('彻底删掉项目「' + p.name + '」？\n\n· 代码、数据、引擎记录一起删，找不回来\n· 只想先停掉、以后还要用 → 点「暂停」\n\n确定吗？')) return;
              api('/_gate/projects/delete', { method: 'POST', body: JSON.stringify({ key: p.key, purge: true }) }).then(function (r2) {
                if (!r2.ok) { alert((r2.body && r2.body.error) || '删不掉'); return; }
                // 删干净了没有要说清楚 —— 只下线不删文件时不能装作删了
                if (r2.body && r2.body.note) alert(r2.body.note);
                refresh();
              });
            };
            acts.appendChild(bDel);
            }
            // 管理员：把项目转给某个客户
            if (isAdmin && owners.length) {
              var sel = document.createElement('select');
              sel.className = 'ab-input';
              sel.style.cssText = 'max-width:150px;padding:5px 8px;font-size:13.5px';
              sel.title = '把项目转给某个客户';
              owners.forEach(function (o) {
                var op = document.createElement('option');
                op.value = o.user;
                op.textContent = o.name || o.user;
                if (o.user === p.owner) op.selected = true;
                sel.appendChild(op);
              });
              sel.onchange = function () {
                api('/_gate/projects/owner', { method: 'POST', body: JSON.stringify({ key: p.key, owner: sel.value }) })
                  .then(function (r2) {
                    if (!r2.ok) { alert((r2.body && r2.body.error) || '转不了'); }
                    refresh();
                  });
              };
              acts.appendChild(sel);
            }
            card.appendChild(acts);
            el.appendChild(card);
          });
        });
      }
      refresh();
    });
  }


  /* ── 空间（用量 / 配额 / 整盘） ── */
  function fmtMb(mb) {
    if (mb == null) return '—';
    if (mb >= 1024) return (mb / 1024).toFixed(1) + ' GB';
    return mb + ' MB';
  }
  function openSpace() {
    openLayer('空间', function (body) {
      body.innerHTML = '<div id="ab-space">加载中…</div>';
      api('/_gate/usage?force=1').then(function (r) {
        var d = r.body || {};
        var el = body.querySelector('#ab-space');
        if (!el) return;
        var html = '';
        if (d.disk) {
          html += '<div class="ab-card"><div class="ab-n">服务器磁盘</div>' +
            '<div class="ab-s">可用 ' + fmtMb(d.disk.availMb) + ' / 共 ' + fmtMb(d.disk.totalMb) +
            '（已用 ' + Math.round((d.disk.usedMb / d.disk.totalMb) * 100) + '%）</div></div>';
        }
        html += '<div class="ab-tip">每个项目默认上限 ' + fmtMb(d.defaultQuotaMb) + '；满了之后上传和改动会被拦下。</div>';
        if (!(d.projects || []).length) html += '<div class="ab-tip">名下还没有项目。</div>';
        (d.projects || []).forEach(function (p) {
          var bar = '<div style="height:6px;background:#21262d;border-radius:3px;margin-top:8px;overflow:hidden">' +
            '<div style="height:100%;width:' + Math.min(100, p.pct || 0) + '%;background:' + (p.over ? '#f85149' : '#3fb950') + '"></div></div>';
          html += '<div class="ab-card"><div class="ab-n">' + esc(p.name) + '</div>' +
            '<div class="ab-s">已用 ' + fmtMb(p.totalMb) + ' / ' + fmtMb(p.quotaMb) + '（' + (p.pct || 0) + '%）' +
            (p.over ? ' <span style="color:#f85149">⚠️ 满了，先清一下</span>' : '') + '</div>' + bar +
            (p.parts || []).map(function (x) {
              return '<div class="ab-s">· ' + esc(x.what) + '：' + (x.mb == null ? '读不到' : fmtMb(x.mb)) + '</div>';
            }).join('') +
            '</div>';
        });
        el.innerHTML = html;
      });
    });
  }

  /* ── 定时任务（搬表：官方 /v1/automations 整族，官方 web 没界面）──
   * 2026-09-16。引擎侧 7 条路由先实测通了一遍（建 → 立刻跑 → 查到 completed → 删）。
   * 官方 CLI 用 RRULE 表达时间（`FREQ=CRON;EXPR=0 9 * * *`）—— 对不懂编程的老板是天书，
   * 所以这里做**人话 ↔ RRULE 双向翻译**：界面选「每天 / 每周 / 每月 / 每小时」+ 时间。
   * 引擎支持的时间格式（读源码 runtime_api/automation_manager）：FREQ=ONCE|HOURLY|WEEKLY|CRON，
   * CRON 是标准 5 段（分 时 日 月 周），**按本地时间**解释（系统北京时间）。
   * ⚠️ 自动化 = 到点自己动手、不等你确认（引擎侧 auto_approve）—— 界面上必须明说。
   */
  var AB_WEEK = [[1, '一'], [2, '二'], [3, '三'], [4, '四'], [5, '五'], [6, '六'], [0, '日']];
  var AB_DOW = { 0: '日', 1: '一', 2: '二', 3: '三', 4: '四', 5: '五', 6: '六', 7: '日' };

  function ab2(n) { return String(n).padStart(2, '0'); }

  /* 存的是 UTC、给人看一律本地时间（2026-09-15 ㉔ 定的统一规矩） */
  function abLocal(iso) {
    if (!iso) return '';
    var d = new Date(iso);
    if (isNaN(d.getTime())) return '';
    return (d.getMonth() + 1) + '月' + d.getDate() + '日 ' + ab2(d.getHours()) + ':' + ab2(d.getMinutes());
  }

  /* RRULE → 人话（看不懂的格式就原样显示，不硬编） */
  function abRruleHuman(s) {
    var u = String(s || '').toUpperCase().trim();
    var m = u.match(/^FREQ=CRON\s*;\s*EXPR=(.+)$/);
    if (m) {
      var f = m[1].trim().split(/\s+/);
      if (f.length === 5) {
        var mi = f[0], hh = f[1], dom = f[2], dow = f[4];
        var at = (/^\d+$/.test(hh) && /^\d+$/.test(mi)) ? (ab2(+hh) + ':' + ab2(+mi)) : '';
        var dows = dow === '*' ? '' : dow.split(',').map(function (x) {
          var n = parseInt(x, 10);
          return isNaN(n) ? x : '周' + (AB_DOW[n] || x);
        }).join('、');
        if (dom === '*' && dow === '*') return hh === '*' ? ('每小时第 ' + mi + ' 分钟') : ('每天 ' + at);
        if (dom === '*' && dow !== '*') return '每' + dows + (at ? ' ' + at : '');
        if (dom !== '*' && dow === '*') return '每月 ' + dom + ' 号' + (at ? ' ' + at : '');
      }
      return '按计划（' + m[1] + '）';
    }
    if (/^FREQ=HOURLY/.test(u)) {
      var iv = (u.match(/INTERVAL=(\d+)/) || [])[1] || '1';
      var bm = (u.match(/BYMINUTE=(\d+)/) || [])[1];
      return '每 ' + iv + ' 小时' + (bm != null ? '（第 ' + bm + ' 分钟）' : '');
    }
    if (/^FREQ=ONCE/.test(u)) {
      var at2 = (u.match(/AT=([^;]+)/) || [])[1];
      return at2 ? ('只跑一次（' + abLocal(at2) + '）') : '只跑一次';
    }
    if (/^FREQ=WEEKLY/.test(u)) {
      var bd = (u.match(/BYDAY=([^;]+)/) || [])[1] || '';
      var bh = (u.match(/BYHOUR=(\d+)/) || [])[1];
      var bmi = (u.match(/BYMINUTE=(\d+)/) || [])[1];
      var names = { MO: '一', TU: '二', WE: '三', TH: '四', FR: '五', SA: '六', SU: '日' };
      var ds = bd.split(',').map(function (x) { return names[x] || x; }).join('、');
      return '每周' + ds + (bh != null ? ' ' + ab2(+bh) + ':' + ab2(+(bmi || 0)) : '');
    }
    return s || '';
  }

  /* 人话表单 → RRULE */
  function abRrule(freq, time, days, dom) {
    var t = String(time || '09:00').split(':');
    var hh = parseInt(t[0], 10); var mi = parseInt(t[1], 10);
    if (isNaN(hh)) hh = 9;
    if (isNaN(mi)) mi = 0;
    if (freq === 'hourly') return 'FREQ=HOURLY;INTERVAL=1;BYMINUTE=' + mi;
    if (freq === 'weekly') return 'FREQ=CRON;EXPR=' + mi + ' ' + hh + ' * * ' + ((days && days.length) ? days.join(',') : '1');
    if (freq === 'monthly') return 'FREQ=CRON;EXPR=' + mi + ' ' + hh + ' ' + (parseInt(dom, 10) || 1) + ' * *';
    return 'FREQ=CRON;EXPR=' + mi + ' ' + hh + ' * * *';
  }

  function abRunState(s) {
    return { completed: '跑成了', failed: '失败了', queued: '排队中', running: '正在跑', canceled: '已取消' }[s] || s || '';
  }

  /* 跑完的提醒「看一次就消」—— 记在本机 localStorage，按任务 id 存「我上次看到的最新一次运行」。
   * 老板 2026-09-16 定：「跑完提醒就在定时任务界面提醒即可」—— 不往外发通知，就在这个界面里看。
   * ⚠️ 放本机（不是服务端）：它只是「我还没翻过」的提示，不是审计记录；换设备重新看过一次无妨。
   */
  var AB_SEEN_KEY = 'ab-auto-seen';
  function abSeen() {
    try { return JSON.parse(localStorage.getItem(AB_SEEN_KEY) || '{}') || {}; } catch (e) { return {}; }
  }
  function abSeenSave(m) { try { localStorage.setItem(AB_SEEN_KEY, JSON.stringify(m)); } catch (e) {} }
  function abByNewest(x, y) { return String(x.created_at || '').localeCompare(String(y.created_at || '')); }
  function abRunsOf(r) { return Array.isArray(r.body) ? r.body : ((r.body && r.body.runs) || []); }

  /* 一行状态：时间 + 结果 + 没看过的标「新结果」；失败额外说一句 */
  function abPaintRun(box, run, seenIso) {
    var st = run.status || '';
    var fresh = run.created_at && (!seenIso || String(run.created_at) > String(seenIso));
    var html = '上次：' + esc(abLocal(run.created_at)) + ' ' + esc(abRunState(st));
    if (fresh) html += ' <span style="color:#3fb950">● 新结果</span>';
    if (st === 'failed') html += ' <span style="color:#f85149">—— 没干成，进去看一眼</span>';
    else if (fresh && st === 'completed') html += ' <span style="color:#8b949e">—— 干完了</span>';
    box.innerHTML = html;
  }

  /* 结果一句话：把跑出来的那个会话的开头拿出来（看不到就不显示，不编内容）
   * ⚠️ 2026-09-16 踩过：`GET /v1/threads/{id}` 返回的是 `{thread,turns,items}`，**没有 preview/title**；
   *   摘要只在 `GET /v1/threads/summary` 里 —— 拿 {id} 取 preview 永远是 undefined。
   *   多条卡片共用一次 summary（5 秒缓存），避免 N 条主 N 次请求。
   */
  var AB_SUM_CACHE = { at: 0, list: [] };
  function abThreadSummary(cb) {
    if (AB_SUM_CACHE.at && Date.now() - AB_SUM_CACHE.at < 5000) return cb(AB_SUM_CACHE.list);
    api('/v1/threads/summary?limit=50').then(function (r) {
      var list = Array.isArray(r.body) ? r.body : ((r.body && r.body.threads) || []);
      AB_SUM_CACHE = { at: Date.now(), list: list };
      cb(list);
    });
  }

  function abPaintResult(card, id, threadId) {
    if (!card || !threadId) return;
    var box = card.querySelector('#au-res-' + id);
    if (!box) return;
    abThreadSummary(function (list) {
      var hit = list.filter(function (t) { return t.id === threadId; })[0];
      var pv = hit && (hit.preview || hit.title);
      if (pv) box.textContent = '它说：' + String(pv).replace(/\s+/g, ' ').slice(0, 92);
    });
  }

  /* ── 定时任务：列表 ── */
  function openAuto() {
    openLayer('定时任务', function (body) {
      body.innerHTML = '<div id="ab-auto">加载中…</div>';
      Promise.all([api('/_gate/projects'), api('/v1/automations')]).then(function (rs) {
        var el = body.querySelector('#ab-auto');
        if (!el) return;
        var projs = (rs[0].body && rs[0].body.projects) || [];
        var act = projs.filter(function (p) { return p.active; })[0] || projs[0] || null;
        var list = Array.isArray(rs[1].body) ? rs[1].body : ((rs[1].body && rs[1].body.automations) || []);
        if (!rs[1].ok) {
          el.innerHTML = '<div class="ab-tip" style="color:#f85149">读不到定时任务：' +
            esc((rs[1].body && rs[1].body.error) || ('HTTP ' + rs[1].code)) + '</div>';
          return;
        }
        if (!act) {
          el.innerHTML = '<div class="ab-tip">名下还没有项目，先建一个项目再来。</div>';
          return;
        }
        el.innerHTML =
          '<div class="ab-tip">到点它会<b>自己动手</b>（改文件、跑命令都不用你确认），干完记在「' +
          esc(act.name) + '」的会话里。</div>' +
          '<button class="ab-menu-item" id="ab-au-new">+ 新建定时任务<small>每天 / 每周 / 每月 / 每小时</small></button>' +
          '<div id="ab-au-list">' + (list.length ? '' : '<div class="ab-tip">还没有定时任务。</div>') + '</div>';
        el.querySelector('#ab-au-new').onclick = function () {
          openAutoForm(act.dir, function () { openAuto(); });
        };
        if (!list.length) return;

        var holder = el.querySelector('#ab-au-list');
        holder.innerHTML = list.map(function (a) {
          return '<div class="ab-card" id="au-' + esc(a.id) + '">' +
            '<div class="ab-card-top"><span class="ab-n">' + esc(a.name || '（没名字）') + '</span>' +
            '<span class="ab-s" style="margin:0;color:' + (a.status === 'active' ? '#3fb950' : '#d29922') + '">' +
            (a.status === 'active' ? '启用中' : '已暂停') + '</span></div>' +
            '<div class="ab-s">' + esc(abRruleHuman(a.rrule)) +
            (a.next_run_at ? ' · 下次 ' + esc(abLocal(a.next_run_at)) : '') + '</div>' +
            '<div class="ab-s" id="au-run-' + esc(a.id) + '">上次：查中…</div>' +
            '<div class="ab-s" id="au-res-' + esc(a.id) + '" style="color:#8b949e"></div>' +
            '<div class="ab-s" style="color:#8b949e">要它做的：' + esc((a.prompt || '').slice(0, 110)) +
            ((a.prompt || '').length > 110 ? '…' : '') + '</div>' +
            '<div style="display:flex;gap:8px;margin-top:9px;flex-wrap:wrap">' +
            '<button class="ab-btn sm" data-act="run" data-id="' + esc(a.id) + '">立刻跑一次</button>' +
            '<button class="ab-btn ghost sm" data-act="' + (a.status === 'active' ? 'pause' : 'resume') +
            '" data-id="' + esc(a.id) + '">' + (a.status === 'active' ? '暂停' : '恢复') + '</button>' +
            '<button class="ab-btn danger sm" data-act="del" data-id="' + esc(a.id) + '">删除</button></div></div>';
        }).join('');

        // 每条：拉最近一次运行 + 结果一句话 + 「跑完还没看过」的标
        var seenBefore = abSeen(), seenAfter = Object.assign({}, seenBefore);
        list.forEach(function (a) {
          var card = body.querySelector('#au-' + a.id);
          api('/v1/automations/' + encodeURIComponent(a.id) + '/runs').then(function (r) {
            var box = body.querySelector('#au-run-' + a.id);
            if (!box) return;
            var runs = abRunsOf(r).slice().sort(abByNewest);
            if (!runs.length) { box.textContent = '上次：还没跑过'; return; }
            var last = runs[runs.length - 1];
            if (last.created_at) seenAfter[a.id] = last.created_at;
            abPaintRun(box, last, seenBefore[a.id]);
            if (last.status === 'completed') abPaintResult(card, a.id, last.thread_id);
          });
        });
        // 「新结果」标本次仍显示（让客户至少看见一次），两秒后再记成「看过了」
        setTimeout(function () { abSeenSave(seenAfter); }, 2000);

        holder.onclick = function (e) {
          var b = e.target.closest ? e.target.closest('button[data-act]') : null;
          if (!b) return;
          var id = b.getAttribute('data-id'); var act2 = b.getAttribute('data-act');
          var rec = list.filter(function (x) { return x.id === id; })[0] || {};
          if (act2 === 'del') {
            if (!confirm('删掉「' + (rec.name || '这个定时任务') + '」？\n（已经记下的历史会话不会删）')) return;
            b.disabled = true;
            api('/v1/automations/' + encodeURIComponent(id), { method: 'DELETE' }).then(function () { openAuto(); });
            return;
          }
          if (act2 === 'pause' || act2 === 'resume') {
            b.disabled = true;
            api('/v1/automations/' + encodeURIComponent(id) + '/' + act2, { method: 'POST' }).then(function () { openAuto(); });
            return;
          }
          // 立刻跑一次 —— 排上队后轮询到跑完（老板 2026-09-16：结果就在这个界面里给）
          b.disabled = true; b.textContent = '正在跑…';
          api('/v1/automations/' + encodeURIComponent(id) + '/run', { method: 'POST' }).then(function (r) {
            if (!r.ok) { b.disabled = false; b.textContent = '立刻跑一次'; alert('没跑起来：' + ((r.body && r.body.error) || ('HTTP ' + r.code))); return; }
            var runId = (r.body && r.body.id) || '';
            var card = body.querySelector('#au-' + id);
            var tries = 0;
            var box0 = body.querySelector('#au-run-' + id);
            if (box0) box0.textContent = '正在跑…（干完这里会自己更新）';
            var timer = setInterval(function () {
              tries++;
              api('/v1/automations/' + encodeURIComponent(id) + '/runs').then(function (rr) {
                var runs = abRunsOf(rr).slice().sort(abByNewest);
                var cur = runs.filter(function (x) { return !runId || x.id === runId; }).pop() || runs[runs.length - 1];
                var box = body.querySelector('#au-run-' + id);
                if (!cur || !box) return;
                abPaintRun(box, cur, null);          // 刚发生的，按「新」显示
                var done = cur.status === 'completed' || cur.status === 'failed' || cur.status === 'canceled';
                if (cur.status === 'completed') abPaintResult(card, id, cur.thread_id);
                if (done || tries >= 40) {
                  clearInterval(timer);
                  b.disabled = false; b.textContent = '立刻跑一次';
                  if (cur.created_at) {
                    var m = abSeen(); m[id] = cur.created_at; abSeenSave(m);   // 就在眼前发生的，不用再标「新」
                  }
                }
              });
            }, 3000);
          });
        };
      });
    });
  }

  /* ── 定时任务：新建 ── */
  function openAutoForm(dir, onDone) {
    openLayer('新建定时任务', function (body) {
      var dayBox = AB_WEEK.map(function (w) {
        return '<label class="ab-chk" style="display:inline-flex;margin:0 12px 0 0"><input type="checkbox" value="' +
          w[0] + '"' + (w[1] === '一' ? ' checked' : '') + '>周' + w[1] + '</label>';
      }).join('');
      body.innerHTML =
        '<div class="ab-tip">到点它会自己动手 —— <b>改文件、跑命令都不再问你</b>，干完写进这个项目的会话里。' +
        '拿不准就先写「只看不动」的活（比如「把逾期清单写成报告」）。</div>' +
        '<div class="ab-row"><label>叫什么</label><input class="ab-input" id="au-name" placeholder="例：每天早上看逾期款"></div>' +
        '<div class="ab-row"><label>多久一次</label><select class="ab-input" id="au-freq">' +
        '<option value="daily">每天</option><option value="weekly">每周</option>' +
        '<option value="monthly">每月</option><option value="hourly">每小时</option></select></div>' +
        '<div class="ab-row"><label>几点</label><input class="ab-input" id="au-time" type="time" value="09:00"></div>' +
        '<div class="ab-row" id="au-days" style="display:none"><label>周几</label><div style="flex:1">' + dayBox + '</div></div>' +
        '<div class="ab-row" id="au-dom" style="display:none"><label>几号</label><input class="ab-input" id="au-domv" type="number" min="1" max="31" value="1"></div>' +
        '<div class="ab-row" style="align-items:flex-start"><label>要它做什么</label>' +
        '<textarea class="ab-input" id="au-prompt" rows="4" placeholder="例：看 data/app.db 里逾期没付的订单，把清单写进 reports/逾期.md，并回我一句话总结"></textarea></div>' +
        '<div style="display:flex;gap:8px;margin-top:14px"><button class="ab-btn" id="au-save" type="button">建好</button>' +
        '<button class="ab-btn ghost" id="au-cancel" type="button">取消</button></div><div class="ab-msg" id="au-msg"></div>';

      var freqEl = body.querySelector('#au-freq');
      var msgEl = body.querySelector('#au-msg');
      function syncRows() {
        var f = freqEl.value;
        body.querySelector('#au-days').style.display = f === 'weekly' ? '' : 'none';
        body.querySelector('#au-dom').style.display = f === 'monthly' ? '' : 'none';
        body.querySelector('#au-time').parentNode.style.display = f === 'hourly' ? 'none' : '';
      }
      freqEl.onchange = syncRows;
      syncRows();
      body.querySelector('#au-cancel').onclick = closeLayer;
      body.querySelector('#au-save').onclick = function () {
        var prompt = body.querySelector('#au-prompt').value.trim();
        if (!prompt) { msg(msgEl, '还没写「要它做什么」', false); return; }
        var days = Array.prototype.slice.call(body.querySelectorAll('#au-days input:checked')).map(function (c) { return c.value; });
        if (freqEl.value === 'weekly' && !days.length) { msg(msgEl, '每周的至少选一天', false); return; }
        var name = body.querySelector('#au-name').value.trim() || prompt.slice(0, 20);
        var payload = {
          name: name,
          prompt: prompt,
          rrule: abRrule(freqEl.value, body.querySelector('#au-time').value, days, body.querySelector('#au-domv').value),
          cwds: dir ? [dir] : [],
          mode: 'agent',
          allow_shell: true,
          auto_approve: true,
        };
        var btn = body.querySelector('#au-save');
        btn.disabled = true;
        msg(msgEl, '正在建…', true);
        api('/v1/automations', { method: 'POST', body: JSON.stringify(payload) }).then(function (r) {
          if (!r.ok) {
            btn.disabled = false;
            msg(msgEl, ((r.body && (r.body.error || r.body.message)) || ('没建成（HTTP ' + r.code + '）')) + '', false);
            return;
          }
          closeLayer();
          if (onDone) onDone();
        });
      };
    });
  }

  /* ── 「它会做什么」：把引擎的本事清单搬给客户看（2026-09-16 · 搬表：`GET /v1/skills`）──
   * 为什么做：引擎里 38 项技能（做 PPT / 表格 / Word / PDF / 图表 / 联网查资料…），
   *   官方 web 界面**一条都没接** —— 客户既不知道它有什么用，也不敢把活交给它。
   * 为什么是**只读清单、不给装**：官方 `POST /v1/skills/install` 要填的是 `github:owner/repo`
   *   或网址（那是给开发者的口子），客户填不来；我们的做法是平台把技能统一挂公共目录
   *   （引擎家里的 skills 软链 → `/opt/asbudy/share/skills`）→ 客户项目**全都有**。
   *   所以这个面板只回答一句话：「它到底会哪些本事」。
   * ⚠️ 中文名是我们自己加的（官方描述多是英文）；**表里没列的技能一律落进「其他能力」显示原名** ——
   *   官方以后加技能不会丢，也不会因为漏翻译就消失；而表里列了但引擎当前没装的，直接不显示。
   */
  var SKILL_GROUPS = [
    ['做文件', [
      ['pptx', '做 PPT', '按你说的做幻灯片、改版式、配图'],
      ['presentations', '做 PPT（另一套工具）', '同上；版式复杂时换它做更稳'],
      ['xlsx', '做 Excel 表格', '建表、算公式、清洗数据、导出'],
      ['spreadsheets', '做表格（另一套工具）', '同上；CSV / TSV 也能处理'],
      ['docx', '做 Word 文档', '写文档、改格式、套模板'],
      ['documents', '做 Word（另一套工具）', '同上；合同、通知、报告都行'],
      ['pdf', '处理 PDF', '拆开、合并、旋转、加水印、提文字、识扫描件'],
      ['dataviz', '做图表', '把数据画成图、做看板，让人一眼看懂'],
      ['document', '写说明文档', '整理使用说明这类文档'],
    ]],
    ['查资料 / 对接别的系统', [
      ['research', '上网查资料', '联网找最新信息，并给出来源'],
      ['feishu', '对接飞书', '飞书机器人、云文档、表格、审批流'],
      ['alapi', '对接 ALAPI 接口', '需要调 ALAPI 平台的接口时'],
    ]],
  ];

  function abSkillCard(title, desc, sk) {
    return '<div class="ab-card"><div class="ab-card-top"><span class="ab-n">' + esc(title) + '</span>' +
      (sk && sk.enabled === false ? '<span class="ab-s">已关</span>' : '') + '</div>' +
      '<div class="ab-s">' + esc(desc || '') + '</div></div>';
  }

  function openSkills() {
    openLayer('它会做什么', function (body) {
      body.innerHTML = '<div id="ab-skills"><div class="ab-tip">正在问它…</div></div>';
      api('/v1/skills').then(function (r) {
        var el = body.querySelector('#ab-skills');
        if (!el) return;
        if (!r.ok) {
          el.innerHTML = '<div class="ab-tip">读不到清单：' + esc((r.body && r.body.error) || r.code) + '</div>';
          return;
        }
        var list = ((r.body || {}).skills || []);
        var known = {};
        var html = '<div class="ab-tip">这些是它<b>已经学会</b>的本事 —— <b>你不用挑、也不用点</b>：' +
          '直接跟它说要做什么，它自己会挑合适的用。</div>';
        SKILL_GROUPS.forEach(function (g) {
          var rows = '';
          g[1].forEach(function (it) {
            var sk = null;
            for (var i = 0; i < list.length; i++) if (list[i].name === it[0]) { sk = list[i]; break; }
            if (!sk) return;                     // 引擎当前没装 → 不显示（表是死的，清单是活的）
            known[it[0]] = 1;
            rows += abSkillCard(it[1], it[2], sk);
          });
          if (rows) html += '<div style="margin:12px 0 6px;color:#e6edf3;font-size:14px">' + esc(g[0]) + '</div>' + rows;
        });
        var rest = [];
        for (var j = 0; j < list.length; j++) if (!known[list[j].name]) rest.push(list[j]);
        if (rest.length) {
          html += '<div style="margin-top:14px"><button class="ab-btn ghost sm" id="ab-sk-more" type="button">还有 ' +
            rest.length + ' 项别的本事（点开看）</button></div><div id="ab-sk-rest" hidden>' +
            rest.map(function (s) { return abSkillCard(s.name, (s.description || '').slice(0, 120), s); }).join('') + '</div>';
        }
        html += '<div class="ab-tip" style="margin-top:14px">想让它多会一样？跟平台说一声就行 —— 你自己不用装。</div>';
        el.innerHTML = html;
        var more = el.querySelector('#ab-sk-more');
        if (more) more.onclick = function () {
          var box = el.querySelector('#ab-sk-rest');
          var wasHidden = box.hidden;
          box.hidden = !wasHidden;
          more.textContent = wasHidden ? '收起' : ('还有 ' + rest.length + ' 项别的本事（点开看）');
        };
      });
    });
  }

  /* ── AI 的记忆（搬表：官方 /v1/memory，官方 web 没界面）──
   * 2026-09-16。引擎侧先实测走通一整条：写一条 → 列出来 → 起一轮对话问它，
   * 它照着记忆回答（问「我们习惯怎么说客户」，答「客户，不说『顾客』」）→ 清空。
   * ⚠️ 官方只有**整批清空**（`DELETE /v1/memory?scope=all|global|workspace`），**没有单条删除**
   *   （路由表里就 GET/POST/DELETE 在集合上）—— 界面上得如实说，并给替代办法。
   * ⚠️ 我们的架构是**一项目一引擎一 HOME** → 这里的记忆只作用于当前项目，不会串到别的项目。
   */
  function abScopeName(s) { return s === 'workspace' ? '本项目' : '通用'; }

  /* 记忆开关：客户自己控制（老板 2026-09-16：「用户自己不能设置吗？」）
   * 官方给的就是配置接口，引擎自己有权限写 config.toml —— 不需要平台介入、不需要 sudo：
   *   POST /v1/config {key:"memory_enabled", value:"true|false", persist:true}
   *   POST /v1/config/reload     ← 官方说明：新的一轮对话会采用新配置（不影响正在跑的）
   */
  function abSetMemory(on, btn, done) {
    if (btn) { btn.disabled = true; btn.textContent = on ? '正在打开…' : '正在关掉…'; }
    var m = document.getElementById('mem-msg');
    // 记忆是「引擎启动时才读」的（remember 工具那时才注册）—— 门卫会在保存后重启本项目的 AI 才真生效，
    // 这里如实告诉客户要等几秒（否则客户会以为「点了没反应」—— 2026-09-16 实测踩到）。
    if (m) { m.className = 'ab-msg'; m.textContent = '正在重启这个项目的 AI（几秒钟），好让设置真生效…'; }
    // 记忆也是**全局偏好**（2026-09-16 老板点名：「包括记忆也是」）——
    // 一次设置，名下所有项目都生效（没在跑的项目等下次打开时自动补）。
    api('/_gate/prefs', {
      method: 'POST',
      body: JSON.stringify({ prefs: { memory_enabled: on ? 'true' : 'false' } }),
    }).then(function (r) {
      if (!r.ok || (r.body && r.body.ok === false)) {
        if (btn) { btn.disabled = false; btn.textContent = on ? '打开记忆' : '关掉记忆'; }
        var why = (r.body && (r.body.error || r.body.message)) || ('HTTP ' + r.code);
        if (m) { m.className = 'ab-msg err'; m.textContent = '没设置成：' + why; }
        return;
      }
      if (m) {
        if (r.body && r.body.restarted && r.body.restarted.ok === false) {
          m.className = 'ab-msg err';
          m.textContent = '设置存下了，但这个项目的 AI 没能重启（' + (r.body.restarted.error || '') + '）—— 得让平台看一眼才能真生效。';
        } else {
          m.className = 'ab-msg ok';
          m.textContent = '好了 —— 本项目的 AI 已重启，设置已经生效。';
        }
      }
      if (done) done();
    });
  }

  function openMemory() {
    openLayer('AI 的记忆', function (body) {
      body.innerHTML = '<div id="ab-mem">加载中…</div>';
      var q = '';
      var entries = [];
      var debounce = null;

      /* ⚠️ 搜索在前端做子串过滤，**不用引擎的 q**：2026-09-16 实测，引擎的 FTS 对中文
       * 基本搜不出来 —— 文本里明明有「人民币」，`?q=人民币` → 0 条；ASCII 的 `?q=RMB` → 1 条。
       * 所以整批拉下来（上限 200）在浏览器里过滤，中文才搜得到。 */
      function renderList() {
        var box = body.querySelector('#mem-list');
        if (!box) return;
        var key = q.trim().toLowerCase();
        var list = key ? entries.filter(function (e) {
          return String(e.summary || '').toLowerCase().indexOf(key) >= 0;
        }) : entries;
        box.innerHTML = list.length ? list.map(function (e) {
          return '<div class="ab-card">' +
            '<div class="ab-n">' + esc(e.summary || '（空白）') + '</div>' +
            '<div class="ab-s">' + esc(abScopeName(e.scope)) +
            (e.line_start != null ? ' · 出自 MEMORY.md 第 ' + esc(e.line_start) + ' 行' : '') +
            (e.stale ? ' · <span style="color:#d29922">来源文件改过了，可能过期</span>' : '') +
            '</div></div>';
        }).join('') : ('<div class="ab-tip">' + (key ? '没搜到。' : '还是空的 —— 它在对话里学到东西时会自己记下来。') + '</div>');
      }

      function load() {
        Promise.all([
          api('/v1/config'),
          api('/v1/memory?scope=all&limit=200'),
        ]).then(function (rs) {
          var el = body.querySelector('#ab-mem');
          if (!el) return;
          var cfg = rs[0].body || {};
          var mem = rs[1].body || {};
          entries = mem.entries || [];
          if (!rs[1].ok) {
            el.innerHTML = '<div class="ab-tip" style="color:#f85149">读不到记忆：' +
              esc(mem.error || ('HTTP ' + rs[1].code)) + '</div>';
            return;
          }
          if (cfg.memory_enabled === false) {
            el.innerHTML =
              '<div class="ab-tip">这个项目的「记忆」<b>还没打开</b>。</div>' +
              '<div class="ab-tip">打开之后：它会把你在对话里说过的习惯、偏好、常用说法自己记下来，' +
              '下次对话自动想起来（每次只会带上最近的几十条）。<b>你随时能在这儿看到它们、也能清空，' +
              '不想用了就关掉 —— 都你自己控制，不用找平台。</b></div>' +
              '<button class="ab-btn" id="mem-on" type="button">打开记忆</button><div class="ab-msg" id="mem-msg"></div>';
            body.querySelector('#mem-on').onclick = function () {
              var btn = this;
              abSetMemory(true, btn, load);
            };
            return;
          }
          var html =
            '<div class="ab-tip">它自己攒下来、下次对话会想起来的事（最近 32 条会带进对话，所以别拿它当资料库）。' +
            '这些只作用于<b>本项目</b>。</div>' +
            '<div class="ab-row"><input class="ab-input" id="mem-q" placeholder="搜一搜（比如「客户」）" value="' + esc(q) + '"></div>';
          html += '<div id="mem-list"></div>';
          html += '<div class="ab-tip" style="margin-top:12px">这里只能<b>整批清空</b>，官方没给「只删这一条」的口子。' +
            '想让它忘掉某一件事，直接在对话里说一声就行（比如「忘掉关于报表格式的偏好」）。</div>';
          html += '<div style="display:flex;gap:8px;flex-wrap:wrap;align-items:center">' +
            '<button class="ab-btn danger sm" id="mem-clear-ws" type="button">清空「本项目」的记忆</button>' +
            '<button class="ab-btn danger sm" id="mem-clear-global" type="button">清空「通用」的记忆</button>' +
            '<button class="ab-btn ghost sm" id="mem-reload" type="button">刷新</button>' +
            '<button class="ab-btn ghost sm" id="mem-off" type="button" style="margin-left:auto">关掉记忆</button></div>' +
            '<div class="ab-msg" id="mem-msg"></div>';
          el.innerHTML = html;

          var qi = body.querySelector('#mem-q');
          // 只重渲染列表，输入框本身不重建（否则一边打字一边失焦）
          qi.oninput = function () {
            clearTimeout(debounce);
            var v = qi.value;
            debounce = setTimeout(function () { q = v.trim(); renderList(); }, 150);
          };
          renderList();
          body.querySelector('#mem-reload').onclick = load;
          function clearOne(scope, label) {
            var n = (scope === 'workspace')
              ? entries.filter(function (e) { return e.scope === 'workspace'; }).length
              : entries.filter(function (e) { return e.scope !== 'workspace'; }).length;
            if (!n) { msg(body.querySelector('#mem-msg'), '「' + label + '」里本来就没有东西。', false); return; }
            if (!confirm('清空「' + label + '」的记忆？\n共 ' + n + ' 条 —— 清掉之后 AI 就不再记得这些了。\n（不影响你的文件、代码和对话记录）')) return;
            var m = body.querySelector('#mem-msg');
            msg(m, '正在清…', true);
            api('/v1/memory?scope=' + scope, { method: 'DELETE' }).then(function (r) {
              if (!r.ok) { msg(m, '没清成：' + ((r.body && r.body.error) || ('HTTP ' + r.code)), false); return; }
              msg(m, '已清空「' + label + '」', true);
              load();
            });
          }
          body.querySelector('#mem-clear-ws').onclick = function () { clearOne('workspace', '本项目'); };
          body.querySelector('#mem-clear-global').onclick = function () { clearOne('global', '通用'); };
          body.querySelector('#mem-off').onclick = function () {
            if (!confirm('关掉记忆？\n以后它不再记新东西、也不再带进对话。\n（已经记下的还留着，想一并清掉就点上面的「清空」）')) return;
            abSetMemory(false, this, load);
          };
        });
      }
      load();
    });
  }

  /* ── 高级设置（git 远程 / 它干活的方式 / 看得见什么 / 只看不改 / 花费）──
   * 这些开关走**官方 `POST /v1/config`**（引擎以 `cus-<项目>` 身份跑，自己写自己的配置）——
   * 不需要平台介入、**不需要 sudo**。可写键受官方白名单限制（写错键时接口会列出全部键）。
   * ⚠️ 官方**故意**不让 API 写密钥（安全设计）→ 模型密钥那条仍走门卫 + root 帮手。
   * ⚠️ 「过程多详细（折叠几行）」= `thinking_preview_lines`，**不在白名单里**；
   *    老做法直接写 `settings.toml` —— 那份属 `cus-<项目>`，门卫（ubuntu）写不进去（EACCES）。
   *    → 已换成下面两个客户看得懂的开关（看得见它在想什么 / 默认摊开），不再碰那个文件。
   */
  function openAdvanced() {
    openLayer('高级设置', function (body) {
      body.innerHTML = '<div id="ab-adv">加载中…</div>';
      Promise.all([api('/_gate/advanced'), api('/_gate/model-key'), api('/v1/config'), api('/_gate/repo')]).then(function (rs) {
        var r = rs[0];
        var mk = rs[1].body || {};
        var cfg = (rs[2] && rs[2].body) || {}
        var repo = (rs[3] && rs[3].body) || {};
        var am = cfg.approval_mode || 'auto';   // auto=小的自己做、拿不准才问（默认）｜ suggest=每步先问 ｜ bypass=全放行
        var cur = cfg.cost_currency === 'cny' ? 'cny' : 'usd';
        var el = body.querySelector('#ab-adv');
        if (!el) return;
        if (!r.ok) {
          el.innerHTML = '<div class="ab-tip">' + esc((r.body && r.body.error) || '读不到') + '</div>';
          return;
        }
        var d = r.body || {};
        var u = d.usage;
        el.innerHTML =
          '<div class="ab-tip">这些是你自己的习惯，<b>改一次，名下所有项目都生效</b>（以后新建的项目也一样）。</div>' +
          '<div class="ab-row"><label>模型服务</label><span class="ab-input" style="cursor:default;color:#8b949e;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">' +
            esc(mk.provider || '—') + ' · ' + esc(mk.model || '—') + ' · ' + (mk.hasKey ? '密钥已配好' : '还没配密钥') +
          '</span><button class="ab-btn ghost sm" id="adv-mk" type="button" style="flex:0 0 auto">改</button></div>' +
          '<div class="ab-row"><label>代码仓库</label><span class="ab-input" style="cursor:default;color:#8b949e;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">' +
            esc(repoSummary(repo)) +
          '</span><button class="ab-btn ghost sm" id="adv-repo" type="button" style="flex:0 0 auto">设置</button></div>' +
          '<div style="margin:14px 0 6px;color:#e6edf3;font-size:14px">它动手前</div>' +
          '<div class="ab-row"><label>审批方式</label><select class="ab-input" id="adv-approval">' +
            '<option value="suggest"' + (am === 'suggest' ? ' selected' : '') + '>每步都先问我（默认）</option>' +
            '<option value="auto"' + (am === 'auto' ? ' selected' : '') + '>小的自己做，拿不准才问我</option>' +
            '<option value="bypass"' + (am === 'bypass' ? ' selected' : '') + '>全部自己做，不问</option>' +
          '</select></div>' +
          '<div class="ab-tip" style="margin:-4px 0 10px 78px">选「全部自己做」之后，它改文件、跑命令就不再问你 —— 拿不准就保持默认。</div>' +
          '<div style="margin:14px 0 6px;color:#e6edf3;font-size:14px">看得见什么</div>' +
          '<label class="ab-chk"><input type="checkbox" id="adv-think"' + (cfg.show_thinking ? ' checked' : '') + '> 看得见它在想什么</label>' +
          '<label class="ab-chk"><input type="checkbox" id="adv-think-exp"' + (cfg.thinking_default_expanded ? ' checked' : '') + '> 思考过程默认摊开（不用点一下）</label>' +
          '<label class="ab-chk"><input type="checkbox" id="adv-tools"' + (cfg.show_tool_details ? ' checked' : '') + '> 看得见它动了哪些文件、跑了什么</label>' +
          '<label class="ab-chk"><input type="checkbox" id="adv-calm"' + (cfg.calm_mode ? ' checked' : '') + '> 安静模式（过程和细节都收起来，只留结论）</label>' +
          '<label class="ab-chk"><input type="checkbox" id="adv-compact"' + (cfg.auto_compact ? ' checked' : '') + '> 聊天太长时自动帮我整理前面</label>' +
          '<div class="ab-tip" style="margin:2px 0 10px 0">自动整理会把前面的内容总结掉 —— 细节会丢一部分（默认开）。<br>⚠️ <b>关掉要留心</b>：聊得久了它可能突然不回话（前面说的已经超出模型一次能记住的范围）。</div>' +
          '<div class="ab-row"><label>钱按哪种显示</label><select class="ab-input" id="adv-currency">' +
            '<option value="cny"' + (cur === 'cny' ? ' selected' : '') + '>人民币 ￥</option>' +
            '<option value="usd"' + (cur === 'usd' ? ' selected' : '') + '>美元 $</option>' +
          '</select></div>' +
          '<label class="ab-chk"><input type="checkbox" id="adv-ro"' + (d.previewReadOnly ? ' checked' : '') + '> 只看不改（防误删）</label>' +
          '<div style="margin:14px 0 6px;color:#e6edf3;font-size:14px">花了多少</div>' +
          (u
            ? '<div class="ab-card"><div class="ab-s">累计 ' + (DISPLAY.cost_currency === 'cny'
                ? '￥' + Number(u.costCny || 0).toFixed(2)
                : '$' + Number(u.costUsd || 0).toFixed(2)) + ' ｜ 改动 ' + (u.turns || 0) + ' 次<br>进 ' + Math.round((u.inTok || 0) / 1000) + 'K / 出 ' + Math.round((u.outTok || 0) / 1000) + 'K token</div></div>'
            : '<div class="ab-tip">这个项目还没有用量记录。</div>') +
          '<div class="ab-msg" id="adv-msg"></div>';
        var msgEl = el.querySelector('#adv-msg');
        abTipPanel(el, 'adv-approval', '它改东西之前要不要先问你，在这儿选 —— 嫌问得多就选「小的自己做」');
        function post(payload) {
          return api('/_gate/advanced', { method: 'POST', body: JSON.stringify(payload) }).then(function (r2) {
            if (r2.ok) msg(msgEl, '已保存', true); else msg(msgEl, (r2.body && r2.body.error) || '保存失败', false);
          });
        }
        /* 配置项：官方 POST /v1/config（persist 才写盘）→ 再 reload 让它生效。
         * reload 官方说明：**新的一轮对话**采用新配置，不影响正在跑的那轮。 */
        function setCfg(key, value) {
          // ★ 全局设置（2026-09-16 老板：「直接做成全局设置就行，包括记忆 ——
          //   用户习惯不可能每个项目都修改吧？」）—— 不再写「当前项目」，
          //   而是交给门卫：存成这个账号的偏好 + 应用到名下所有项目
          //   （没在跑的项目等下次打开时自动补上）。
          return api('/_gate/prefs', {
            method: 'POST',
            body: JSON.stringify({ prefs: (function () { var o = {}; o[key] = String(value); return o; })() }),
          }).then(function (r2) {
            var b = (r2.body || {});
            if (!r2.ok || b.ok === false) {
              msg(msgEl, '没设置成：' + (b.error || r2.code), false);
              return false;
            }
            msg(msgEl, b.message || '已保存（名下所有项目）', true);
            syncDisplayPref(key, value);
            return true;
          });
        }
        /* 存成功后**当场**让网页版跟着变 —— 否则客户得刷新才看得到效果 */
        function syncDisplayPref(key, v) {
          if (key === 'cost_currency') DISPLAY.cost_currency = (v === 'cny' ? 'cny' : 'usd');
          else if (key === 'show_thinking' || key === 'thinking_default_expanded' || key === 'show_tool_details' || key === 'calm_mode') {
            DISPLAY[key] = (v === 'true' || v === true);
          } else return;
          applyDisplayPrefs();
        }
        function bindChk(id, key) {
          var box = el.querySelector('#' + id);
          if (!box) return;
          box.onchange = function () {
            box.disabled = true;
            setCfg(key, box.checked ? 'true' : 'false').then(function (good) {
              box.disabled = false;
              if (!good) box.checked = !box.checked;   // 没存成 → 拨回去，不骗人
            });
          };
        }
        function bindSel(id, key) {
          var sel = el.querySelector('#' + id);
          if (!sel) return;
          sel.onchange = function () { sel.disabled = true; setCfg(key, sel.value).then(function () { sel.disabled = false; }); };
        }
        el.querySelector('#adv-mk').onclick = openModelApiLoader;
        el.querySelector('#adv-repo').onclick = function () { openRepoForm(repo); };
        bindSel('adv-approval', 'approval_mode');
        bindChk('adv-think', 'show_thinking');
        bindChk('adv-think-exp', 'thinking_default_expanded');
        bindChk('adv-tools', 'show_tool_details');
        bindChk('adv-calm', 'calm_mode');
        bindChk('adv-compact', 'auto_compact');
        bindSel('adv-currency', 'cost_currency');
        el.querySelector('#adv-ro').onchange = function (e) { post({ previewReadOnly: e.target.checked }); };
      });
    });
  }

  /* ── 模型服务：客户用自己的大模型 API（Claude / Kimi / 自建网关）──
   * 为什么走门卫而不用官方 API：官方**故意不让 API 写密钥**（POST /v1/config 的允许键里
   * 没有 api_key），密钥只能落项目自己的 config.toml，而那份属主是 cus-<项目>。
   * 所以走门卫的 /_gate/model-key → 受限 root 帮手（sudoers 只放行它、且不带参数）。
   * 提供商列表来自官方目录（48 家），不写死。 */
  function openModelApiForm(d) {
    openLayer('模型服务', function (body) {
      var ps = d.providers || [];
      // 官方目录里有几家显示名重复（如 Model Studio 的四个变体）→ 同名时带上 id 便于区分
      var nameCount = {};
      ps.forEach(function (p) { nameCount[p.name] = (nameCount[p.name] || 0) + 1; });
      var opts = ps.map(function (p) {
        var label = p.name + (nameCount[p.name] > 1 ? ' · ' + p.id : '');
        return '<option value="' + esc(p.id) + '"' + (p.id === d.provider ? ' selected' : '') + '>' +
          esc(label) + (p.ready ? '（已配好）' : '') + '</option>';
      }).join('');
      body.innerHTML =
        '<div class="ab-tip">用你自己的大模型 API：选一家、填密钥。留空的项就不改（端点 / 模型名 / 密钥都是）。</div>' +
        (d.helper ? '' : '<div class="ab-tip" style="color:#d29922">⚠️ 服务端还没装「模型密钥」帮手，现在保存不了 —— 让管理员跑一下安装脚本。</div>') +
        '<div class="ab-row"><label>用哪家</label><select class="ab-input" id="mk-provider">' + opts + '</select></div>' +
        '<div class="ab-row"><label>端点地址</label><input class="ab-input" id="mk-base" placeholder="留空用这家的官方地址" value="' + esc(d.base_url || '') + '"></div>' +
        '<div class="ab-row"><label>自己的密钥</label><input class="ab-input" id="mk-key" type="password" autocomplete="new-password" placeholder="留空 = 不改（密钥不会回显）"></div>' +
        '<div class="ab-row"><label>模型名</label><input class="ab-input" id="mk-model" placeholder="留空用这家的默认"></div>' +
        '<div style="display:flex;gap:8px;margin-top:16px"><button class="ab-btn" id="mk-save" type="button">保存</button>' +
        '<button class="ab-btn ghost" id="mk-cancel" type="button">取消</button>' +
        '<button class="ab-btn danger" id="mk-clear" type="button" style="margin-left:auto">清除这家的配置</button></div><div class="ab-msg" id="mk-msg"></div>';
      var msgEl = body.querySelector('#mk-msg');
      body.querySelector('#mk-cancel').onclick = closeLayer;
      var sel = body.querySelector('#mk-provider');
      function refreshHint() {
        var p = ps.filter(function (x) { return x.id === sel.value; })[0] || {};
        body.querySelector('#mk-model').placeholder = p.defaultModel ? ('留空用 ' + p.defaultModel) : '留空用这家的默认';
      }
      sel.onchange = refreshHint;
      refreshHint();
      body.querySelector('#mk-clear').onclick = function () {
        var pid = sel.value;
        var nm = (ps.filter(function (x) { return x.id === pid; })[0] || {}).name || pid;
        if (!confirm('把「' + nm + '」的配置（密钥 / 端点 / 模型名）全删掉？\n如果当前用的就是它，会自动回退到 DeepSeek。')) return;
        msg(msgEl, '正在清除…', true);
        api('/_gate/model-key', { method: 'POST', body: JSON.stringify({ provider: pid, clear: true }) }).then(function (r) {
          var b = r.body || {};
          if (!r.ok || b.ok === false) {
            msg(msgEl, (b.error || '清除失败') + '（若提示帮手是旧版，让管理员重装一下）', false);
            return;
          }
          body.querySelector('#mk-key').value = '';
          msg(msgEl, '已清除「' + nm + '」的配置', true);
        });
      };
      body.querySelector('#mk-save').onclick = function () {
        var payload = {
          provider: sel.value,
          base_url: body.querySelector('#mk-base').value.trim(),
          model: body.querySelector('#mk-model').value.trim(),
        };
        var k = body.querySelector('#mk-key').value;
        if (k) payload.api_key = k;
        msg(msgEl, '正在保存…', true);
        api('/_gate/model-key', { method: 'POST', body: JSON.stringify(payload) }).then(function (r) {
          var b = r.body || {};
          if (!r.ok || b.ok === false) { msg(msgEl, b.error || '保存失败', false); return; }
          body.querySelector('#mk-key').value = '';
          var what = (b.changed || []).join(' / ') || '无变化';
          msg(msgEl, '改好了（' + what + '）' + (b.verified ? ' · 引擎已重读并确认' : ''), true);
        });
      };
    });
  }
  function openModelApiLoader() {
    api('/_gate/model-key').then(function (r) {
      var d = r.body || {};
      if (!r.ok) { alert(d.error || '读不到配置'); return; }
      openModelApiForm(d);
    });
  }

  /* ── 代码仓库：把项目接到客户自己的仓库（2026-09-16 老板定）──
   * 平台只做三件事：填地址、备凭据、看得见状态。
   * **同步（push / pull）交给 AI 在对话里做** —— 客户说「推到我的仓库」它才推
   * （规矩写在项目自己的 AGENTS.md 里，引擎每次干活都读）。
   * 凭据落在项目自己的引擎家（engine-home-<key>/.ssh/），门卫写不进去 → 走受限 root 帮手。
   * 设计边界（别推翻）：平台不托管仓库、不预置任何地址、不替客户 push、也不让 AI 自作主张 push。 */
  function repoSummary(r) {
    if (!r) return '还没接（代码只在这台服务器上）';
    if (!r.remote) return r.helper === false ? '还没接（平台还没装帮手）' : '还没接（代码只在这台服务器上）';
    return String(r.remote).replace(/^[a-z]+:\/\//, '').replace(/^[^@/]*@/, '');
  }
  function repoWhen(iso) {
    var d = new Date(iso);
    if (isNaN(d.getTime())) return '';
    function p(n) { return (n < 10 ? '0' : '') + n; }
    return p(d.getMonth() + 1) + '-' + p(d.getDate()) + ' ' + p(d.getHours()) + ':' + p(d.getMinutes());
  }
  function repoStateLine(d) {
    if (!d.initialized) return '这个项目还没有版本库 —— 保存地址时会自动建一个，并把现在的文件提交一份。';
    var p = ['版本库 ' + (d.branch || 'main')];
    if (d.lastCommit) p.push('最新：' + d.lastCommit.subject + '（' + repoWhen(d.lastCommit.when) + '）');
    if (typeof d.ahead === 'number') p.push(d.ahead > 0 ? ('有 ' + d.ahead + ' 次改动还没推上去') : '本机和仓库里的一致');
    if (d.dirty) p.push('有改动还没提交');
    return p.join(' ｜ ');
  }
  function openRepoForm(init, flash) {
    var d = init || {};
    var plat = d.platform || 'gitee';
    var auth = d.authMode === 'token' ? 'token' : 'deploy-key';
    var pub = d.publicKey || '';

    openLayer('代码仓库', function (body) {
      var noHelper = d.helper === false;
      var html = '<div class="ab-tip">把项目代码接到<b>你自己的</b>仓库，东西就不只存在这台服务器上。' +
        '接好之后，在对话里说「推到我的仓库」它才会推 —— 不会自作主张。</div>';
      if (noHelper) {
        html += '<div class="ab-tip" style="color:#d29922">⚠️ 服务端还没装「代码仓库」帮手，现在存不了 —— 让管理员跑一下安装脚本。</div>';
      }

      html += '<div class="ab-row"><label>用哪家</label><select class="ab-input" id="rp-platform">' +
        [['gitee', 'Gitee（码云）'], ['github', 'GitHub'], ['other', '其他 / 自己搭的']].map(function (x) {
          return '<option value="' + x[0] + '"' + (x[0] === plat ? ' selected' : '') + '>' + x[1] + '</option>';
        }).join('') + '</select></div>';
      html += '<div class="ab-row"><label>仓库地址</label><input class="ab-input" id="rp-remote" placeholder="git@gitee.com:你的账号/仓库.git" value="' + esc(d.remote || '') + '"></div>';
      html += '<div class="ab-tip" style="margin:-4px 0 10px 78px">在仓库页面点「克隆」，把 <b>SSH</b> 那一行贴进来（也可以是 https:// 地址）。' +
        '请用你自己的<b>私有</b>仓库 —— 平台不托管、也不会把代码推到别处。</div>';
      html += '<div style="display:flex;gap:8px;margin:-2px 0 6px 78px">' +
        '<button class="ab-btn sm" id="rp-save" type="button"' + (noHelper ? ' disabled' : '') + '>保存地址</button>' +
        '<button class="ab-btn ghost sm" id="rp-clear" type="button"' + (noHelper ? ' disabled' : '') + '>移除地址</button></div>';

      html += '<div style="margin:14px 0 6px;color:#e6edf3;font-size:14px">怎么证明是你</div>';
      html += '<div class="ab-row"><label>方式</label><select class="ab-input" id="rp-auth">' +
        '<option value="deploy-key"' + (auth === 'deploy-key' ? ' selected' : '') + '>部署密钥（推荐）</option>' +
        '<option value="token"' + (auth === 'token' ? ' selected' : '') + '>访问令牌（https 地址用）</option>' +
        '</select></div>';

      var keyBox = '';
      if (d.hasKey && pub) {
        keyBox += '<label style="color:#8b949e;font-size:13.5px">你的公钥（这段贴给仓库，不碍事）</label>' +
          '<textarea class="ab-input" id="rp-pub" readonly style="height:70px;font-family:ui-monospace,Menlo,Consolas,monospace;font-size:12.5px;margin:4px 0;resize:vertical">' + esc(pub) + '</textarea>' +
          '<div style="display:flex;gap:8px;margin-bottom:8px"><button class="ab-btn sm" id="rp-copy" type="button">复制公钥</button>' +
          '<button class="ab-btn danger sm" id="rp-delkey" type="button" style="margin-left:auto">删掉密钥</button></div>';
        if (d.fingerprint) keyBox += '<div class="ab-s" style="margin-bottom:6px">指纹 ' + esc(d.fingerprint) + '</div>';
        keyBox += '<div class="ab-tip">还没贴到仓库？到仓库页面 → <b>设置 → 部署公钥（Deploy Keys）</b> → 粘贴 → 勾上「允许写入」。' +
          '勾不上写入的话，推的时候会被拒。</div>';
      } else {
        keyBox += '<div class="ab-tip">还没有密钥。点下面这个按钮生成一对：<b>公钥</b>贴到你的仓库，<b>私钥</b>留在项目里（谁都看不到）。</div>' +
          '<button class="ab-btn" id="rp-genkey" type="button"' + (noHelper ? ' disabled' : '') + '>生成密钥</button>';
      }
      html += '<div id="rp-keybox">' + keyBox + '</div>';

      var tokBox = '<div class="ab-row"><label>仓库用户名</label><input class="ab-input" id="rp-user" placeholder="登录仓库的账号名"></div>' +
        '<div class="ab-row"><label>访问令牌</label><input class="ab-input" id="rp-token" type="password" autocomplete="new-password" placeholder="' +
        (d.hasToken ? '已经存好了（要换就填新的）' : '在仓库设置里生成一个') + '"></div>' +
        '<div style="display:flex;gap:8px"><button class="ab-btn sm" id="rp-savetoken" type="button"' + (noHelper ? ' disabled' : '') + '>保存令牌</button>' +
        (d.hasToken ? '<button class="ab-btn danger sm" id="rp-cleartoken" type="button" style="margin-left:auto">清除令牌</button>' : '') + '</div>' +
        '<div class="ab-tip" style="margin-top:8px">令牌只存在这个项目里（600），不会写进对话、也不会回显。</div>';
      html += '<div id="rp-tokenbox"' + (auth === 'token' ? '' : ' hidden') + '>' + tokBox + '</div>';

      html += '<div style="margin:14px 0 6px;color:#e6edf3;font-size:14px">现在怎么样</div>';
      html += '<div class="ab-card"><div class="ab-s" id="rp-state">' + esc(repoStateLine(d)) + '</div></div>';
      html += '<div class="ab-msg" id="rp-msg"></div>';
      body.innerHTML = html;

      var msgEl = body.querySelector('#rp-msg');
      if (flash) msg(msgEl, flash.text, flash.ok);
      function refresh(flash2) {
        api('/_gate/repo').then(function (r) { openRepoForm(r.body || {}, flash2); });
      }
      function post(payload, okText) {
        msg(msgEl, '正在处理…', true);
        api('/_gate/repo', { method: 'POST', body: JSON.stringify(payload) }).then(function (r) {
          var b = r.body || {};
          if (!r.ok || b.ok === false) { msg(msgEl, b.error || '没成功', false); return; }
          if (okText) refresh({ text: okText, ok: true }); else msg(msgEl, '已保存', true);
        });
      }
      function syncState() {
        api('/_gate/repo').then(function (r) {
          var b = r.body || {};
          var el = body.querySelector('#rp-state');
          if (el) el.textContent = repoStateLine(b);
        });
      }

      body.querySelector('#rp-auth').onchange = function (e) {
        body.querySelector('#rp-tokenbox').hidden = e.target.value !== 'token';
        body.querySelector('#rp-keybox').hidden = e.target.value === 'token';
      };
      body.querySelector('#rp-save').onclick = function () {
        var remote = body.querySelector('#rp-remote').value.trim();
        if (!remote) { msg(msgEl, '先把仓库地址填上', false); return; }
        post({ action: 'save', platform: body.querySelector('#rp-platform').value, remote: remote });
        syncState();
      };
      body.querySelector('#rp-clear').onclick = function () {
        if (!confirm('把仓库地址移掉？代码就只留在这台服务器上了（本地版本库不动）。')) return;
        post({ action: 'clear', platform: body.querySelector('#rp-platform').value });
        syncState();
      };
      var bg = body.querySelector('#rp-genkey');
      if (bg) bg.onclick = function () { post({ action: 'genkey' }, '密钥生成好了 —— 复制公钥贴到你的仓库'); };
      var bc = body.querySelector('#rp-copy');
      if (bc) bc.onclick = function () {
        var ta = body.querySelector('#rp-pub');
        ta.select();
        var done = function () { msg(msgEl, '公钥已复制，去仓库页面粘贴吧', true); };
        if (navigator.clipboard) navigator.clipboard.writeText(ta.value).then(done, function () { msg(msgEl, '复制失败，手动选中它复制吧', false); });
        else { try { document.execCommand('copy'); done(); } catch (e) { msg(msgEl, '复制失败，手动选中它复制吧', false); } }
      };
      var bd = body.querySelector('#rp-delkey');
      if (bd) bd.onclick = function () {
        if (!confirm('删掉这对密钥？删了之后要重新生成、并重新贴到仓库才能推。')) return;
        post({ action: 'delkey' }, '密钥已删除');
      };
      body.querySelector('#rp-savetoken').onclick = function () {
        post({ action: 'token', username: body.querySelector('#rp-user').value.trim(), token: body.querySelector('#rp-token').value.trim() }, '令牌存好了');
      };
      var bt = body.querySelector('#rp-cleartoken');
      if (bt) bt.onclick = function () { post({ action: 'clearToken' }, '令牌已清除'); };
    });
  }

  /* ── 修改密码 ── */
  /* ── 我的账号（2026-09-16 老板问：用户自己的账号和名字在哪里能看到？）──
   * 只读面板：名字 / 登录账号 / 角色 / 归属 / 项目额度。
   * 正在别人的视角里时，额外写明「真实登录的是谁」+ 一个退出口（不要让人困在别人的界面里）。
   * 数据来自 /_gate/whoami（视角下返回的就是被切那个账号的信息）。 */
  function openAccount() {
    openLayer('我的账号', function (body) {
      var me = ME || {};
      var roleTxt = me.role === 'admin' ? '管理员（平台）' : me.role === 'staff' ? '员工' : '客户老板';
      var opRoleTxt = me.operatorRole === 'admin' ? '管理员' : me.operatorRole === 'staff' ? '员工' : '客户老板';
      function row(k, v) {
        return '<div class="ab-row"><label>' + k + '</label>' +
          '<span class="ab-input" style="cursor:default;color:#e6edf3;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">' + esc(v) + '</span></div>';
      }
      var html = '<div class="ab-tip">你登录进来用的就是这个账号 —— 名字只是显示用，登录时填的是「登录账号」。</div>';
      html += row('名字', me.name || me.user || '—');
      html += row('登录账号', me.user || '—');
      html += row('角色', roleTxt);
      if (me.role === 'staff') html += row('归属', me.ownerName ? ('归「' + me.ownerName + '」管') : '平台直接管');
      if (typeof me.quota === 'number') html += row('能建几个项目', me.quota + ' 个');
      if (me.viewAs) {
        html += '<div class="ab-tip" style="color:#d29922;border-color:#d2992255">' +
          '⚠️ 你现在是以「' + esc(me.name || me.viewAs) + '」的视角在看；真实登录的是 ' +
          esc(me.operator || '') + '（' + opRoleTxt + '）。</div>' +
          '<div style="display:flex;gap:8px;margin-top:12px">' +
          '<button class="ab-btn ghost" id="ac-exit" type="button">退出视角，回到我自己</button></div>';
      }
      body.innerHTML = html;
      var bExit = body.querySelector('#ac-exit');
      if (bExit) bExit.onclick = function () {
        api('/_gate/view-as', { method: 'POST', body: JSON.stringify({ as: '' }) }).then(function () { location.href = '/'; });
      };
    });
  }

  function openPassword() {
    openLayer('修改密码', function (body) {
      body.innerHTML =
        '<div class="ab-row"><label>现在的密码</label><input class="ab-input" id="p-old" type="password"></div>' +
        '<div class="ab-row"><label>新密码</label><input class="ab-input" id="p-new" type="password" placeholder="至少 8 位"></div>' +
        '<div style="display:flex;gap:8px;margin-top:16px"><button class="ab-btn" id="p-save" type="button">保存</button>' +
        '<button class="ab-btn ghost" id="p-cancel" type="button">取消</button></div><div class="ab-msg" id="p-msg"></div>';
      var msgEl = body.querySelector('#p-msg');
      body.querySelector('#p-cancel').onclick = closeLayer;
      body.querySelector('#p-save').onclick = function () {
        var payload = { old: body.querySelector('#p-old').value, new: body.querySelector('#p-new').value };
        fetch('/_gate/password', {
          method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload),
        }).then(function (r) { return r.json(); }).then(function (j) {
          if (j.ok) { msg(msgEl, '改好了。' + (ME.role === 'admin' ? '所有设备要重新登录。' : ''), true); setTimeout(closeLayer, 1200); }
          else msg(j.error || '改不了', false, msgEl);
        });
      };
    });
  }

  /* ── @文件：点文件 → 「已带上：xxx」标签 → 发送时把路径带进消息 ──
   * 老系统的做法（不是上传二进制）：AI 本来就能读工作区文件，只需告诉它「用这份」。
   */
  (function () {
    var picked = null;
    var chip = null;
    function ensureChip() {
      if (chip && document.body.contains(chip)) return chip;
      var form = document.getElementById('composer');
      var ta = document.getElementById('composer-input');
      if (!form || !ta) return null;
      chip = document.createElement('div');
      chip.className = 'ab-chip';
      chip.hidden = true;
      chip.innerHTML = '<span>已带上：<b class="ab-chip-name"></b></span>' +
        '<span class="ab-chip-x" title="不带这份文件">×</span>';
      form.insertBefore(chip, ta);
      chip.querySelector('.ab-chip-x').onclick = function () { picked = null; chip.hidden = true; };
      return chip;
    }
    document.addEventListener('asbudy-file-picked', function (e) {
      if (!e.detail || !e.detail.path) return;
      picked = e.detail.path;
      var c = ensureChip();
      if (!c) return;
      c.querySelector('.ab-chip-name').textContent = e.detail.name || e.detail.path;
      c.hidden = false;
    });
    // 发送时把路径塞进消息
    // ⚠️ 官方有**两条**发送路径：① 点「发送」→ submit 事件；② **按回车 → keydown 里直接
    //    调 sendMessage()，不经过 submit**。只拦 submit 的话，回车发送就白带了（实测踩过）。
    function injectPicked() {
      if (!picked) return;
      var ta = document.getElementById('composer-input');
      if (!ta) return;
      if (ta.value.indexOf(picked) >= 0) return;
      var clean = ta.value.replace(/\s*（用这份：[\s\S]*?）\s*$/, '');
      ta.value = clean + (clean ? '\n' : '') + '（用这份：' + picked + '）';
    }
    document.addEventListener('submit', function (e) {
      if (e.target && e.target.id === 'composer') injectPicked();
    }, true);
    document.addEventListener('keydown', function (e) {
      if (e.key !== 'Enter' || e.shiftKey) return;
      var t = e.target;
      if (!t || t.id !== 'composer-input') return;
      injectPicked();
    }, true);
    var tries = 0;
    var t = setInterval(function () { if (ensureChip() || ++tries > 60) clearInterval(t); }, 400);
  })();

  /* ── 干活计时（「已用 N 秒」）—— 官方用 #interrupt-turn 的显隐标记「在干活」 ── */
  (function () {
    var tick = null;
    var startedAt = 0;

    function ensureEl() {
      var el = document.getElementById('asbudy-tick');
      if (el && document.body.contains(el)) return el;
      var wrap = document.querySelector('.composer-wrap') || document.getElementById('composer');
      if (!wrap || !wrap.parentNode) return null;
      el = document.createElement('div');
      el.id = 'asbudy-tick';
      el.hidden = true;
      wrap.parentNode.insertBefore(el, wrap);
      return el;
    }
    function fmt(sec) {
      if (sec < 60) return sec + ' 秒';
      return Math.floor(sec / 60) + ' 分 ' + (sec % 60) + ' 秒';
    }
    function paint() {
      var el = ensureEl();
      if (!el) return;
      var sec = Math.round((Date.now() - startedAt) / 1000);
      el.textContent = '⏱ 正在干活 · 已用 ' + fmt(sec) + (sec >= 60 ? '（还在干，不是卡死）' : '');
    }
    function start() {
      startedAt = Date.now();
      var el = ensureEl();
      if (el) el.hidden = false;
      paint();
      if (tick) clearInterval(tick);
      tick = setInterval(paint, 500);
    }
    function stop() {
      if (tick) { clearInterval(tick); tick = null; }
      var el = document.getElementById('asbudy-tick');
      if (el) el.hidden = true;
    }
    function watch() {
      var btn = document.getElementById('interrupt-turn');
      if (!btn) return false;
      var active = !btn.hidden;
      if (active) start(); else stop();
      new MutationObserver(function () {
        var now = !btn.hidden;
        if (now === active) return;
        active = now;
        if (now) start(); else stop();
      }).observe(btn, { attributes: true, attributeFilter: ['hidden'] });
      return true;
    }
    if (!watch()) {
      var n = 0;
      var t = setInterval(function () { if (watch() || ++n > 60) clearInterval(t); }, 400);
    }
  })();

  /* ── PC 侧栏可隐藏（官方只在窄屏给了开关）── */
  (function () {
    function ensure() {
      var shell = document.getElementById('app-shell');
      var brand = document.querySelector('.rail-brand');
      if (!shell || !brand) return false;
      if (!document.getElementById('asbudy-rail-hide')) {
        var b = document.createElement('button');
        b.id = 'asbudy-rail-hide';
        b.type = 'button';
        b.className = 'rail-hide';
        b.textContent = '收起';
        b.title = '收起侧栏（腾地方看内容）';
        brand.appendChild(b);
      }
      if (!document.getElementById('asbudy-rail-reveal')) {
        var r = document.createElement('button');
        r.id = 'asbudy-rail-reveal';
        r.type = 'button';
        r.className = 'rail-reveal';
        r.textContent = '☰ 侧栏';
        r.title = '展开侧栏';
        shell.appendChild(r);
      }
      return true;
    }
    document.addEventListener('click', function (e) {
      var t = e.target;
      if (!t || !t.id) return;
      var shell = document.getElementById('app-shell');
      if (!shell) return;
      if (t.id === 'asbudy-rail-hide') shell.classList.add('rail-hidden');
      else if (t.id === 'asbudy-rail-reveal') shell.classList.remove('rail-hidden');
    });
    if (!ensure()) {
      var n = 0;
      var tm = setInterval(function () { if (ensure() || ++n > 60) clearInterval(tm); }, 400);
    }
  })();

  /* ── 对话重试 / 撤销（官方有 API，前端没接）──
   * 拿不到官方内部的 selectedThreadId，所以拦 fetch 记下当前 thread。
   */
  (function () {
    var LAST_THREAD = '';
    var origFetch = window.fetch;
    if (typeof origFetch === 'function') {
      window.fetch = function (url, opt) {
        try {
          var u = String((url && url.url) || url || '');
          var m = u.match(/\/v1\/threads\/([^/?]+)/);
          if (m && m[1] && m[1] !== 'summary') LAST_THREAD = m[1];
          // 发完一轮（POST .../turns）后晚一点刷新「记性」——那时引擎才算得出本轮用量
          if (/\/v1\/threads\/[^/]+\/turns\b/.test(u)
              && String((opt && opt.method) || '').toUpperCase() === 'POST') {
            setTimeout(function () { try { loadCtx(); } catch (e0) {} }, 4000);
          }
        } catch (e0) {}
        return origFetch.apply(this, arguments);
      };
    }

    function ensure() {
      var el = document.getElementById('asbudy-msgbar');
      if (el && document.body.contains(el)) return el;
      var wrap = document.querySelector('.composer-wrap') || document.getElementById('composer');
      if (!wrap || !wrap.parentNode) return null;
      el = document.createElement('div');
      el.id = 'asbudy-msgbar';
      var b1 = document.createElement('button');
      b1.type = 'button';
      b1.id = 'asbudy-retry';
      b1.textContent = '↻ 重试';
      b1.title = '让 AI 把上一句重新答一遍（不动项目文件）';
      var b2 = document.createElement('button');
      b2.type = 'button';
      b2.id = 'asbudy-undo-turn';   // 注意：不能叫 asbudy-undo —— 那是退回面板容器的 id（重复 id 会让 getElementById 拿到错的）
      b2.textContent = '↩ 撤销';
      b2.title = '把最后一轮问答去掉，你那句话回到输入框（不动项目文件）';
      var b3 = document.createElement('button');
      b3.type = 'button';
      b3.id = 'asbudy-compact';
      b3.textContent = '🗜 压缩';
      b3.title = '把这段长对话压短，省 token（要点保留）';
      el.appendChild(b1);
      el.appendChild(b2);
      el.appendChild(b3);
      var ctxEl = document.createElement('span');
      ctxEl.id = 'asbudy-ctx';
      ctxEl.setAttribute('aria-live', 'polite');
      el.appendChild(ctxEl);
      wrap.parentNode.insertBefore(el, wrap);
      return el;
    }

    // ── 「记性 N%」：这次对话用了模型多少「记忆」（2026-09-15 · 老板要求）──
    // 为什么要门卫算：官方 CLI 状态栏有 `ctx NN%`，但那是引擎**进程内部状态**，web 拿不到；
    // 门卫复刻了引擎同一套窗口规则（按模型名查表），所以换模型会自动跟着变，不用人工设。
    // 不猜：门卫拿不到窗口或没数据时返回 available:false，这里就不显示。
    function fmtK(n) {
      n = Number(n) || 0;
      if (n >= 10000) {
        var w = Math.round(n / 10000 * 10) / 10;
        return (w % 1 === 0 ? String(w) : w.toFixed(1)) + ' 万';
      }
      return String(n);
    }
    async function loadCtx() {
      var el = document.getElementById('asbudy-ctx');
      if (!el) return;
      // ⚠️ 2026-09-16 老板：「界面只显示「压缩」，哪有百分比？」——
      //   病根：以前**没数据就把文字清空**（元素在、但空）→ 看着就是“没这功能”。
      //   现在**永远显示**：没数据就说「记性 —」，鼠标移上去告诉为什么。
      function show(txt, title, hot) {
        el.textContent = txt;
        el.style.color = hot ? '#f85149' : '#8b949e';
        el.title = title;
        el.style.cursor = 'help';
      }
      if (!LAST_THREAD) {
        show('记性 —', '还没开始对话 —— 说一句之后，这里会显示它用了多少「记忆」');
        return;
      }
      try {
        var r = await fetch('/_gate/context?thread=' + encodeURIComponent(LAST_THREAD), { credentials: 'same-origin' });
        if (!r.ok) { show('记性 —', '暂时读不到（接口 ' + r.status + '）'); return; }
        var d = await r.json();
        if (!d || !d.available) {
          show('记性 —', '这条对话还没有用量记录' + ((d && d.model) ? '（模型 ' + d.model + '）' : ''));
          return;
        }
        var hot = d.percent >= 80;
        show('记性 ' + d.percent + '%',
          '这次对话占了模型「记忆」的 ' + d.percent + '%（' + fmtK(d.used) + ' / ' + fmtK(d.window) + '）'
          + (hot ? '\n快满了 —— 开个新对话，AI 会更清醒' : ''), hot);
      } catch (e) { show('记性 —', '暂时读不到'); }
    }
    setInterval(loadCtx, 15000);
    setTimeout(loadCtx, 3000);

    async function fire(kind) {
      if (!LAST_THREAD) { alert('先在右边说一句，才有可操作的对话'); return; }
      var labels = { retry: '重试', undo: '撤销', compact: '压缩' };
      if (kind === 'undo' && !confirm('撤销最后这一轮？\n\n这一问一答会从对话里去掉，你那句话回到输入框（可以改了再发）。\n项目里的文件不受影响 —— 要退文件，用左侧的「退回」。')) return;
      if (kind === 'compact' && !confirm('把当前对话压短？\n\n要点会保留，超长的历史会被 AI 总结掉 —— 能省 token，但细节会丢。')) return;
      try {
        var r = await fetch('/v1/threads/' + encodeURIComponent(LAST_THREAD) + '/' + kind, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          credentials: 'same-origin',
          body: '{}',
        });
        if (!r.ok) {
          var j = await r.json().catch(function () { return {}; });
          alert(labels[kind] + '失败：' + (j.error || r.status));
          return;
        }
        // 压缩是后台跑一个 turn，给久一点再刷新（其余操作很快）
        setTimeout(function () { location.reload(); }, kind === 'compact' ? 5000 : 600);
      } catch (e) {
        alert(labels[kind] + '失败：' + e.message);
      }
    }

    document.addEventListener('click', function (e) {
      var t = e.target;
      if (!t || !t.id) return;
      if (t.id === 'asbudy-retry') fire('retry');
      else if (t.id === 'asbudy-undo-turn') fire('undo');
      else if (t.id === 'asbudy-compact') fire('compact');
    });

    if (!ensure()) {
      var n = 0;
      var tm = setInterval(function () { if (ensure() || ++n > 60) clearInterval(tm); }, 400);
    }
  })();

  /* ── 把 logo 变成「我的」入口 ── */
  function bindLogo() {
    var logo = document.querySelector('.brand-mark');
    if (!logo || logo.dataset.asbudyMy === '1') return false;
    logo.dataset.asbudyMy = '1';
    logo.title = '我的（账号、空间、设置）';
    logo.setAttribute('role', 'button');
    logo.setAttribute('tabindex', '0');
    logo.addEventListener('click', openMyMenu);
    logo.addEventListener('keydown', function (e) {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); openMyMenu(); }
    });
    return true;
  }

  api('/_gate/whoami').then(function (r) { if (r.ok) ME = r.body; });

  /* ── 按需提示（in-context tips）────────────────────────────────────────────
   * 2026-09-16 老板定 B 方案。**为什么废掉原来那套 7 步挖洞引导**：它不是没做好，
   * 是方向错 —— NN/g《Mobile App Onboarding》(2020) 研究结论：卡片式教程**并没有提升
   * 用户的任务表现**，且交互成本高、易被跳过、还增加记忆负担；他们的建议是
   * 「尽可能不做引导，用户碰到那个界面时再出现提示（in-context / pull revelation）」，
   * 并且「先测不带引导的版本，卡住了先改界面」。
   * 所以这里只做三件事，**都不遮屏、不强制、看过就不再来**：
   *   ① 第一次进项目  → 一行条：设置入口在左上角的标记
   *   ② 第一次开「我的」→ 面板里一行：这些改一次，名下所有项目都生效
   *   ③ 第一次进「高级设置」→ 面板里一行：审批方式决定它动不动就问你
   * ⚠️ 每条只出现**一次**（localStorage 按 key 记）；客户关掉了就是不想看，别再来。
   */
  var TIP_PREFIX = 'ab-tip-';

  function tipSeen(key) {
    try { return !!localStorage.getItem(TIP_PREFIX + key); } catch (e) { return true; }   // 存不了 → 当看过（别反复烦）
  }
  function tipMark(key) { try { localStorage.setItem(TIP_PREFIX + key, '1'); } catch (e) { /* 无所谓 */ } }

  /** 对话区顶上的一行轻提示（3 秒后自己淡出，也能手动关） */
  function abTipTop(key, text, ms) {
    if (tipSeen(key)) return;
    tipMark(key);
    var bar = document.createElement('div');
    bar.id = 'asbudy-tip';
    bar.innerHTML = '<span>' + text + '</span><button type="button" aria-label="关闭">✕</button>';
    var st = document.createElement('style');
    st.textContent = '#asbudy-tip{position:fixed;left:50%;transform:translateX(-50%);top:12px;z-index:99998;display:flex;' +
      'align-items:center;gap:10px;background:#0d1117;border:1px solid #30363d;border-radius:10px;padding:9px 12px;' +
      'box-shadow:0 8px 24px rgba(0,0,0,.5);color:#c9d1d9;font-size:13.5px;max-width:88vw;transition:opacity .3s}' +
      '#asbudy-tip b{color:#e6edf3}#asbudy-tip button{background:none;border:0;color:#8b949e;cursor:pointer;font-size:13px;padding:0 2px}';
    document.head.appendChild(st);
    document.body.appendChild(bar);
    function bye() { bar.style.opacity = '0'; setTimeout(function () { bar.remove(); }, 320); }
    bar.querySelector('button').onclick = bye;
    setTimeout(bye, ms || 4200);
  }

  /** 面板里的一行提示（插在面板内容最顶部，跟着面板一起关） */
  function abTipPanel(body, key, text) {
    if (!body || tipSeen(key)) return;
    tipMark(key);
    var d = document.createElement('div');
    d.className = 'ab-tip';
    d.style.cssText = 'border:1px solid #3b7ddd66;background:#3b7ddd14;border-radius:8px;padding:8px 11px;margin:0 0 12px';
    d.innerHTML = text;
    body.insertBefore(d, body.firstChild);
  }

  /* ── 上手清单（checklist，2026-09-16 老板定 C 方案）──────────────────────────
   * 依据：行业实践（Notion / Slack 都这么做）—— 把上手要做的事列成 3 条、有顺序、有进度，
   *   用户自己掌控节奏；不像强推教程那样遮屏或强迫走完。
   * ⚠️ 三条**都不是「配设置」**，而是「先拿到一次价值」（NN/g：先让用户做成事，别先教配置）：
   *   ① 让它帮你做一件事（真发过一句话就自动打勾）② 看看它用哪个模型 ③ 看看它自己记的事
   * 三条齐了 → 自己消失；点「收起」也不再出现（都记在本机）。
   */
  var CK_KEY = 'ab-checklist-v1';
  function ckState() { try { return JSON.parse(localStorage.getItem(CK_KEY) || '{}'); } catch (e) { return {}; } }
  function ckSave(s) { try { localStorage.setItem(CK_KEY, JSON.stringify(s)); } catch (e) { /* 无所谓 */ } }
  function ckSet(k) { var s = ckState(); s[k] = 1; ckSave(s); ckPaint(); }

  var ckCss = document.createElement('style');
  ckCss.textContent = [
    '#asbudy-checklist{padding:0 0 8px}',
    '.ck-wrap{border:1px solid #30363d;background:#0d1117;border-radius:10px;padding:10px 12px}',
    '.ck-hd{display:flex;align-items:center;gap:9px;margin-bottom:8px}',
    '.ck-hd b{color:#e6edf3;font-size:13.5px;flex:none}',
    '.ck-bar{flex:1;height:5px;border-radius:3px;background:#21262d;overflow:hidden;display:block}',
    '.ck-bar i{display:block;height:100%;background:#238636;transition:width .25s}',
    '.ck-x{background:none;border:0;color:#8b949e;font-size:12.5px;cursor:pointer;flex:none}',
    '.ck-x:hover{color:#e6edf3}',
    '.ck-row{display:flex;align-items:center;gap:9px;width:100%;text-align:left;background:none;border:0;',
    'border-top:1px solid #161b22;padding:8px 0;cursor:pointer;color:#c9d1d9;font:inherit;font-size:13.5px}',
    '.ck-row:first-of-type{border-top:0}',
    '.ck-row:hover{color:#e6edf3}',
    '.ck-box{flex:none;width:17px;height:17px;border:1px solid #30363d;border-radius:5px;font-size:12px;',
    'line-height:15px;text-align:center;color:#3fb950}',
    '.ck-row.on .ck-box{border-color:#238636;background:#23863622}',
    '.ck-txt{flex:1}.ck-txt small{display:block;color:#6b7280;font-size:12.5px}',
    '.ck-go{color:#6b7280}',
    '.ck-done{border-color:#23863666;background:#23863612;color:#3fb950;font-size:13.5px}',
  ].join('\n');
  document.head.appendChild(ckCss);

  function ckPaint() {
    var el = document.getElementById('asbudy-checklist');
    var s = ckState();
    if (s.hidden) { if (el) el.remove(); return; }
    if (!el || !document.body.contains(el)) {
      var wrap = document.querySelector('.composer-wrap') || document.getElementById('composer');
      if (!wrap || !wrap.parentNode) return;                 // 界面还没起来，下次再说
      el = document.createElement('div');
      el.id = 'asbudy-checklist';
      wrap.parentNode.insertBefore(el, wrap);
    }
    var items = [
      { k: 'act', text: '让它帮你做一件事', hint: '在下面跟它说一句就行' },
      { k: 'model', text: '看看它在用哪个模型', hint: '平台已配好，也能换成你自己的' },
      { k: 'mem', text: '看看它自己记的事', hint: '记忆的开关也在这儿' },
    ];
    var done = 0;
    for (var i = 0; i < items.length; i++) if (s[items[i].k]) done++;
    if (done === items.length) {
      // 先让他看见「完成」这一下（3 秒），再记「已收起」并移除 ——
      // 别立刻把 hidden 记上（那样下一次 paint 会当场抹掉，客户根本没看见）
      el.innerHTML = '<div class="ck-wrap ck-done">✅ 上手完成 —— 以后想改设置，' + whereSettingsText() + '。</div>';
      if (!el.dataset.doneAt) {
        el.dataset.doneAt = String(Date.now());
        setTimeout(function () {
          var ss = ckState(); ss.hidden = 1; ckSave(ss);
          var e2 = document.getElementById('asbudy-checklist'); if (e2) e2.remove();
        }, 3000);
      }
      return;
    }
    var rows = items.map(function (it) {
      return '<button type="button" class="ck-row' + (s[it.k] ? ' on' : '') + '" data-ck="' + it.k + '">' +
        '<span class="ck-box">' + (s[it.k] ? '✓' : '') + '</span>' +
        '<span class="ck-txt">' + it.text + '<small>' + it.hint + '</small></span>' +
        '<span class="ck-go">›</span></button>';
    }).join('');
    el.innerHTML = '<div class="ck-wrap"><div class="ck-hd"><b>上手 ' + done + '/' + items.length + '</b>' +
      '<i class="ck-bar"><i style="width:' + Math.round(done / items.length * 100) + '%"></i></i>' +
      '<button type="button" class="ck-x" id="ck-hide">收起</button></div>' + rows + '</div>';
    el.querySelectorAll('[data-ck]').forEach(function (b) {
      b.onclick = function () {
        var k = b.getAttribute('data-ck');
        if (k === 'act') {
          ckSet('act');
          var box = document.querySelector('.composer textarea, .composer input, #composer textarea, #composer input');
          if (box) { try { box.focus(); } catch (e) { /* 无所谓 */ } }
        } else if (k === 'model') { ckSet('model'); openModelApiLoader(); }
        else if (k === 'mem') { ckSet('mem'); openMemory(); }
      };
    });
    el.querySelector('#ck-hide').onclick = function () {
      var s2 = ckState(); s2.hidden = 1; ckSave(s2);
      var e3 = document.getElementById('asbudy-checklist'); if (e3) e3.remove();
    };
  }

  /* ① 自动判定：他真的跟 AI 说过话（对话里出现了 AI 回复）→ 自动打勾 */
  (function watchFirstTurn() {
    var n = 0;
    var t = setInterval(function () {
      n++;
      if (document.querySelector('article.message.agent')) { ckSet('act'); clearInterval(t); }
      else if (n > 400) clearInterval(t);                    // 10 分钟还没聊过 → 不再盯
    }, 1500);
    ckPaint();
  })();

  /* 「设置在哪」得**分屏宽说**（2026-09-16 实测）：
   *   宽屏：logo 就在左上角（元素 x=18 y=15）→「点左上角那个标记」
   *   窄屏/手机：左上角是「会话」按钮（拉开侧栏用），**logo 在屏幕外**（x=-380）→
   *     必须先让他点「会话」把栏拉出来，再说「最上面那个标记」。
   *   ⚠️ 之前一律写「点左上角那个标记」，手机客户点下去是拉开侧栏，找不到设置。 */
  function whereSettingsText() {
    return window.matchMedia('(max-width: 800px)').matches
      ? '点左上角「会话」，再点最上面那个标记'
      : '点左上角那个标记';
  }

  /* ① 第一次进项目：设置入口在左上角的标记（不遮屏，几秒后自己消失） */
  setTimeout(function () { abTipTop('where-settings', '想换模型、改设置？' + whereSettingsText() + '。'); }, 2600);

  /* 进了别人的视角 → 弹一次提示（附三 §13：替别人操作是敏感事，得让你清楚自己在谁的界面里） */
  (function () {
    var box = document.getElementById('asbudy-files');
    var vw = box ? (box.getAttribute('data-viewas') || '') : '';
    var nm = box ? (box.getAttribute('data-viewasname') || vw) : '';
    if (!vw) { try { sessionStorage.removeItem('ab-viewas-notice'); } catch (e) {} return; }
    try {
      if (sessionStorage.getItem('ab-viewas-notice') === vw) return;   // 同一个视角只提示一次
      sessionStorage.setItem('ab-viewas-notice', vw);
    } catch (e) { /* 隐私模式等存不了：不记住，照弹 */ }
    setTimeout(function () {
      openLayer('你正在替别人操作', function (body) {
        body.innerHTML =
          '<div style="display:flex;gap:12px;align-items:flex-start">' +
            '<span style="font-size:22px;line-height:1.1;color:#d29922">⚠️</span>' +
            '<div>' +
              '<div style="color:#d29922;font-weight:600;margin-bottom:6px">当前视角：' + esc(nm) + '（' + esc(vw) + '）</div>' +
              '<div style="color:#c9d1d9;font-size:14px;line-height:1.65">' +
                '你现在看到和操作的，都是<b>这个人的东西</b>；每一步都会留痕。' +
                '<div style="color:#8b949e;margin-top:8px">回到自己的界面：点侧栏项目名旁的 <b>⇄</b> → 「退出，回到我自己的」。</div>' +
              '</div>' +
            '</div>' +
          '</div>';
      });
    }, 700);
  })();

  if (!bindLogo()) {
    var tries = 0;
    var t = setInterval(function () { if (bindLogo() || ++tries > 60) clearInterval(t); }, 400);
  }
})();
