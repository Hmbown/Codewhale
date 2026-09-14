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
    '.ab-x{color:#8b949e;cursor:pointer;font-size:13px;border:1px solid #30363d;border-radius:6px;padding:3px 10px;background:transparent}',
    '.ab-x:hover{color:#e6edf3;border-color:#8b949e}',
    '.ab-body{padding:14px 18px;overflow:auto}',
    '.ab-menu-item{display:block;width:100%;text-align:left;padding:12px 14px;border:1px solid #21262d;border-radius:8px;margin-bottom:8px;cursor:pointer;background:transparent;color:#e6edf3;font-size:14px}',
    '.ab-menu-item:hover{border-color:#58a6ff;background:#58a6ff0d}',
    '.ab-menu-item small{display:block;color:#8b949e;font-size:12px;margin-top:3px}',
    '.ab-card{border:1px solid #21262d;border-radius:9px;padding:11px 13px;margin-bottom:9px}',
    '.ab-card-top{display:flex;justify-content:space-between;align-items:center;gap:10px}',
    '.ab-n{color:#e6edf3;font-size:14px}',
    '.ab-s{color:#8b949e;font-size:12px;margin-top:3px;line-height:1.5}',
    '.ab-btn{background:#238636;color:#fff;border:none;border-radius:7px;padding:7px 14px;font-size:13px;cursor:pointer}',
    '.ab-btn:hover{background:#2ea043}',
    '.ab-btn.ghost{background:transparent;color:#8b949e;border:1px solid #30363d}',
    '.ab-btn.ghost:hover{color:#e6edf3;border-color:#8b949e}',
    '.ab-btn.danger{background:#8b2c2c}.ab-btn.danger:hover{background:#a33}',
    '.ab-btn.sm{padding:4px 10px;font-size:12px}',
    '.ab-row{display:flex;gap:8px;align-items:center;margin-bottom:10px}',
    '.ab-row>label{color:#8b949e;font-size:13px;min-width:70px}',
    '.ab-input{flex:1;background:#010409;border:1px solid #30363d;border-radius:7px;color:#e6edf3;padding:7px 10px;font-size:13px;box-sizing:border-box}',
    '.ab-input:focus{outline:none;border-color:#58a6ff}',
    '.ab-chk{display:flex;align-items:center;gap:8px;padding:6px 0;color:#e6edf3;font-size:13px;cursor:pointer}',
    '.ab-tip{color:#8b949e;font-size:12.5px;line-height:1.6;margin-bottom:12px}',
    '.ab-msg{font-size:13px;margin-top:10px;min-height:16px}',
    '.ab-msg.err{color:#f85149}.ab-msg.ok{color:#3fb950}',
    '.ab-chip{display:flex;align-items:center;gap:8px;margin:0 0 8px;padding:6px 10px;border:1px solid #3b7ddd66;background:#3b7ddd14;border-radius:8px;font-size:12.5px;color:#e6edf3;width:fit-content}',
    '.ab-chip b{color:#58a6ff;font-weight:600}',
    '.ab-chip-x{cursor:pointer;color:#8b949e;padding:0 4px;font-size:15px;line-height:1}',
    '.ab-chip-x:hover{color:#f85149}',
    '.fact-chip[data-asbudy-model]{cursor:pointer}',
    '.fact-chip[data-asbudy-model] strong{text-decoration:underline;text-underline-offset:2px;text-decoration-style:dotted}',
    '.fact-chip[data-asbudy-model]:hover strong{color:#58a6ff}',
    '#asbudy-tick{font-size:12.5px;color:#8b949e;padding:0 0 6px 2px}',
    '#asbudy-msgbar{display:flex;gap:8px;padding:0 0 6px 2px}',
    '#asbudy-msgbar button{font:inherit;font-size:12px;color:#8b949e;background:transparent;border:1px solid #30363d;border-radius:6px;padding:3px 10px;cursor:pointer}',
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

  /* ── 模型小标签可点（对话区上方的「模型: xxx」——比藏在菜单里好找）── */
  function openModelPicker() {
    api('/_gate/model').then(function (r) {
      var models = (r.body && r.body.models) || [];
      var cur = (r.body && r.body.current) || '';
      if (!models.length) { alert('读不到模型列表'); return; }
      openLayer('换个模型', function (body) {
        body.innerHTML = '<div class="ab-tip">当前项目用的模型。换完下一轮对话生效。</div>' +
          models.map(function (m) {
            var on = m.id === cur;
            return '<button class="ab-menu-item" data-m="' + esc(m.id) + '" style="' + (on ? 'border-color:#58a6ff' : '') + '">' +
              esc(m.name) + (on ? '（当前）' : '') + '<small>' + esc(m.note || '') + '</small></button>';
          }).join('') +
          '<div class="ab-msg" id="mp-msg"></div>';
        var msgEl = body.querySelector('#mp-msg');
        body.querySelectorAll('button[data-m]').forEach(function (b) {
          b.onclick = function () {
            api('/_gate/model', { method: 'POST', body: JSON.stringify({ model: b.getAttribute('data-m') }) }).then(function (r2) {
              if (!r2.ok) { msg(msgEl, (r2.body && r2.body.error) || '换不了', false); return; }
              msg(msgEl, '换好了，下一轮生效', true);
              setTimeout(closeLayer, 900);
            });
          };
        });
      });
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
      new MutationObserver(bindModelChip).observe(facts, { childList: true, subtree: true });
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
      html += '<button class="ab-menu-item" id="ab-m-new">新建项目<small>从零开始（带示例 / 空白）</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-import">导入已有项目<small>我已经有一套代码 / 系统，搬进来</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-proj">项目管理<small>暂停（停引擎、省内存）/ 恢复 / 删除</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-files">我的文件<small>传资料（Excel / 合同 / 代码），不绑项目</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-space">空间<small>磁盘用量、每个项目占多少 / 上限多少</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-adv">高级设置<small>代码存哪里（git）/ 过程显示多详细 / 只看不改 / 花了多少</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-pw">修改密码<small>改自己的登录密码</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-logout">退出登录<small>退出当前账号</small></button>';
      if (installEvt) html += '<button class="ab-menu-item" id="ab-m-install">装到桌面<small>把这个页面装成桌面应用</small></button>';
      body.innerHTML = html;
      var bStaff = body.querySelector('#ab-m-staff');
      if (bStaff) bStaff.onclick = function () { openStaff(role === 'admin' ? null : ME.user); };
      var bUsers = body.querySelector('#ab-m-users');
      if (bUsers) bUsers.onclick = openUsers;
      var bNew = body.querySelector('#ab-m-new');
      if (bNew) bNew.onclick = openNewProject;
      var bImp = body.querySelector('#ab-m-import');
      if (bImp) bImp.onclick = openImportProject;
      body.querySelector('#ab-m-proj').onclick = openProjects;
      var bFiles = body.querySelector('#ab-m-files');
      if (bFiles) bFiles.onclick = openFiles;
      var bSpace = body.querySelector('#ab-m-space');
      if (bSpace) bSpace.onclick = openSpace;
      var bAdv = body.querySelector('#ab-m-adv');
      if (bAdv) bAdv.onclick = openAdvanced;
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
        openLayer('员工管理 · 先选客户', function (body) {
          body.innerHTML = '<div class="ab-tip">先选一个客户，看它名下的员工。</div>' +
            users.map(function (u) {
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
      openLayer('员工管理' + (owner ? '（' + esc(owner) + '）' : ''), function (body) {
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
                (s.projects && s.projects.length ? ' ｜ 已自建：' + esc(s.projects.join('、')) : '') + '<br>' +
                '可看项目：' + (granted.length ? esc(granted.join('、')) : '<span style="color:#d29922">未分配</span>') + '</div></div></div>';
              var acts = document.createElement('div');
              acts.style.cssText = 'display:flex;gap:7px;margin-top:10px';
              var bEdit = document.createElement('button'); bEdit.className = 'ab-btn ghost sm'; bEdit.type = 'button'; bEdit.textContent = '编辑';
              bEdit.onclick = function () { openStaffForm(s, owner, projects, staff, reopen); };
              var bDel = document.createElement('button'); bDel.className = 'ab-btn danger sm'; bDel.type = 'button'; bDel.textContent = '删除';
              bDel.onclick = function () {
                if (!confirm('删掉员工「' + (s.name || s.user) + '」？\n他建的项目会转回你名下（项目本身不删）。')) return;
                api('/_gate/staff', { method: 'DELETE', body: JSON.stringify({ user: s.user }) }).then(function (r) {
                  if (r.ok) { refresh(); } else { alert(r.body.error || '删除失败'); }
                });
              };
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
        '<div class="ab-row"><label>项目额度</label><input class="ab-input" id="f-quota" type="number" min="0" max="50" value="' + (isNew ? 2 : esc(rec.quota)) + '" style="max-width:110px"><span style="color:#8b949e;font-size:12px">最多能自己建几个项目</span></div>' +
        '<div style="margin:12px 0 6px;color:#e6edf3;font-size:13px">能看能操作的项目' + (isNew ? '（建完再分配也行）' : '') + '</div>' +
        '<div id="f-projs">' + (projects.length
          ? projects.map(function (p) {
              var on = !isNew && (rec.grants || []).indexOf(p.key) >= 0;
              return '<label class="ab-chk"><input type="checkbox" value="' + esc(p.key) + '"' + (on ? ' checked' : '') + '> ' + esc(p.name) + ' <span style="color:#8b949e;font-size:12px">（' + esc(p.key) + '）</span></label>';
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
        var grants = [];
        body.querySelectorAll('#f-projs input[type=checkbox]').forEach(function (c) { if (c.checked) grants.push(c.value); });
        if (!isNew && !pw && !name) { /* 允许只改配额/授权 */ }
        var payload = { user: uname, displayName: name, quota: quota };
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
  function openUsers() {
    api('/_gate/users').then(function (r) {
      if (!r.ok) { alert(r.body.error || '打不开'); return; }
      var users = (r.body && r.body.users) || [];
      openLayer('客户管理', function (body) {
        body.innerHTML =
          '<div class="ab-tip">客户账号 = 一个客户公司。客户老板登录后能自己给员工建账号、分项目。<br>把项目转给客户：在项目上设归属（管理员）。</div>' +
          '<button class="ab-btn" id="ab-add-cust" type="button">+ 添加客户</button>' +
          '<div id="ab-ulist" style="margin-top:14px"></div>';
        var listEl = body.querySelector('#ab-ulist');
        users.forEach(function (u) {
          var card = document.createElement('div'); card.className = 'ab-card';
          card.innerHTML = '<div class="ab-card-top"><div><div class="ab-n">' + esc(u.name || u.user) + '</div>' +
            '<div class="ab-s">账号：' + esc(u.user) + ' ｜ 名下项目：' + ((u.projects && u.projects.length) ? esc(u.projects.join('、')) : '无') + '</div></div></div>';
          var acts = document.createElement('div'); acts.style.cssText = 'display:flex;gap:7px;margin-top:10px';
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
        '<div style="display:flex;gap:8px;margin-top:16px"><button class="ab-btn" id="c-save" type="button">保存</button>' +
        '<button class="ab-btn ghost" id="c-cancel" type="button">取消</button></div><div class="ab-msg" id="c-msg"></div>';
      var msgEl = body.querySelector('#c-msg');
      body.querySelector('#c-cancel').onclick = closeLayer;
      body.querySelector('#c-save').onclick = function () {
        var payload = {
          user: body.querySelector('#c-user').value.trim(),
          displayName: body.querySelector('#c-name').value.trim(),
          password: body.querySelector('#c-pw').value,
        };
        api('/_gate/users', { method: 'POST', body: JSON.stringify(payload) }).then(function (r) {
          if (!r.ok) { msg(r.body.error || '保存失败', msgEl); return; }
          closeLayer(); if (onDone) onDone();
        });
      };
    });
  }

  /* ── 导入已有项目（把客户现有的代码 / 系统搬进来）── */
  function openImportProject() {
    api('/_gate/files').then(function (r) {
      var files = (r.body && r.body.files) || [];
      var dirs = files.filter(function (n) { return n.isDir; });
      openLayer('导入已有项目', function (body) {
        var helpHtml =
          '<div class="ab-tip" id="imp-help" hidden style="border:1px solid #30363d;border-radius:8px;padding:10px 12px;background:#161b22">' +
          '<b style="color:#e6edf3">这是干什么的</b><br>' +
          '把你现有的代码 / 系统搬进来，AI 就能在上面干活。<br><br>' +
          '<b style="color:#e6edf3">自动跳过的</b><br>' +
          'node_modules、.git、venv、__pycache__、dist 这些「装出来的」目录 —— 不用传，来了自己装。<br><br>' +
          '<b style="color:#e6edf3">搬完之后</b><br>' +
          '建议跟 AI 说一句：「帮我把环境装好，跑起来」——它会自己看代码、装依赖、起服务。' +
          '</div>';
        body.innerHTML =
          '<div style="display:flex;align-items:center;gap:8px;margin-bottom:10px">' +
          '<span class="ab-tip" style="margin:0">把已上传的文件夹搬成一个新项目</span>' +
          '<span id="imp-q" title="这是干什么的" style="cursor:pointer;width:18px;height:18px;line-height:18px;text-align:center;border:1px solid #30363d;border-radius:50%;color:#8b949e;font-size:12px;flex:none">?</span>' +
          '</div>' + helpHtml +
          '<div class="ab-row"><label>名字</label><input class="ab-input" id="imp-name" placeholder="如：我原来的客户系统"></div>' +
          '<div class="ab-row"><label>从哪导</label>' +
          (dirs.length
            ? '<select class="ab-input" id="imp-src">' + dirs.map(function (d) { return '<option value="' + esc(d.path) + '">' + esc(d.name) + '</option>'; }).join('') + '</select>'
            : '<span style="color:#d29922;font-size:12.5px">还没有上传过文件夹</span>') +
          '</div>' +
          (dirs.length ? '' : '<div class="ab-tip">先去侧栏「项目文件」面板 →「我的文件」→「+ 上传」→「选整个文件夹」，传完再回来。</div>') +
          '<div style="display:flex;gap:8px;margin-top:16px">' +
          '<button class="ab-btn" id="imp-go" type="button"' + (dirs.length ? '' : ' disabled') + '>导入</button>' +
          '<button class="ab-btn ghost" id="imp-cancel" type="button">取消</button></div>' +
          '<div class="ab-msg" id="imp-msg"></div>';
        var msgEl = body.querySelector('#imp-msg');
        body.querySelector('#imp-q').onclick = function () {
          var h = body.querySelector('#imp-help');
          h.hidden = !h.hidden;
        };
        body.querySelector('#imp-cancel').onclick = closeLayer;
        body.querySelector('#imp-go').onclick = function () {
          var name = body.querySelector('#imp-name').value.trim();
          var srcEl = body.querySelector('#imp-src');
          if (!name) { msg(msgEl, '起个名字吧', false); return; }
          if (!srcEl) return;
          var go = body.querySelector('#imp-go');
          go.disabled = true;
          msg(msgEl, '正在搬，约 30~60 秒…', true);
          api('/_gate/projects/create', { method: 'POST', body: JSON.stringify({ name: name, template: 'blank', from: srcEl.value }) })
            .then(function (r2) {
              go.disabled = false;
              if (!r2.ok) { msg(msgEl, (r2.body && r2.body.error) || '没导进来', false); return; }
              // 搬完的引导：告诉用户下一步让 AI 装环境
              body.querySelector('.ab-body, body') && null;
              openLayer('导入好了', function (b2) {
                b2.innerHTML =
                  '<div style="color:#3fb950;font-size:15px;margin-bottom:10px">「' + esc(name) + '」已导入。</div>' +
                  '<div class="ab-tip">代码已经搬进来了，但环境（依赖、启动）还没装。<br><br>' +
                  '<b style="color:#e6edf3">下一步：</b>切到这个项目，跟它说一句 ——<br>' +
                  '<span style="display:block;margin:8px 0;padding:8px 12px;background:#161b22;border:1px solid #30363d;border-radius:8px;color:#58a6ff">' +
                  '帮我把环境装好，跑起来</span>' +
                  '它会自己看代码、装依赖、起服务。</div>' +
                  '<div style="display:flex;gap:8px;margin-top:14px"><button class="ab-btn" id="imp-done" type="button">去看看</button></div>';
                b2.querySelector('#imp-done').onclick = function () { location.reload(); };
              });
            });
        };
      });
    });
  }

  /* ── 新建项目 ── */
  function openNewProject() {
    openLayer('新建项目', function (body) {
      body.innerHTML =
        '<div class="ab-tip">每个项目是一套独立系统（自己的引擎 + 自己的工作区）。建一个约 30~60 秒。</div>' +
        '<div class="ab-row"><label>名字</label><input class="ab-input" id="np-name" placeholder="如：客户管理系统"></div>' +
        '<div class="ab-row"><label>一句话说</label><input class="ab-input" id="np-note" placeholder="可选，这个系统干什么用"></div>' +
        '<div style="margin:12px 0 6px;color:#e6edf3;font-size:13px">从哪开始</div>' +
        '<label class="ab-chk"><input type="radio" name="np-tpl" value="example" checked> 带示例（有客户 / 跟进 / 订单三张样例表）</label>' +
        '<label class="ab-chk"><input type="radio" name="np-tpl" value="blank"> 空白（从零开始）</label>' +
        '<div style="display:flex;gap:8px;margin-top:16px"><button class="ab-btn" id="np-go" type="button">建</button>' +
        '<button class="ab-btn ghost" id="np-cancel" type="button">取消</button></div>' +
        '<div class="ab-msg" id="np-msg"></div>';
      var msgEl = body.querySelector('#np-msg');
      body.querySelector('#np-cancel').onclick = closeLayer;
      body.querySelector('#np-go').onclick = function () {
        var nameEl = body.querySelector('#np-name');
        var name = nameEl.value.trim();
        var note = body.querySelector('#np-note').value.trim();
        var tplEl = body.querySelector('input[name=np-tpl]:checked');
        if (!name) { msg(msgEl, '起个名字吧', false); return; }
        var go = body.querySelector('#np-go');
        go.disabled = true;
        msg(msgEl, '正在建，约 30~60 秒…', true);
        api('/_gate/projects/create', { method: 'POST', body: JSON.stringify({ name: name, note: note, template: tplEl ? tplEl.value : 'example' }) })
          .then(function (r) {
            go.disabled = false;
            if (!r.ok) { msg(msgEl, (r.body && r.body.error) || '没建成', false); return; }
            msg(msgEl, '建好了：' + name + '，正在刷新…', true);
            setTimeout(function () { location.reload(); }, 1200);
          });
      };
    });
  }

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
              sel.style.cssText = 'max-width:150px;padding:5px 8px;font-size:12px';
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

  /* ── 我的文件（文件池：上传的资料；不绑项目） ── */
  function openFiles() {
    openLayer('我的文件', function (body) {
      body.innerHTML =
        '<div class="ab-tip">传上来的资料（Excel / 合同 / 代码）。不绑项目，随手放着；要让 AI 用某份，在左侧「项目文件」里点它。</div>' +
        '<div style="display:flex;gap:8px;margin-bottom:12px">' +
        '<button class="ab-btn" id="ab-up-file" type="button">+ 传文件</button>' +
        '<button class="ab-btn ghost" id="ab-up-dir" type="button">+ 传文件夹</button>' +
        '</div>' +
        '<div id="ab-flist" style="margin-top:6px"></div>' +
        '<input type="file" id="ab-file-input" multiple style="display:none">' +
        '<input type="file" id="ab-dir-input" webkitdirectory style="display:none">' +
        '<div class="ab-msg" id="ab-fmsg"></div>';
      var listEl = body.querySelector('#ab-flist');
      var msgEl = body.querySelector('#ab-fmsg');
      var fileInput = body.querySelector('#ab-file-input');
      var dirInput = body.querySelector('#ab-dir-input');

      function renderTree(nodes, container) {
        (nodes || []).forEach(function (n) {
          if (n.isDir) {
            var d = document.createElement('div');
            d.className = 'ab-card';
            d.style.cursor = 'pointer';
            d.innerHTML = '<div class="ab-n">▸ ' + esc(n.name) + '</div>';
            var kids = document.createElement('div');
            kids.style.display = 'none';
            kids.style.marginLeft = '12px';
            d.onclick = function (ev) {
              if (ev.target && ev.target.tagName === 'BUTTON') return;
              var open = kids.style.display !== 'none';
              kids.style.display = open ? 'none' : 'block';
              d.querySelector('.ab-n').textContent = (open ? '▸ ' : '▾ ') + n.name;
            };
            d.appendChild(kids);
            renderTree(n.children, kids);
            container.appendChild(d);
          } else {
            var f = document.createElement('div');
            f.className = 'ab-card';
            f.style.display = 'flex';
            f.style.alignItems = 'center';
            f.style.justifyContent = 'space-between';
            f.innerHTML = '<span class="ab-n">' + esc(n.name) + '</span>' +
              '<button class="ab-btn danger sm" type="button">删除</button>';
            f.querySelector('button').onclick = function () {
              if (!confirm('删掉「' + n.name + '」？')) return;
              api('/_gate/file?name=' + encodeURIComponent(n.path), { method: 'DELETE' }).then(function (r) {
                if (r.ok) { msg(msgEl, '已删除', true); load(); } else { msg(msgEl, (r.body && r.body.error) || '删不掉', false); }
              });
            };
            container.appendChild(f);
          }
        });
      }
      function load() {
        api('/_gate/files').then(function (r) {
          var files = (r.body && r.body.files) || [];
          listEl.innerHTML = '';
          if (!files.length) { listEl.innerHTML = '<div class="ab-tip">还没有文件。</div>'; return; }
          renderTree(files, listEl);
        });
      }
      function upload(files, input) {
        var arr = Array.prototype.slice.call(files || []);
        if (!arr.length) return;
        msg(msgEl, '正在上传 ' + arr.length + ' 个…', true);
        var done = 0;
        var failed = 0;
        var chain = Promise.resolve();
        arr.forEach(function (f) {
          chain = chain.then(function () {
            var rel = f.webkitRelativePath || f.name;
            return fetch('/_gate/upload', {
              method: 'POST',
              headers: { 'Content-Type': 'application/octet-stream', 'X-Filename': encodeURIComponent(rel) },
              body: f,
            }).then(function (res) {
              if (res.ok) { done++; return; }
              failed++;
              return res.json().catch(function () { return {}; }).then(function (j) { if (j.error) msg(msgEl, '传不上去：' + j.error, false); });
            }).catch(function () { failed++; });
          });
        });
        chain.then(function () {
          msg(msgEl, '传完：成功 ' + done + (failed ? '，失败 ' + failed : ''), !failed);
          input.value = '';
          load();
        });
      }
      body.querySelector('#ab-up-file').onclick = function () { fileInput.click(); };
      body.querySelector('#ab-up-dir').onclick = function () { dirInput.click(); };
      fileInput.onchange = function () { upload(fileInput.files, fileInput); };
      dirInput.onchange = function () { upload(dirInput.files, dirInput); };
      load();
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
      Promise.all([api('/_gate/advanced'), api('/_gate/model')]).then(function (rs) {
        var r = rs[0];
        var mdl = rs[1];
        var el = body.querySelector('#ab-adv');
        if (!el) return;
        if (!r.ok) {
          el.innerHTML = '<div class="ab-tip">' + esc((r.body && r.body.error) || '读不到') + '</div>';
          return;
        }
        var d = r.body || {};
        var u = d.usage;
        var models = (mdl.body && mdl.body.models) || [];
        var curModel = (mdl.body && mdl.body.current) || '';
        var modelOpts = models.map(function (x) {
          return '<option value="' + esc(x.id) + '"' + (x.id === curModel ? ' selected' : '') + '>' + esc(x.name) + ' —— ' + esc(x.note || '') + '</option>';
        }).join('');
        el.innerHTML =
          '<div class="ab-tip">只影响当前项目（' + esc((d.project && d.project.name) || '') + '）。</div>' +
          '<div class="ab-row"><label>用哪个模型</label><select class="ab-input" id="adv-model">' + modelOpts + '</select></div>' +
          '<div class="ab-row"><label>代码存哪</label><input class="ab-input" id="adv-git" placeholder="git@gitee.com:某人/仓库.git" value="' + esc(d.gitRemote || '') + '"></div>' +
          '<div style="display:flex;gap:8px;margin:-2px 0 14px 78px"><button class="ab-btn ghost sm" id="adv-git-save" type="button">保存</button><button class="ab-btn ghost sm" id="adv-git-clear" type="button">清空</button></div>' +
          '<div class="ab-row"><label>过程多详细</label><input class="ab-input" id="adv-lines" type="number" min="0" max="50" value="' + (d.thinkLines != null ? d.thinkLines : 3) + '" style="max-width:110px"><span style="color:#8b949e;font-size:12px">折叠时显示几行</span></div>' +
          '<div style="margin:-2px 0 14px 78px"><button class="ab-btn ghost sm" id="adv-lines-save" type="button">保存</button></div>' +
          '<label class="ab-chk"><input type="checkbox" id="adv-ro"' + (d.previewReadOnly ? ' checked' : '') + '> 只看不改（防误删）</label>' +
          '<div style="margin:14px 0 6px;color:#e6edf3;font-size:13px">花了多少</div>' +
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
        function post2(url, payload) {
          return api(url, { method: 'POST', body: JSON.stringify(payload) }).then(function (r2) {
            if (r2.ok) msg(msgEl, '已保存（下轮生效）', true); else msg(msgEl, (r2.body && r2.body.error) || '保存失败', false);
          });
        }
        el.querySelector('#adv-model').onchange = function (e) {
          post2('/_gate/model', { model: e.target.value });
        };
        el.querySelector('#adv-git-save').onclick = function () { post({ gitRemote: el.querySelector('#adv-git').value }); };
        el.querySelector('#adv-git-clear').onclick = function () { el.querySelector('#adv-git').value = ''; post({ gitRemote: '' }); };
        el.querySelector('#adv-lines-save').onclick = function () { post({ thinkLines: Number(el.querySelector('#adv-lines').value) }); };
        el.querySelector('#adv-ro').onchange = function (e) { post({ previewReadOnly: e.target.checked }); };
      });
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
      wrap.parentNode.insertBefore(el, wrap);
      return el;
    }

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
  if (!bindLogo()) {
    var tries = 0;
    var t = setInterval(function () { if (bindLogo() || ++tries > 60) clearInterval(t); }, 400);
  }
})();
