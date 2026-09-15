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
    '.brand-mark{cursor:pointer}',
    '.brand-mark:hover{opacity:.85}',
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
    '#asbudy-msgbar{display:flex;gap:8px;padding:0 0 6px 2px}',
    '#asbudy-msgbar button{font:inherit;font-size:13.5px;color:#8b949e;background:transparent;border:1px solid #30363d;border-radius:6px;padding:3px 10px;cursor:pointer}',
    '#asbudy-msgbar button:hover{color:#e6edf3}',
  ].join('\n');
  document.head.appendChild(st);

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
      try {
        var u = typeof url === 'string' ? url : ((url && url.url) || '');
        var m = u.match(/\/v1\/threads\/([^\/?]+)/);
        if (m && m[1] && m[1] !== 'summary') MODEL_THREAD = m[1];
      } catch (e) {}
      return prev.apply(this, arguments);
    };
  })();

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
        '（' + (role === 'admin' ? '管理员' : role === 'staff' ? '员工' : '客户老板') + '）</div>';
      if (role === 'admin' || role === 'customer') {
        html += '<button class="ab-menu-item" id="ab-m-staff">员工管理<small>给员工建账号、分配可看项目、设项目额度</small></button>';
      }
      if (role === 'admin') {
        html += '<button class="ab-menu-item" id="ab-m-users">客户管理<small>建客户账号、把项目转给客户</small></button>';
      }
      html += '<button class="ab-menu-item" id="ab-m-proj">项目管理<small>暂停（停引擎、省内存）/ 恢复 / 删除</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-space">空间<small>磁盘用量、每个项目占多少 / 上限多少</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-adv">高级设置<small>代码存哪里（git）/ 过程显示多详细 / 只看不改 / 花了多少</small></button>';
      if (role === 'customer') {
        html += '<button class="ab-menu-item" id="ab-m-consent">平台协助<small>让 AsBudy 平台协助你排查问题（只有你能开，随时可关）</small></button>';
      }
      html += '<button class="ab-menu-item" id="ab-m-pw">修改密码<small>改自己的登录密码</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-logout">退出登录<small>退出当前账号</small></button>';
      if (installEvt) html += '<button class="ab-menu-item" id="ab-m-install">装到桌面<small>把这个页面装成桌面应用</small></button>';
      body.innerHTML = html;
      var bStaff = body.querySelector('#ab-m-staff');
      if (bStaff) bStaff.onclick = function () { openStaff(role === 'admin' ? null : ME.user); };
      var bUsers = body.querySelector('#ab-m-users');
      if (bUsers) bUsers.onclick = openUsers;
      body.querySelector('#ab-m-proj').onclick = openProjects;
      var bSpace = body.querySelector('#ab-m-space');
      if (bSpace) bSpace.onclick = openSpace;
      var bAdv = body.querySelector('#ab-m-adv');
      if (bAdv) bAdv.onclick = openAdvanced;
      var bConsent = body.querySelector('#ab-m-consent');
      if (bConsent) bConsent.onclick = openConsent;
      body.querySelector('#ab-m-pw').onclick = openPassword;
      var bIns = body.querySelector('#ab-m-install');
      if (bIns) bIns.onclick = function () { if (installEvt) { installEvt.prompt(); installEvt = null; closeLayer(); } };
      body.querySelector('#ab-m-logout').onclick = function () {
        if (!confirm('退出登录？')) return;
        fetch('/_gate/logout', { method: 'POST', credentials: 'same-origin' })
          .then(function () { location.replace('/login.html'); })
          .catch(function () { location.replace('/login.html'); });
      };
    });
  }

  /* ── 员工管理 ── */
  function openStaff(owner) {
    // 管理员没指定客户时，先让选一个（后端对 admin 要求带 ?owner=）
    if (!owner && ME && ME.role === 'admin') {
      api('/_gate/users').then(function (r) {
        var users = (r.body && r.body.users) || [];
        openLayer('员工管理 · 先选归属', function (body) {
          body.innerHTML = '<div class="ab-tip">员工归谁？平台自己（内部）或某个客户。</div>' +
            '<button class="ab-menu-item" data-u="admin">平台自己的员工<small>归平台（你直接管）</small></button>' +
            users.filter(function (u) { return (u.role || 'customer') === 'customer'; }).map(function (u) {
              return '<button class="ab-menu-item" data-u="' + esc(u.user) + '">' + esc(u.name || u.user) + '<small>' + esc(u.user) + '</small></button>';
            }).join('');
          body.querySelectorAll('button[data-u]').forEach(function (b) {
            b.onclick = function () { openStaff(b.getAttribute('data-u')); };
          });
        });
      });
      return;
    }
    var q = owner ? ('?owner=' + encodeURIComponent(owner)) : '';
    Promise.all([api('/_gate/projects'), api('/_gate/staff' + q)]).then(function (rs) {
      var projects = (rs[0].body && rs[0].body.projects) || [];
      var staff = (rs[1].body && rs[1].body.staff) || [];
      openLayer('员工管理' + (owner ? '（' + (owner === 'admin' ? '平台自己的' : esc(owner)) + '）' : ''), function (body) {
        body.innerHTML =
          '<div class="ab-tip">给员工建账号、设「最多能建几个项目」、勾选「能看能操作哪几个项目」。<br>' +
          '员工自己建的项目归<b>客户公司</b>（你可见可管）；删员工时项目转回你名下，<b>不删项目</b>。</div>' +
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
                if (!confirm('删掉员工「' + (s.name || s.user) + '」？\n他建的项目会转回你名下（项目本身不删）。')) return;
                api('/_gate/staff', { method: 'DELETE', body: JSON.stringify({ user: s.user }) }).then(function (r) {
                  if (r.ok) { refresh(); } else { alert(r.body.error || '删除失败'); }
                });
              };
              // 「进他的视角」—— 下级的东西不并排铺在我这儿（附三 §13 原则④）
              var bView = document.createElement('button'); bView.className = 'ab-btn ghost sm'; bView.type = 'button'; bView.textContent = '进他的视角';
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
        '<div class="ab-row"><label>资料空间</label><input class="ab-input" id="f-quotamb" type="number" min="0" placeholder="MB，留空 = 默认" value="' + (isNew || !rec.quotaMb ? '' : esc(rec.quotaMb)) + '" style="max-width:130px"><span style="color:#8b949e;font-size:13.5px">他能上传多少资料（不能超过你自己的）</span></div>' +
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
            var bDel = document.createElement('button');
            bDel.className = 'ab-btn danger sm';
            bDel.type = 'button';
            bDel.textContent = '删除';
            bDel.onclick = function () {
              if (!confirm('删掉项目「' + p.name + '」？\n默认只下线（文件不动）；真要连文件一起删，得在服务器上跑 --purge。')) return;
              api('/_gate/projects/delete', { method: 'POST', body: JSON.stringify({ key: p.key }) }).then(function (r2) {
                if (!r2.ok) { alert((r2.body && r2.body.error) || '删不掉'); return; }
                refresh();
              });
            };
            acts.appendChild(bDel);
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

  /* ── 高级设置（git 远程 / 详细度 / 只看不改 / 花费） ── */
  function openAdvanced() {
    openLayer('高级设置', function (body) {
      body.innerHTML = '<div id="ab-adv">加载中…</div>';
      Promise.all([api('/_gate/advanced'), api('/_gate/model-key')]).then(function (rs) {
        var r = rs[0];
        var mk = rs[1].body || {};
        var el = body.querySelector('#ab-adv');
        if (!el) return;
        if (!r.ok) {
          el.innerHTML = '<div class="ab-tip">' + esc((r.body && r.body.error) || '读不到') + '</div>';
          return;
        }
        var d = r.body || {};
        var u = d.usage;
        el.innerHTML =
          '<div class="ab-tip">只影响当前项目（' + esc((d.project && d.project.name) || '') + '）。</div>' +
          '<div class="ab-row"><label>模型服务</label><span class="ab-input" style="cursor:default;color:#8b949e;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">' +
            esc(mk.provider || '—') + ' · ' + esc(mk.model || '—') + ' · ' + (mk.hasKey ? '密钥已配好' : '还没配密钥') +
          '</span><button class="ab-btn ghost sm" id="adv-mk" type="button" style="flex:0 0 auto">改</button></div>' +
          '<div class="ab-row"><label>代码存哪</label><input class="ab-input" id="adv-git" placeholder="git@gitee.com:某人/仓库.git" value="' + esc(d.gitRemote || '') + '"></div>' +
          '<div style="display:flex;gap:8px;margin:-2px 0 14px 78px"><button class="ab-btn ghost sm" id="adv-git-save" type="button">保存</button><button class="ab-btn ghost sm" id="adv-git-clear" type="button">清空</button></div>' +
          '<div class="ab-row"><label>过程多详细</label><input class="ab-input" id="adv-lines" type="number" min="0" max="50" value="' + (d.thinkLines != null ? d.thinkLines : 3) + '" style="max-width:110px"><span style="color:#8b949e;font-size:13.5px">折叠时显示几行</span></div>' +
          '<div style="margin:-2px 0 14px 78px"><button class="ab-btn ghost sm" id="adv-lines-save" type="button">保存</button></div>' +
          '<label class="ab-chk"><input type="checkbox" id="adv-ro"' + (d.previewReadOnly ? ' checked' : '') + '> 只看不改（防误删）</label>' +
          '<div style="margin:14px 0 6px;color:#e6edf3;font-size:14px">花了多少</div>' +
          (u
            ? '<div class="ab-card"><div class="ab-s">累计 ￥' + Number(u.costCny || 0).toFixed(2) + ' ｜ 改动 ' + (u.turns || 0) + ' 次<br>进 ' + Math.round((u.inTok || 0) / 1000) + 'K / 出 ' + Math.round((u.outTok || 0) / 1000) + 'K token</div></div>'
            : '<div class="ab-tip">这个项目还没有用量记录。</div>') +
          '<div class="ab-msg" id="adv-msg"></div>';
        var msgEl = el.querySelector('#adv-msg');
        function post(payload) {
          return api('/_gate/advanced', { method: 'POST', body: JSON.stringify(payload) }).then(function (r2) {
            if (r2.ok) msg(msgEl, '已保存', true); else msg(msgEl, (r2.body && r2.body.error) || '保存失败', false);
          });
        }
        el.querySelector('#adv-mk').onclick = openModelApiLoader;
        el.querySelector('#adv-git-save').onclick = function () { post({ gitRemote: el.querySelector('#adv-git').value }); };
        el.querySelector('#adv-git-clear').onclick = function () { el.querySelector('#adv-git').value = ''; post({ gitRemote: '' }); };
        el.querySelector('#adv-lines-save').onclick = function () { post({ thinkLines: Number(el.querySelector('#adv-lines').value) }); };
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

  /* ── 修改密码 ── */
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
      b1.textContent = '↻ 重试这轮';
      b1.title = '让 AI 重新回答上一轮';
      var b2 = document.createElement('button');
      b2.type = 'button';
      b2.id = 'asbudy-undo-turn';   // 注意：不能叫 asbudy-undo —— 那是退回面板容器的 id（重复 id 会让 getElementById 拿到错的）
      b2.textContent = '↩ 撤销这轮';
      b2.title = '撤回上一轮（含它改的文件）';
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
      if (!LAST_THREAD) { el.textContent = ''; return; }
      try {
        var r = await fetch('/_gate/context?thread=' + encodeURIComponent(LAST_THREAD), { credentials: 'same-origin' });
        if (!r.ok) { el.textContent = ''; return; }
        var d = await r.json();
        if (!d || !d.available) { el.textContent = ''; return; }
        var hot = d.percent >= 80;
        el.textContent = '记性 ' + d.percent + '%';
        el.style.color = hot ? '#f85149' : '#8b949e';
        el.title = '这次对话占了模型「记忆」的 ' + d.percent + '%（' + fmtK(d.used) + ' / ' + fmtK(d.window) + '）'
          + (hot ? '\n快满了 —— 开个新对话，AI 会更清醒' : '');
      } catch (e) { el.textContent = ''; }
    }
    setInterval(loadCtx, 15000);
    setTimeout(loadCtx, 3000);

    async function fire(kind) {
      if (!LAST_THREAD) { alert('先在右边说一句，才有可操作的对话'); return; }
      var labels = { retry: '重试', undo: '撤销', compact: '压缩' };
      if (kind === 'undo' && !confirm('撤销上一轮？AI 这一轮改的文件也会回退。')) return;
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
    logo.title = '我的';
    logo.addEventListener('click', openMyMenu);
    return true;
  }

  api('/_gate/whoami').then(function (r) { if (r.ok) ME = r.body; });

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
