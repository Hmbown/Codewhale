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

  /* ── 操作反馈（2026-09-22 加）─────────────────────────────────────────
   * 【为什么要有】老板报「点归档、新建会话、删除项目或文件…点击后都没有状态显示」。
   *   真浏览器实测：官方那两个操作**本身是好的**，但**成功后一律静默** ——
   *   `archiveThread()` 只在 catch 里 `showStatus(error.message)`（成功什么都不说）；
   *   `quickNewThread()` 直接建一条、连对话框都不弹（实测：点一下 `POST /v1/threads`，
   *   `#new-thread-dialog` 根本没开）。客户点完只能自己去列表里找变化。
   * 【顺带修一个真 bug】下面原来调的 `notify(...)` **在官方界面里根本没定义** ——
   *   `function notify` 在整个 runtime_web 里 grep 不到（desk.html 里那个定义在**父页面**，
   *   iframe 里拿不到）⇒ 插件开关那两处一调就抛 `ReferenceError`，
   *   **连紧跟其后的列表刷新都没执行**。
   * 【做法】不碰官方逻辑：只**旁听** fetch 的响应，成功后给一条我们自己的轻提示。
   *   样式写 inline（不往官方样式表里塞东西，不和官方 CSS 抢）；提提示层 `pointer-events:none`
   *   （不可能挡住任何点击）。 */
  function notify(msg, sticky) {
    try {
      var el = document.getElementById('asbudy-opnote');
      if (!el) {
        el = document.createElement('div');
        el.id = 'asbudy-opnote';
        el.style.cssText = 'position:fixed;left:50%;bottom:26px;transform:translateX(-50%);'
          + 'background:rgba(14,26,48,.96);border:1px solid rgba(72,215,255,.28);color:#f6f2e8;'
          + 'padding:9px 16px;border-radius:9px;font-size:13px;z-index:99999;pointer-events:none;'
          + 'opacity:0;transition:opacity .18s;box-shadow:0 8px 24px rgba(0,0,0,.45)';
        document.body.appendChild(el);
      }
      el.textContent = String(msg || '');
      el.style.opacity = '1';
      clearTimeout(notify._t);
      if (sticky) return;               // 「进行中」这类要一直挂着，等结果回来再换
      notify._t = setTimeout(function () { el.style.opacity = '0'; }, 2600);
    } catch (e) {}
  }

  /* ── 「正在处理」提示（2026-09-22 老板：「等待期间没有任何状态提示，
     这几秒钟的等待时间用户不知道自己点了没有」）──
     跟桌上那层同一个做法：在 fetch 这一层**统一旁听写操作**，延迟 250ms 才显示
     （快操作不闪），结果回来再换成结果。排除对话（`/turns`、`/events`）——
     那条官方自己有「发送中」，而且要跑很久。 */
  (function () {
    if (window.__asbudyBusy) return;
    window.__asbudyBusy = true;
    var orig = window.fetch;
    var inflight = 0, timer = null, shown = false;
    var noteEl = function () { return document.getElementById('asbudy-opnote'); };
    function start() {
      if (timer !== null) return;
      timer = setTimeout(function () { shown = true; notify('正在处理…', true); }, 250);
    }
    function finish() {
      if (timer !== null) { clearTimeout(timer); timer = null; }
      if (!shown) return;
      setTimeout(function () {
        if (!shown) return;
        shown = false;
        var el = noteEl();
        if (el && el.textContent === '正在处理…') notify('已完成');
        else if (el) { clearTimeout(notify._t); el.style.opacity = '0'; }
      }, 200);
    }
    window.fetch = function (input, init) {
      var url = (typeof input === 'string') ? input : ((input && input.url) || '');
      var m = String((init && init.method) || (input && input.method) || 'GET').toUpperCase();
      if (m === 'GET' || m === 'HEAD' || m === 'OPTIONS' || /\/v1\/threads\/[^/]+\/(turns|events)/.test(url)) {
        return orig.apply(this, arguments);
      }
      inflight++;
      start();
      var p = orig.apply(this, arguments);
      var done = function () { if (--inflight <= 0) { inflight = 0; finish(); } };
      p.then(done, done);
      return p;
    };
  })();

  /* 旁听官方那两个「点完静默」的操作 —— 成功了给一句反馈（不改官方请求、不拦不改） */
  (function () {
    if (window.__asbudyFetchTap) return;
    window.__asbudyFetchTap = true;
    var orig = window.fetch;
    window.fetch = function (input, init) {
      var url = (typeof input === 'string') ? input : ((input && input.url) || '');
      var method = String((init && init.method) || (input && input.method) || 'GET').toUpperCase();
      var body = (init && typeof init.body === 'string') ? init.body : '';
      var p = orig.apply(this, arguments);
      try {
        if (method === 'POST' && /\/v1\/threads(\?|$)/.test(url)) {
          p.then(function (r) { if (r && r.ok) notify('已新建会话'); }, function () {});
        } else if (method === 'PATCH' && /\/v1\/threads\/[^/?]+$/.test(url) && body.indexOf('archived') >= 0) {
          p.then(function (r) {
            if (!r || !r.ok) return;
            notify(/"archived"\s*:\s*true/.test(body) ? '已归档' : '已更新');
          }, function () {});
        }
      } catch (e) {}
      return p;
    };
  })();

  /* ── 样式 ── */
  var st = document.createElement('style');
  st.textContent = [
    // 「我的」入口 = 侧栏左上角那个 logo。它的按钮化样式写在 styles.css 里
    // （带 [data-asbudy-my] 前缀，只有脚本真绑上点击才生效）。
    // 2026-09-15：右下角角标、旁边「我的」文字胶囊 —— 都做过，老板说画蛇添足，已拿掉。
    '#asbudy-layer{position:fixed;inset:0;background:rgba(2,7,17,.72);z-index:99999;display:flex;align-items:center;justify-content:center}',
    // 2026-09-17 老板：设置面板在电脑上是手机样式（宽 420）—— 弹层默认按桌面尺寸来，
    // 小屏再用 92vw 兜住。配色仍是旧的 GitHub 深色（不在这次的改动范围，已单独记档）。
    '.ab-box{background:var(--surface);border:1px solid var(--line);border-radius:12px;width:min(880px,92vw);max-height:88vh;display:flex;flex-direction:column;overflow:hidden;box-shadow:0 12px 48px rgba(0,0,0,.6)}',
    '.ab-head{display:flex;justify-content:space-between;align-items:center;padding:12px 18px;border-bottom:1px solid var(--line);gap:14px}',
    '.ab-title{font-size:15px;color:var(--text);font-weight:600;margin-right:auto}',
    '.ab-x{color:var(--text-dim);cursor:pointer;font-size:14px;border:1px solid var(--line);border-radius:6px;padding:3px 10px;background:transparent}',
    '.ab-x:hover{color:var(--text);border-color:var(--text-dim)}',
    '.ab-back{color:var(--text-dim);cursor:pointer;font:inherit;font-size:14px;border:1px solid var(--line);border-radius:6px;padding:3px 10px;background:transparent;flex:none}',
    '.ab-back:hover{color:var(--text);border-color:var(--text-dim)}',
    '.ab-body{padding:14px 18px;overflow:auto}',
    '.ab-menu-item{display:block;width:100%;text-align:left;padding:12px 14px;border:1px solid var(--line);border-radius:8px;margin-bottom:8px;cursor:pointer;background:transparent;color:var(--text);font-size:14px}',
    '.ab-menu-item:hover{border-color:var(--action);background:var(--action-soft)}',
    '.ab-menu-item small{display:block;color:var(--text-dim);font-size:13.5px;margin-top:3px}',
    '.ab-card{border:1px solid var(--line);border-radius:9px;padding:11px 13px;margin-bottom:9px}',
    '.ab-card-top{display:flex;justify-content:space-between;align-items:center;gap:10px}',
    '.ab-n{color:var(--text);font-size:14px}',
    '.ab-s{color:var(--text-dim);font-size:13.5px;margin-top:3px;line-height:1.5}',
    '.ab-btn{background:var(--live);color:var(--action-contrast);border:none;border-radius:7px;padding:7px 14px;font-size:14px;cursor:pointer}',
    '.ab-btn:hover{background:var(--live)}',
    '.ab-btn.ghost{background:transparent;color:var(--text-dim);border:1px solid var(--line)}',
    '.ab-btn.ghost:hover{color:var(--text);border-color:var(--text-dim)}',
    '.ab-btn.danger{background:rgba(255,134,178,.22)}.ab-btn.danger:hover{background:#a33}',
    '.ab-btn.sm{padding:4px 10px;font-size:13.5px}',
    '.ab-row{display:flex;gap:8px;align-items:center;margin-bottom:10px}',
    '.ab-row>label{color:var(--text-dim);font-size:14px;min-width:70px}',
    '.ab-input{flex:1;background:var(--bg);border:1px solid var(--line);border-radius:7px;color:var(--text);padding:7px 10px;font-size:14px;box-sizing:border-box}',
    '.ab-input:focus{outline:none;border-color:var(--action)}',
    '.ab-chk{display:flex;align-items:center;gap:8px;padding:6px 0;color:var(--text);font-size:14px;cursor:pointer}',
    '.ab-tip{color:var(--text-dim);font-size:13.5px;line-height:1.6;margin-bottom:12px}',
    '.ab-msg{font-size:14px;margin-top:10px;min-height:16px}',
    '.ab-msg.err{color:var(--danger)}.ab-msg.ok{color:var(--live)}',
    '.ab-chip{display:flex;align-items:center;gap:8px;margin:0 0 8px;padding:6px 10px;border:1px solid rgba(106,174,242,.4);background:var(--action-soft);border-radius:8px;font-size:13.5px;color:var(--text);width:fit-content}',
    '.ab-chip b{color:var(--action);font-weight:600}',
    '.ab-chip-x{cursor:pointer;color:var(--text-dim);padding:0 4px;font-size:15px;line-height:1}',
    '.ab-chip-x:hover{color:var(--danger)}',
    '.fact-chip[data-asbudy-model]{cursor:pointer}',
    '.fact-chip[data-asbudy-model] strong{text-decoration:underline;text-underline-offset:2px;text-decoration-style:dotted}',
    '.fact-chip[data-asbudy-model]:hover strong{color:var(--action)}',
    '#asbudy-tick{font-size:13.5px;color:var(--text-dim);padding:0 0 6px 2px}',
    '#asbudy-msgbar{display:flex;flex-wrap:wrap;align-items:center;gap:8px;padding:0 0 6px 2px}',
    '#asbudy-msgbar button{font:inherit;font-size:13.5px;color:var(--text-dim);background:transparent;border:1px solid var(--line);border-radius:6px;padding:3px 10px;cursor:pointer}',
    '#asbudy-msgbar button:hover{color:var(--text)}',
    // 「正在做什么」状态行（2026-09-18）：跟其他小字同档，但用等宽感区分一下
    '#asbudy-live{font-size:13px;color:var(--text-dim);white-space:nowrap}',
    '#asbudy-live[hidden]{display:none}',
    // 会话指标（2026-09-19 · ttft / 平均 tok/s / ↓ tokens）：与状态行同档，不动形态
    '#asbudy-metrics{font-size:13px;color:var(--text-dim);white-space:nowrap}',
    '#asbudy-metrics[hidden]{display:none}',
    // ── 工具卡按类型分开（2026-09-18 · 老板：「CLI 的工具卡是按类型分开渲染的，直接做了吧」）──
    // CLI 那边 9 种卡各画各的（Exec/Exploring/PatchSummary/PlanUpdate/…）；web 这里做形态区分：
    // 圆点颜色 + 排版 + 降噪。形态由 app.mjs 的 receiptVariant 算好，挂在 data-variant 上。
    '.receipt[data-variant="exec"] .receipt-dot{background:var(--ok)}',
    '.receipt[data-variant="file"] .receipt-dot{background:var(--action)}',
    '.receipt[data-variant="plan"] .receipt-dot{background:var(--status-live)}',
    '.receipt[data-variant="web"] .receipt-dot{background:var(--status-live)}',
    '.receipt[data-variant="mcp"] .receipt-dot{background:var(--status-human)}',
    '.receipt[data-variant="status"] .receipt-dot{background:var(--text-faint)}',
    // 命令：等宽字体，一眼看出这是命令不是散文
    '.receipt[data-variant="exec"] .receipt-summary{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:12.5px}',
    // 降噪：查看/搜索低调一点，引擎节拍（进度）最淡 —— 界面的主角应该是 AI 说的话
    '.receipt[data-variant="explore"]{opacity:.82}',
    '.receipt[data-variant="status"]{opacity:.6}',
    // 徽标（耗时 / 退出码）
    '.receipt-meta{color:var(--text-dim);font-size:12.5px;margin-left:6px;white-space:nowrap}',
    '.receipt[data-variant="exec"] .receipt-meta{color:var(--text-muted)}',
    // ── 工具族字形（2026-09-20 搬 · 照 CLI 的 `family_glyph`）──
    //   把 7×7 的圆点换成族字形：`▷` 读 · `◆` 改 · `▶` 跑 · `⌕` 找 · `◐` 子代理 · `✓` 验证 · `•` 其它。
    //   ⚠️ 这两组必须**排在 `[data-variant]` 那组之后** —— 两组特异性相同（2 属性 + 1 类），
    //     靠后定义的赢；排前面的话圆点背景会盖在字形上。
    '.receipt[data-family] .receipt-dot{flex:0 0 auto;width:auto;height:auto;margin-top:0;border-radius:0;background:none;font-size:13px;line-height:1.5;color:var(--text-faint)}',
    '.receipt[data-family="read"] .receipt-dot{color:var(--text-dim)}',
    '.receipt[data-family="patch"] .receipt-dot{color:var(--action)}',
    '.receipt[data-family="run"] .receipt-dot{color:var(--ok)}',
    '.receipt[data-family="find"] .receipt-dot{color:var(--status-live)}',
    '.receipt[data-family="verify"] .receipt-dot{color:var(--ok)}',
    '.receipt[data-family="delegate"] .receipt-dot{color:var(--status-human)}',
    // 失败优先（跟 styles.css 的 `.receipt.failed .receipt-dot` 同义，只是换成前景色）
    '.receipt.failed[data-family] .receipt-dot{color:var(--danger)}',
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
  /* ⚠️ 2026-09-22 对齐官方出厂默认（老板拍「1，对齐」）—— 这几个「读不到时的兜底值」
   * 应当就是**官方默认**，否则引擎读不到时客户看到的是另一套行为。
   * 官方 `crates/tui/src/settings.rs:528-570` 的 `default()` 原文：
   *   · `calm_mode: true`  —— 注释 *"#4095: default presentation is compact/calm; verbose detail is opt-in."*
   *   · `show_thinking: false` —— 注释 *"Reasoning is useful when explicitly requested, but it should never
   *        displace the actual conversation in the default TUI."*
   *   · `thinking_highlight: true` / `inline_diffs: "full"` / `thinking_preview_lines: 2` / `show_tool_details: false`
   * ⚠️ 注意：老板**自己的 CLI 配置**（`~/.codewhale/settings.toml`）是 `calm_mode=false` + `show_thinking=true`
   *   —— 那是他调过的，**不是出厂默认**。这里对齐的是出厂默认。 */
  var DISPLAY = { show_thinking: false, thinking_default_expanded: false, show_tool_details: false, calm_mode: true, cost_currency: 'usd', thinking_highlight: true, inline_diffs: 'full', thinking_preview_lines: 2, statusline_off: null };

  var stD = document.createElement('style');
  stD.textContent = [
    // 不显示思考过程 → 思考卡整个收起来（这条是**官方语义**：`show_thinking` 关＝不显示思考）
    'html[data-ab-think="off"] article.reasoning{display:none!important}',
    // 「显示文件与命令明细」——语义**照官方**（`crates/tui/src/tui/history.rs:471-491` ＋
    //   `tui/history/constants.rs:87`）。官方**关掉**时也**不隐藏内容**，只是**限行**：
    //   工具卡保留头部（`<符号> <动词> <状态> · <摘要>`）＋ 最多 4 行正文 ＋ 一行「展开」提示，
    //   源码注释原话 "enough to answer 'what did that do?' without opening anything"；
    //   而且**失败的工具卡完全不吃这套**（那个渲染分支写着 `!cell.is_failed()`，原样全显示）。
    //   ⚠️ 2026-09-19 修：我们以前把「明细关」做成了**摘要行整行 display:none**，
    //   而 `show_tool_details` 出厂默认就是 **false**（`settings.rs:571` / `:628`）——
    //   于是默认档下界面只剩「工具 · 完成」这种没宾语的标签，客户根本不知道它跑了什么
    //   （老板 2026-09-19：「还是不显示进度反馈、摘要」）。
    //   摘要 ＝ 官方那行 header，**永远显示**；这个开关管的是**明细正文**：
    //   关 = 收进「查看回执」折叠块（点得开）、开 = 默认摊开（见下面 openReceiptDetails）。
    // ── 安静模式：**照官方语义**——「限行」，不是「隐藏」（2026-09-19 对齐）
    //   官方注释（tui/history/constants.rs:100-107）原文：
    //     *"Calm mode is about quiet, **not about hiding**, so it bounds the card at the header
    //      plus the full successful-run preview plus the expand affordance."*
    //   而且官方特意修过一个反直觉的 bug：calm 的上限曾比「明细关」还严（4 行 vs 6 行），
    //   导致「开了明细反而看得更少」—— 现在 calm = `TOOL_CARD_SUMMARY_LINES` ＝ 成功预览 6 行 + 2。
    //   ⚠️ 我们以前写的是 `article.reasoning{display:none}` + `.receipt{display:none}`（整卡抹掉）——
    //     那是自己发明的，跟官方两回事。官方 calm **根本不管思考卡**（思考由 show_thinking 管）。
    //   web 的对应：卡片 = 头部(1) ＋ 摘要(1) ＋ 正文。
    //   ⚠️ 2026-09-19 实测作废：以前这里用 CSS 给**折叠块里那个 `<pre>`** 限高
    //     （`details pre{max-height:6.2em;overflow:hidden}`），本意是复刻官方的「最多 4 行」。
    //     但两边的结构不一样 —— 官方的「限行」是**整张卡**在渲染时 `truncate` 到 6 行
    //     （`tui/history.rs:469-480`，卡里本来就有正文，所以真能看见开头几行）；
    //     而网页的正文**只活在折叠块里**，收起时压根不渲染。那条 CSS 唯一的实际效果就是：
    //     **客户点开「查看回执」也看不全，而且 `overflow:hidden` 连滚都滚不动。**
    //     真浏览器实测（2026-09-19）：展开后 `clientHeight 82px` vs `scrollHeight 197px`，
    //     `overflow-y: hidden` —— 后面的内容直接不存在。
    //     所以这两条**删掉**，正文回到官方 `styles.css` 的 `.receipt pre{max-height:320px;overflow:auto}`：
    //     展开即看得到、长了能滚。明细开关的语义回到「要不要默认摊开折叠块」（`openReceiptDetails`），
    //     不再假装自己在做行数限制。
    //   ⚠️ 遗留：安静模式（calm）在官方也是「限行」（8 行）—— 同一个道理，网页上它同样没有落点，
    //     不再用 CSS 伪装。真要复刻官方的「默认露开头几行」，得先给卡加一个"默认可见的正文预览"元素
    //     （像思考卡那个 `ab-think-preview`），那是**形态改动**，得先跟老板定。
    // ── 工具卡「默认可见的正文预览」（2026-09-19 加 · 照官方明细关）──
    //   官方 `tui/history.rs:469-480`：明细关时整张卡 `truncate(TOOL_SUMMARY_CARD_LINES)`
    //   ＝「头部 ＋ 最多 4 行正文 ＋ 展开提示」；失败卡不吃这套，走完整 20 行预算。
    //   行数与排序都在 app.mjs 的 `selectedOutputLines()` 里算好，这里只管排版。
    //   ⚠️ 展开「查看回执」时让位 —— 否则同一段内容在上面（预览）和下面（全文）各出现一次。
    // ⚠️ 2026-09-22 修：官方 `.receipt` 是 `display:flex` ＋ **width:fit-content**（而且**不换行**），
    //   而我们往它里面直接 append 了两个块级 <pre>（ab-out-preview / ab-diff）⇒ 它们被当成
    //   **横向的列**参与排版，把左边的 .receipt-copy 挤成一条柱子。
    //   实测（老板报「左边一条、右边一个黑框」那天量的）：卡总宽 526px，
    //   `.receipt-copy` 只剩 **127px**、里面的 `raw <pre>` 变成 **127×320 的黑柱**，
    //   而 `.ab-out-preview` 占着右边 367px。
    //   修法（纯样式、不碰官方 DOM 结构）：允许换行，这两个块自己占满一整行。
    '.receipt{flex-wrap:wrap}',
    '.receipt-copy{flex:1 1 auto}',
    '.receipt .ab-out-preview,.receipt .ab-diff{flex:1 0 100%;min-width:0}',
    '.ab-out-preview{margin:8px 0 0;padding:8px 10px;border:1px solid var(--line);border-radius:var(--radius-control);background:var(--well-deep);color:var(--text-soft);font-family:ui-monospace,SFMono-Regular,Menlo,"Noto Sans Mono CJK SC",monospace;font-size:12px;line-height:1.5;white-space:pre-wrap;overflow-wrap:anywhere;max-height:13em;overflow-y:auto}',
    '.ab-out-preview[hidden]{display:none!important}',
    '.ab-out-line{white-space:pre-wrap}',
    '.ab-out-more{color:var(--text-faint);font-style:italic}',
    '.receipt:has(details[open]) .ab-out-preview{display:none!important}',
    // ── 文件改动的内联 diff（照官方 inline_diffs，默认 full，2026-09-19）──
    //   官方 Full = *"a bounded red/green unified diff"*，最多 14 行（app.mjs 的 MAX_INLINE_DIFF_LINES）
    '.ab-diff{margin:8px 0 0;padding:10px 12px;border:1px solid var(--line);border-radius:var(--radius-control);background:var(--well-deep);color:var(--text-soft);font-family:ui-monospace,SFMono-Regular,Menlo,"Noto Sans Mono CJK SC",monospace;font-size:12px;line-height:1.5;white-space:pre-wrap;overflow-x:auto}',
    '.ab-diff-line{white-space:pre-wrap}',
    '.ab-diff-add{color:var(--status-live)}',
    '.ab-diff-del{color:var(--status-danger)}',
    '.ab-diff-hunk{color:var(--action)}',
    '.ab-diff-meta{color:var(--text-faint)}',
    '.ab-diff-more{color:var(--text-faint);font-style:italic}',
    '.ab-diff-stat{color:var(--text-muted)}',
    // 文件改动显示的三个档（官方 inline_diffs: full / summary / off，默认 full）
    'html[data-ab-diffs="off"] .receipt .ab-diff{display:none!important}',
    'html[data-ab-diffs="summary"] .receipt .ab-diff-line:not(.ab-diff-stat){display:none!important}',
    // ── 思考卡「收起时的预览」（照官方 thinking_preview_lines，默认 2 行）──
    //   展开着的时候不重复显示预览（:has 不被支持时只是多显示两行，不影响功能）
    '.ab-think-preview{margin-top:6px;color:var(--text-faint);font-size:12.5px;line-height:1.5;white-space:pre-wrap;overflow-wrap:anywhere}',
    'article.reasoning:has(details[open]) .ab-think-preview{display:none!important}',
    // ── 思考背景填充（照官方 thinking_highlight，引擎里默认就是 true）──
    'html[data-ab-thinkbg="on"] article.reasoning pre{background:var(--surface-raised)}',
  ].join('\n');
  document.head.appendChild(stD);

  /* 「默认展开思考过程」只作用于**新出现的**思考卡。
   * ⚠️ 2026-09-22 修真 bug（档案 §8.7 201 第①条）：以前这里把**所有**未展开的 details 全打开，
   *   而它挂在 MutationObserver 上（任何一个 DOM 变化都会跑）⇒ 客户**手动收起**思考卡之后，
   *   只要来一条新消息 / 工具卡刷新一下，就被**again弹开** —— 客户根本收不起来，
   *   看起来就是「这个开关时灵时不灵」（老板说的「很多都实际无效」里就有它）。
   * 现在：进过 `reasoningSeen` 的节点一律不再动（客户收起的就一直是收起的）；
   *   设置值自己变化时清空一次（客户改设置 = 明确想要新状态，见 syncDisplayPref）。 */
  var reasoningSeen = (typeof WeakSet === 'function') ? new WeakSet() : null;
  function openReasoning() {
    if (!reasoningSeen) return;
    var list = document.querySelectorAll('article.reasoning details');
    for (var i = 0; i < list.length; i++) {
      var d = list[i];
      if (reasoningSeen.has(d)) continue;   // 见过的不再动（含客户手动收起的）
      reasoningSeen.add(d);
      d.open = true;
    }
  }
  /* 明细开关打开 → 回执的「查看回执」默认摊开。
   * 官方 tools=on 时是不截断、**完整显示**；web 上「完整」装在折叠块里，
   * 所以「开」＝把它摊开。两档都看得到摘要，开关只决定明细要不要动手点。 */
  /* 明细开关打开 → 回执的「查看回执」默认摊开；**明细关就不动它**。
   * ⚠️ 2026-09-19 修：以前这里**无条件**展开（174 轮定的 —— 那时正文只活在折叠块里，
   *   不展开就什么都看不到）。可现在卡上有了「默认可见的正文预览」（照官方明细关的 4 行），
   *   再一律展开就两头坏：预览被 CSS 让位规则顶掉（永远看不见），而客户在明细关档下看到的
   *   反而是**全文**（比官方多）。官方明细关的语义是「头部 ＋ 最多 4 行正文 ＋ 展开提示」——
   *   所以展开归 `show_tool_details` 管。 */
  function openReceiptDetails() {
    if (!DISPLAY.show_tool_details) return;
    // ⚠️ 安静模式（2026-09-22 让它**真生效** —— 档案 §8.7 201：以前它是个完全无效的开关，
    //   值写进了引擎与 prefs.json，网页端却**零消费方**）。
    //   官方语义查准了（`tui/history.rs:469-480` 的 match 顺序，别照注释猜）：
    //     · 明细**关**            → 卡截到 `TOOL_SUMMARY_CARD_LINES`=6 行（calm **不参与**，第一个分支先命中）
    //     · 明细**开** + calm 开  → 卡截到 `TOOL_CARD_SUMMARY_LINES`=8 行
    //     · 明细**开** + calm 关  → 完整
    //   ⇒ **calm 只在明细开着时才起作用**（这就是它在默认档下「看起来没用」的真正原因）。
    //   网页上「完整」＝把折块摊开，所以 calm 开时不摊开，正文改由卡上预览按 8 行预算露出
    //   （`app.mjs` 的 `calmPreviewActive()` 读 data-ab-calm 算行数）。
    if (DISPLAY.calm_mode) return;
    var list = document.querySelectorAll('article.receipt details:not([open])');
    for (var i = 0; i < list.length; i++) list[i].open = true;
  }
  // 回执的正文**一律展开**（长度由 CSS 按官方上限截）：
  //   官方「明细关」不是把正文收起来，而是「头部 + 最多 4 行内容 + 展开提示」——
  //   收起来的话客户什么都看不到（正是 2026-09-19 老板报到的那件事）。
  var reasonObserver = null;
  function applyDisplayPrefs() {
    var h = document.documentElement;
    h.setAttribute('data-ab-think', DISPLAY.show_thinking ? 'on' : 'off');
    h.setAttribute('data-ab-tools', DISPLAY.show_tool_details ? 'on' : 'off');
    h.setAttribute('data-ab-calm', DISPLAY.calm_mode ? 'on' : 'off');
    h.setAttribute('data-ab-thinkbg', DISPLAY.thinking_highlight ? 'on' : 'off');
    h.setAttribute('data-ab-diffs', DISPLAY.inline_diffs || 'full');
    h.setAttribute('data-ab-thinklines', String(DISPLAY.thinking_preview_lines));
    openReceiptDetails();
    if (DISPLAY.thinking_default_expanded) openReasoning();
    if (!reasonObserver && window.MutationObserver) {
      var pending = false;
      reasonObserver = new MutationObserver(function () {
        if (pending) return;
        pending = true;
        requestAnimationFrame(function () {
          pending = false;
          if (DISPLAY.thinking_default_expanded) openReasoning();
          openReceiptDetails();
        });
      });
      reasonObserver.observe(document.body, { childList: true, subtree: true });
    }
  }

  api('/v1/config').then(function (r) {
    if (!r.ok) return;                 // 引擎没起 / 读不到 → 用默认（显示思考），不打扰客户
    var c = r.body || {};
    // ⚠️ 2026-09-22 对齐官方默认（`settings.rs:538/564`）：`show_thinking` 官方默认 **false**、
    //   `calm_mode` 官方默认 **true**。所以「读不到」时的兵产要倒过来 ——
    //   以前写的是 `show_thinking !== false`（读不到当 true）＋ `calm_mode === true`（读不到当 false），
    //   恰好与官方相反。
    DISPLAY.show_thinking = c.show_thinking === true;
    DISPLAY.thinking_default_expanded = c.thinking_default_expanded === true;
    DISPLAY.show_tool_details = c.show_tool_details === true;
    DISPLAY.calm_mode = c.calm_mode !== false;
    // 引擎里默认就是 true（settings.rs:567）；读不到就保持默认 true，不倒挂
    DISPLAY.thinking_highlight = c.thinking_highlight !== false;
    // 文件改动显示：官方三档（默认 full）
    DISPLAY.inline_diffs = (c.inline_diffs === 'summary' || c.inline_diffs === 'off') ? c.inline_diffs : 'full';
    // 思考预览行数：引擎可能还没暴露这个键（不在 /v1/config 名单里）→ 退回官方默认 2
    DISPLAY.thinking_preview_lines = (typeof c.thinking_preview_lines === 'number') ? c.thinking_preview_lines : 2;
    DISPLAY.cost_currency = c.cost_currency === 'cny' ? 'cny' : 'usd';
    applyDisplayPrefs();
  });

  /* 「状态行显示」按人存（门卫 `/_gate/prefs` 的 `statusline_off`）—— 2026-09-22 改。
   * 读法：先认账号上那份；账号上没有而本机 localStorage 有 → **一次性迁上去**
   *   （老客户不用重设），迁完就没 localStorage 什么事了。 */
  function useStatuslineOff(list) {
    DISPLAY.statusline_off = Array.isArray(list) ? list : [];
    try { window.dispatchEvent(new CustomEvent('asbudy:statusline-change')); } catch (e) { /* 监听还没挂就算了 */ }
  }
  api('/_gate/prefs').then(function (r) {
    if (!r.ok) return;
    var raw = (r.body && r.body.prefs) ? r.body.prefs.statusline_off : null;
    if (typeof raw === 'string' && raw) {
      try {
        var arr = JSON.parse(raw);
        if (Array.isArray(arr)) { useStatuslineOff(arr); return; }
      } catch (e) { /* 坏值当没设 */ }
    }
    try {
      var old = JSON.parse(localStorage.getItem(STATUSLINE_OFF_KEY) || 'null');
      if (Array.isArray(old) && old.length) {
        useStatuslineOff(old);
        setStatuslineOff(old);        // 搬到账号上（以后换浏览器也跟着跑）
      }
    } catch (e) { /* 没 localStorage 就算了 */ }
  });

  /* ── 浮层 ── */
  // 「层」的记录：栈里每一项 = **怎么把这一层重新打开**。用来实现『← 返回』。
  // ⚠️ 为什么需要它（2026-09-21 老板报「子界面没有返回按钮」，实测确认）：这个浮层是
  //   **同一个框换内容** —— `openLayer` 每次都先 `closeLayer` 再重建，而且只有一个出口
  //   「关闭」＝**退出整个浮层**。于是从「我的」菜单点进「项目管理」之后菜单那一层已经被销毁，
  //   想再看另一个面板只能重新点「我的」、在 14 项里再找一遍（实测：菜单项 14 → 0）。
  //   现在：只要这一层**上面还有层**，标题左边就多一个「← 返回」，点了回到上一层。
  //   ⚠️ 24 个 openLayer 调用点**一个都不用改** —— 层级关系由栈自己推出来（不靠人记得传参）。
  var layerStack = [];

  function closeLayer() {
    var o = document.getElementById('asbudy-layer');
    if (o) o.remove();
    layerStack = [];                      // 浮层整个关掉 = 这条链全没了
  }

  function openLayer(title, build, opts) {
    var o = opts || {};
    var prev = layerStack.slice();         // closeLayer 会把栈清空，先留一份
    closeLayer();
    layerStack = prev.concat([{ title: title, build: build, opts: o }]);
    var hasParent = prev.length > 0 && !o.noBack;   // 上面还有层 → 给「← 返回」

    var L = document.createElement('div'); L.id = 'asbudy-layer';
    var box = document.createElement('div'); box.className = 'ab-box';
    var head = document.createElement('div'); head.className = 'ab-head';
    // 没有「返回」可给（栈底那一层）→ 打个记号：桌面窗口里的浮层会把标题栏整条收起来
    // （外壳由**桌面那个窗口**提供：标题 + ×。带进子面板就不一样了，见 styles.css 里那段注释）
    if (!hasParent) head.classList.add('ab-head-bare');
    if (hasParent) {
      var bk = document.createElement('button');
      bk.className = 'ab-back'; bk.type = 'button'; bk.textContent = '← 返回';
      bk.title = '回到上一层';
      bk.addEventListener('click', function () {
        var up = layerStack[layerStack.length - 2];      // 上一层
        // ⚠️ 要弹掉**两层**（自己 ＋ 上一层）—— 上一层马上会被重开、重新压回栈里。
        //    只弹一层的话，重开时它会把自己那条旧记录当成「父层」⇒ 菜单层（栈底）也会冒出返回按钮
        //    （2026-09-21 实测抓到）。
        layerStack = layerStack.slice(0, Math.max(0, layerStack.length - 2));
        if (up) openLayer(up.title, up.build, up.opts);  // 重开上一层（它还自己决定要不要返回）
      });
      head.appendChild(bk);
    }
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
  /* 会话对象缓存 —— 官方自己取回来的那份会话数据，留着给 bindPermissionChip **同步**纠错用。
   * 为什么要它：官方渲染「审批」标签是**同步**的，而我们的纠正原来是**异步** HTTP
   * ⇒ 打开会话的头几百毫秒显示错值（2026-09-22 老板报的就是这一瞬）。详见 bindPermissionChip。 */
  var THREAD_CACHE = {};
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
      // ★ 缓存「官方自己取回来的」单条会话对象（bindPermissionChip 靠它同步纠错）。
      //   只认 `GET /v1/threads/<id>` 这一种（带子路径的 /events、/turns 和列表 /summary 都不算）；
      //   用 clone 读副本，绝不消耗官方那份响应。
      try {
        if (p && p.then && u.indexOf('/v1/threads/summary') < 0
            && /\/v1\/threads\/[^\/?]+(\?|$)/.test(u)) {
          p.then(function (r) {
            if (!r || !r.ok || typeof r.clone !== 'function') return;
            var cp;
            try { cp = r.clone(); } catch (e) { return; }   // 已被读过就 clone 不了 —— 放过去
            cp.json().then(function (b) {
              var th = (b && (b.thread || b)) || null;
              if (th && th.id) THREAD_CACHE[th.id] = th;
            }).catch(function () {});
          }).catch(function () {});
        }
      } catch (e) {}
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
      + 'background:rgba(246,196,83,.15);border:1px solid var(--human);color:var(--text);font-size:13.5px;line-height:1.5;'
      + 'box-shadow:0 8px 28px rgba(0,0,0,.55)}'
      + '#asbudy-fixbar button{flex:none;font:inherit;font-size:13.5px;font-weight:600;color:var(--action-contrast);background:var(--live);'
      + 'border:0;border-radius:7px;padding:7px 13px;cursor:pointer}'
      + '#asbudy-fixbar button:hover{background:var(--live)}'
      + '#asbudy-fixbar button:disabled{opacity:.6;cursor:default}';
    document.head.appendChild(st);
    var bar = document.createElement('div');
    bar.id = 'asbudy-fixbar';
    bar.innerHTML = '<span>此对话出现异常（有一处记录未完成）。项目文件、代码与数据均未受影响。</span>'
      + '<button type="button" id="asbudy-fixbar-go">开启新对话继续</button>';
    document.body.appendChild(bar);
    var btn = bar.querySelector('#asbudy-fixbar-go');
    btn.onclick = function () {
      if (!MODEL_THREAD) { btn.textContent = '请先输入一条消息'; return; }
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
          alert('对话未受损 —— 可能是网络波动，请重试');
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
    if (!tid) { alert('请先选择一个会话'); return; }
    api('/v1/providers').then(function (r) {
      var all = (r.body && r.body.providers) || [];
      var ready = all.filter(function (p) { return p && p.id && p.credentialState === 'configured'; });
      var cur = (r.body && r.body.current) || '';
      if (!ready.length) {
        // 客户最容易在这里以为「坏了」：列表空 ≠ 系统不支持别家，只是**这个账号还没配别家的密钥**。
        // 所以提示要说清「去哪儿配」，而不是只报一个错（照 §8.7 198.2 ①）。
        alert('还没有配置任何模型服务。\n\n去「我的 → 高级设置 → 模型服务」填一个服务商的密钥，这里就会出现它。');
        return;
      }
      api('/v1/threads/' + encodeURIComponent(tid)).then(function (tr) {
        var body = tr.body || {};
        var th = body.thread || body;          // ⚠️ 详情接口把 thread 包在 .thread 里（列表才是裸数组）
        var thProvider = String(th.model_provider_id || th.model_provider || '');
        var thModel = String(th.model || '');
        openLayer('换个模型', function (body) {
          body.innerHTML = '<div class="ab-tip" id="mp-tip">所选模型将应用于当前对话。<br>' +
            '只列出<b>已配置密钥</b>的服务商；要接入其他服务商，去「我的 → 高级设置 → 模型服务」。</div>' +
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
              var head = '<div style="color:var(--text-dim);font-size:13px;margin:12px 0 6px">' +
                esc(g.p.display_name || g.p.id) + (g.p.id === cur ? '（默认）' : '') + '</div>';
              if (!g.models.length) return head + '<div class="ab-tip">这个提供商没有模型目录</div>';
              return head + g.models.map(function (m) {
                var on = m.id === thModel;
                var sub = [];
                // 别家：现在**能点**了（点一下用这家新开一条）—— 不再只说一句「需新建会话」
                // 让客户自己去找入口（§8.7 198.2 ②）。
                if (!same) sub.push('点一下 = 用这家新开一条');
                if (m.image_input === 'supported') sub.push('可看图');
                return '<button class="ab-menu-item" data-m="' + esc(m.id) + '" data-p="' + esc(g.p.id) + '"' +
                  (same ? '' : ' style="opacity:.78"') + '>' + esc(m.id) + (on ? '（当前）' : '') +
                  '<small style="' + (on ? 'color:var(--action)' : '') + '">' + esc(sub.join(' · ')) + '</small></button>';
              }).join('');
            }).join('');
            list.querySelectorAll('button[data-m]').forEach(function (b) {
              b.onclick = function () {
                var mid = b.getAttribute('data-m');
                var pid = b.getAttribute('data-p');
                if (thProvider && pid !== thProvider) {
                  // 别家服务商：不能就地换，但可以**用这家新开一条**（官方那个浮层走的就是这一步）
                  var g2 = null, m2 = null;
                  for (var gi = 0; gi < groups.length; gi++) {
                    if (groups[gi].p.id !== pid) continue;
                    g2 = groups[gi];
                    for (var mi = 0; mi < g2.models.length; mi++) if (g2.models[mi].id === mid) m2 = g2.models[mi];
                  }
                  if (g2) offerNewThreadOnVendor(g2, m2 || { id: mid }, msgEl, thProvider);
                  return;
                }
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
  /* ── 「用别的服务商新开一条」────────────────────────────────────────────
   * 【为什么不能就地换】引擎在**建会话那一刻**就把「用哪家」写进了会话记录
   *   （`provider_identity_for_thread`：会话记住自己的路线）；`PATCH /v1/threads/{id}`
   *   只认 `model`、**不收 provider**（2026-09-23 实测：跨厂商模型名 PATCH 返 200
   *   不校验、发消息才 400）。官方对这件事的官方说法就写在它自己的新建浮层里：
   *   「此选择仅对当前新会话生效，不会改变运行时的默认设置」——**换家 = 新开一条**。
   * 【这里做什么】把官方那个浮层走的那一步搬到客户已经点到的这个地方：
   *   `buildCreateThreadRequest` 的三个字段（model_provider / model_provider_id / model）
   *   → `POST /v1/threads`。门卫照常补项目提示与工作区（`newThreadInProject`）
   *   ⇒ 建出来的新会话跟官方建的**一模一样**。
   * 【为什么必须先说一句】当前这段对话的内容**不会**带过去 —— 不能让客户点完
   *   才发现「刚才聊的没了」。
   */
  function offerNewThreadOnVendor(group, model, msgEl, fromProvider) {
    var host = msgEl.parentNode;
    var old = host.querySelector('#mp-confirm');
    if (old) old.remove();
    var vendor = esc(group.p.display_name || group.p.id);
    var d = document.createElement('div');
    d.id = 'mp-confirm';
    d.className = 'ab-tip';
    d.style.cssText = 'margin-top:10px;padding:10px;border:1px solid var(--line);border-radius:8px';
    d.innerHTML = '这段对话在 <b>' + esc(fromProvider || '另一家') + '</b> 上，' +
      '<b>' + esc(model.id) + '</b> 属于 <b>' + vendor + '</b>。<br>' +
      '换服务商要<b>新开一条对话</b> —— 这一段聊的内容不会带过去（AI 仍然知道你在哪个项目）。' +
      '<div style="margin-top:9px;display:flex;gap:8px">' +
      '<button class="ab-btn sm" id="mp-go">用 ' + vendor + ' 新开一条</button>' +
      '<button class="ab-btn sm ghost" id="mp-no">取消</button></div>';
    host.appendChild(d);
    d.querySelector('#mp-no').onclick = function () { d.remove(); };
    d.querySelector('#mp-go').onclick = function () {
      var go = d.querySelector('#mp-go');
      go.disabled = true;
      go.textContent = '正在新建…';
      // 字段照官方 `buildCreateThreadRequest` —— provider 的 exact id 有就带上（自定义服务商靠它）
      var req = { model_provider: String(group.p.id), model: String(model.id) };
      var exact = String(group.p.model_provider_id || '');
      if (exact) req.model_provider_id = exact;
      api('/v1/threads', { method: 'POST', body: JSON.stringify(req) }).then(function (r) {
        if (!r.ok) {
          d.remove();
          msg(msgEl, '没建成：' + ((r.body && r.body.error) || '未知原因'), false);
          return;
        }
        msg(msgEl, '已用 ' + vendor + ' 新建一条对话，正在打开…', true);
        // 刷新后官方会选中**最新**那条（`selectThread(app.summaries[0].id)`）⇒ 正好落在新建上
        setTimeout(function () { location.reload(); }, 900);
      });
    };
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
    var box = chip.closest('.fact-chip');
    if (box && !box.getAttribute('data-asbudy-prov')) {
      box.setAttribute('data-asbudy-prov', '1');
      box.title = '模型商 —— 点这里配置';
      box.style.cursor = 'pointer';
      box.addEventListener('click', openModelApiLoader);
    }
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
  /* ── 会话详情那排小标签（2026-09-17 老板）──────────────────────────────
     老板两条意见：① 英文标签要中文（Branch / Provider / Permission）
                   ② 除了「模型」，模型商 / 模式 / 审批也应该能点着改。
     顺手修一个真 bug（档案 §8.7 110）：官方 `permissionLabel` 只看 `auto_approve` 布尔，
     而门卫给「小的自己做」（auto 档）设的是 `auto_approve=false` + `permission_posture=auto_review`
     → 官方判成「每次询问」。**两档都会错**：
       posture=auto_review → 官方显示「每次询问」（应该是「自动审核」，实测见 §8.7 110）
       posture=full_access → 官方显示「自动审核」（应该是「完全访问」）
     所以这里**按 permission_posture 重写显示**，并且点它就能改。 */
  var FACT_LABEL = { branch: '分支', provider: '模型商', permission: '审批' };
  // 客户不需要看的技术标签（键就是官方给的 data-fact）：
  //   '工作区' —— 官方塞的是工作目录 basename，也就是**项目 key**（如 s9l8fqm），对客户就是乱码；
  //   branch   —— git 分支，客户既不需要知道也不会去切。
  // 两个都是**纯展示、没有点击**（实测确认），藏掉不丢功能。
  // 判据还是 DIRECTION 第 6 条：客户在界面上看不看得见技术概念。
  // （2026-09-17 老板转来用户的反馈：「看不懂」。）
  var FACT_HIDE = { '工作区': 1, 'branch': 1 };
      // 三档措辞跟「高级设置 → 审批方式」下拉里的**完全同名** —— 两处说同一件事就必须用同一套词
      //（2026-09-17 老板：「到底是什么关系？两处显示得能不能对应起来？」原来一处「每步都先问我」
      // 一处「每次询问」，看着就是两个东西。改掉。）
  // ★ 三档措辞一律用**官方语言包**（`crates/localization/locales/zh-Hans.json` 的
  //   `ConfigChoiceAsk` / `ConfigChoiceAutoReview` / `ConfigChoiceFullAccess`）——
  //   2026-09-19 老板：「查到官方就照搬」。以前自造的「小的自己做 / 全部自己做」已废弃。
  //   ⚠️ 「自动审核」的说明按**实测**写（官方那句「需要你决定时才询问」跟实现不符：
  //     源码两处写明 Auto-Review never opens an approval modal；实测 135 次审批里
  //     91 次在 1 秒内被引擎自己批掉 —— 老板 2026-09-19 拍「按实测写」）。
  var POSTURE_TEXT = { ask: '询问', auto_review: '自动审核', full_access: '完全访问' };
  // 三档 ↔ 引擎两个字段（跟门卫 syncThreadApproval 同一套映射 —— 改一边必须改另一边）
  /* ── 「状态行显示哪些段」（2026-09-20 搬 · 照官方 CLI 的 `/statusline` 多选选择器）──
   * ⚠️ **必须放顶层**（跟 POSTURE_TEXT 这一档同级）：初版把它们写在会话指标那个嵌套作用域里，
   *   结果 `openAdvanced`（顶层）看不到 `statuslineSummary` —— 高级设置面板直接报
   *   `statuslineSummary is not defined`、永远停在「加载中…」（实测踩到）。
   * 官方那一项存 **`settings.toml`**（`commands/groups/config/config.rs:550`
   *   → `AppAction::OpenStatusPicker`，实现在 `ui/apply.rs:2135`）；
   * 而引擎的 `POST /v1/config` **白名单里没有状态行键** ⇒ 网页端一开始只能落 localStorage。
   * ⚠️ **2026-09-22 改**（档案 §8.7 201 第④条：状态行只存本机、换浏览器要重设）：
   *   现在跟别的显示偏好一样**存账号** —— 门卫 `PREFS_KEYS` 里的 `statusline_off`
   *   （`/_gate/prefs`，值是 JSON 字符串），换浏览器 / 换机器都跟着跑。
   *   `STATUSLINE_OFF_KEY` 只用于**首次迁移**老客户本机存的那份（迁完就自然不用了）。
   *   （**别去写 settings.toml** —— 会撞 EACCES，见 server.js 里那条教训。）
   * 存的是「**关掉**的段」（默认全开 → 新搬的段自动出现，不用改存量设置）。 */
  var STATUSLINE_OFF_KEY = 'ab-statusline-off';
  var STATUSLINE_SEGS = [
    { k: 'ttft', label: '首字延迟（ttft）' },
    { k: 'rate', label: '输出速度（平均 tok/s）' },
    { k: 'output', label: '本轮输出量（↓ tokens）' },
    { k: 'cache', label: '缓存命中率（cache）' },
  ];
  function statuslineOff() {
    var v = DISPLAY.statusline_off;
    return Array.isArray(v) ? v.slice() : [];
  }
  /** 存到**账号**上（换浏览器也跟着跑）——返回是否存成，调用方据此报错、不骗人。 */
  function setStatuslineOff(list) {
    DISPLAY.statusline_off = (list || []).slice();
    return api('/_gate/prefs', {
      method: 'POST',
      body: JSON.stringify({ prefs: { statusline_off: JSON.stringify(list || []) } }),
    }).then(function (r) { return !!(r.ok && !(r.body && r.body.ok === false)); })
      .catch(function () { return false; });
  }
  /** 「状态行显示」那一行的值文字 —— 全开时写「全部」，否则把关掉的列出来 */
  function statuslineSummary() {
    var off = statuslineOff();
    if (!off.length) return '全部 ' + STATUSLINE_SEGS.length + ' 项';
    var names = STATUSLINE_SEGS.filter(function (s) { return off.indexOf(s.k) >= 0; })
      .map(function (s) { return s.label.replace(/（.*）$/, ''); });
    return '已关：' + names.join('、');
  }

  var APPROVAL_OPTS = [
    { m: 'suggest', p: 'ask', a: false, t: '询问', d: '在可能造成重大更改的工具运行前询问（在项目文件夹里写文件不询问）' },
    { m: 'auto', p: 'auto_review', a: false, t: '自动审核', d: '不问你；它自己判断，不行的直接拦下（默认）' },
    { m: 'bypass', p: 'full_access', a: true, t: '完全访问', d: '无需审批提示即可运行工具' },
  ];
  /* 推理级别（思考强度）—— 2026-09-20 搬表 M6（官方 CLI `/effort`，别名 `/thinking`；`Ctrl+T`）
   * ⚠️ **取值只照官方「DeepSeek 路由」那一档**（`tui/model_picker.rs:63` 的 `DEEPSEEK_PICKER_EFFORTS`
   *   ＝ `Auto, Off, Low, High, Max`）—— **不是**通用那套（通用还有 minimal/medium/xhigh/ultra）。
   *   官方注释（`reasoning_preference.rs:224-235`）：DeepSeek 线上文档只认 low/high/max，
   *   `medium` 会被**上取整成 high** ⇒ 官方在 DeepSeek 上就只摆这 5 档，我们跟着摆 5 档。
   *   键名与值串都照官方（`reasoning_effort` / `as_setting()` 的规范串）。
   *   官方 web **没有**这一项（`runtime_web/app.mjs` 对 reasoning_effort 0 命中）—— 这是补空白。
   *   「未设置」在引擎里读出来就是 `auto`（`runtime_api.rs:7343` 的 `unwrap_or("auto")`）。 */
  var EFFORT_OPTS = [
    { v: 'auto', t: '自动（默认）' },
    { v: 'off', t: '关闭 —— 最快' },
    { v: 'low', t: '低 —— 更快' },
    { v: 'high', t: '高 —— 更仔细' },
    { v: 'max', t: '最高 —— 想得最久' },
  ];
  /* 「AI 回复语言」的可选值 —— **官方语言包全集**（`crates/localization/locales/*.json`，共 15 个）。
   * ⚠️ 2026-09-22 补（档案 §8.7 201 第③条：以前只摆了 auto/简体中文/English 三档）。
   *   名字用**该语言自己的写法**（照官方语言包的习惯），不译成中文。
   * ⚠️ 真实效果分两档：引擎自带的语言强化（`prompts.rs:758/784`）**只覆盖
   *   zh-Hans / ja / pt-BR / vi**；简体中文与 English 则由我们门卫在 system_prompt 里
   *   逐字点名（见 server.js 的 `langReinforcement`）。其余语言靠模型自己 —— 界面上的小字说明了这一点。 */
  var LOCALE_OPTS = [
    { v: 'auto', t: '跟随系统（简体中文）' },
    { v: 'zh-Hans', t: '简体中文' },
    { v: 'zh-Hant', t: '繁體中文' },
    { v: 'en', t: 'English' },
    { v: 'ja', t: '日本語' },
    { v: 'ko', t: '한국어' },
    { v: 'de', t: 'Deutsch' },
    { v: 'fr', t: 'Français' },
    { v: 'es-419', t: 'Español (Latinoamérica)' },
    { v: 'pt-BR', t: 'Português (Brasil)' },
    { v: 'ru', t: 'Русский' },
    { v: 'uk', t: 'Українська' },
    { v: 'vi', t: 'Tiếng Việt' },
    { v: 'id', t: 'Bahasa Indonesia' },
    { v: 'hi', t: 'हिन्दी' },
    { v: 'ca', t: 'Català' },
  ];
  /** 审批档的中文名 —— **一份定义两处用**（对话上方的标签、我的 → 高级设置那一行） */
  function approvalTextOf(mode) {
    var o = APPROVAL_OPTS.filter(function (x) { return x.m === mode; })[0];
    return o ? o.t : '自动审核';
  }
  function factChipEl(key) { return document.querySelector('#session-facts .fact-chip[data-fact="' + key + '"]'); }
  function localizeFacts() {
    var chips = document.querySelectorAll('#session-facts .fact-chip');
    for (var i = 0; i < chips.length; i++) {
      var key = chips[i].getAttribute('data-fact') || '';
      // 先中文化、再（按需）隐藏 —— 顺序别反：跳着走会在 DOM 里留下「Branch」这种英文
      //（测试里那条反证「页面上没有 Branch」就是抓这个的）。
      var want = FACT_LABEL[key];
      var lab = chips[i].querySelector('span');
      if (want && lab && lab.textContent !== want) lab.textContent = want;
      if (FACT_HIDE[key]) chips[i].style.display = 'none';
    }
  }
  /* 同一条会话「在飞的请求」只留一个 —— MutationObserver 会因 characterData 频繁触发，
   * 不去重就会对引擎发一堆重复请求（这条会话正在跑长任务时尤其明显）。 */
  var _thInflight = null, _thInflightId = '';
  function currentThread() {
    if (!MODEL_THREAD) return Promise.resolve(null);
    var want = MODEL_THREAD;
    if (_thInflight && _thInflightId === want) return _thInflight;
    var pr = api('/v1/threads/' + encodeURIComponent(want)).then(function (r) {
      var th = (r.body && (r.body.thread || r.body)) || null;
      if (th && th.id) THREAD_CACHE[th.id] = th;   // 顺手补缓存
      return th;
    }).catch(function () { return null; });
    _thInflight = pr; _thInflightId = want;
    pr.then(function () { if (_thInflight === pr) { _thInflight = null; _thInflightId = ''; } });
    return pr;
  }
  /** 按 `permission_posture` 算出该显示的三档文案。
   *  官方 `app.mjs` 的 `permissionLabel` **只看 `auto_approve` 布尔**，所以两档都会错：
   *    · auto_review + auto_approve=false → 官方显示「每次询问」（应为「自动审核」）
   *    · full_access + auto_approve=true  → 官方显示「自动审核」（应为「完全访问」）
   *  这里一律按 posture 判，跟高级设置下拉、跟门卫 syncThreadApproval 同一套映射。 */
  function postureTextOf(th) {
    if (!th) return '';
    var p = String(th.permission_posture || '');
    return POSTURE_TEXT[p] || (th.trust_mode ? '完全访问' : (th.auto_approve ? '自动审核' : '询问'));
  }
  function paintFact(key, text) {
    var s = document.querySelector('#session-facts .fact-chip[data-fact="' + key + '"] strong');
    if (s) s.textContent = text;
  }
  function bindPermissionChip() {
    var c = factChipEl('permission');
    if (!c) return;
    if (!c.getAttribute('data-asbudy-perm')) {
      c.setAttribute('data-asbudy-perm', '1');
      c.title = '审批方式 —— 点这里改';
      c.style.cursor = 'pointer';
      c.addEventListener('click', function () { openApprovalPicker(); });
    }
    var strong = c.querySelector('strong');
    if (!strong) return;
    // ① **同步**纠正 —— 官方刚取回来的那份会话数据就在手边，跟它同一拍写，不留错值窗口。
    //    （原来只有下面那条异步路：客户点开会话，官方先写上「每次询问」，我们的补丁要飞一趟
    //      HTTP 才改成「自动审核」—— 中间那几百毫秒客户看到的就是错值。2026-09-22 实测 0.47s；
    //      引擎在跑长任务时更久，老板正是在那条 285 秒的会话上看到的。）
    //
    // ★ 2026-09-23·§8.7 205 的**确定性**修法：缓存里还没有值时，**先不显示**，而不是先显示官方猜的。
    //   为什么单靠同步纠正不够：① 缓存是 `cp.json()` **异步**填的（copy 也要等一个 IO），
    //   而官方渲染标签是同步的 —— 谁先到不确定；② 刚打开页面时缓存必然是空的。
    //   ⇒ 未命中缓存时把标签文本**清空**（客户看到的是「稍晚出现」，不是「错的档位」）。
    //   ⚠️ 不能改成隐藏或者写占位符：采样器看的是 `textContent`，隐藏照样能采到，
    //      占位符（如「…」）会被当成错值；只有**空文本**是「还没值」的诚实表达。
    var sync = MODEL_THREAD ? postureTextOf(THREAD_CACHE[MODEL_THREAD]) : '';
    if (sync) {
      if (strong.textContent !== sync) strong.textContent = sync;
      c.setAttribute('data-asbudy-perm-known', '1');
    } else if (!c.getAttribute('data-asbudy-perm-known')) {
      if (!c.hasAttribute('data-asbudy-perm-raw')) {
        c.setAttribute('data-asbudy-perm-raw', strong.textContent || '');   // 先存着官方的值（兼底用）
      }
      if (strong.textContent) strong.textContent = '';
      // 兼底：真拿不到时（引擎挂了 / 没会话）不能让它永远空着 —— 3 秒后退回官方那个值。
      if (!c.getAttribute('data-asbudy-perm-t')) {
        c.setAttribute('data-asbudy-perm-t', '1');
        setTimeout(function () {
          if (c.getAttribute('data-asbudy-perm-known')) return;
          var s2 = c.querySelector('strong');
          if (s2 && !s2.textContent) s2.textContent = c.getAttribute('data-asbudy-perm-raw') || '';
        }, 3000);
      }
    }
    // ② 异步兜底：缓存还没有（刚打开页面 / 官方还没取过这条）时补一次；拿到就标记为「已知」。
    currentThread().then(function (th) {
      var text = postureTextOf(th);
      if (!text) return;
      c.setAttribute('data-asbudy-perm-known', '1');
      if (strong.textContent !== text) strong.textContent = text;
    });
  }
  /** 把审批档当场写进当前会话（高级设置改完要用；跟门卫 syncThreadApproval 同一套映射）。 */
  function applyApprovalToThread(mode) {
    if (!MODEL_THREAD) return;
    var o = APPROVAL_OPTS.filter(function (x) { return x.m === mode; })[0];
    if (!o) return;
    api('/v1/threads/' + encodeURIComponent(MODEL_THREAD), {
      method: 'PATCH', body: JSON.stringify({ auto_approve: o.a, permission_posture: o.p }),
    }).then(function () { paintFact('permission', o.t); }).catch(function () { /* 改不动就算了，发消息前门卫还会再拉一次 */ });
  }
  /** 改审批档 —— ★ **一处实现**（2026-09-19 合并入口）：
   *  ① 写账号偏好（以后新建的会话/项目都按这个）② 写当前这条会话（立刻生效、标签跟着对）。
   *  两个入口（对话上方那排小标签、我的 → 高级设置那一行）都调它 —— 不再各写一套。 */
  function applyApprovalChoice(o) {
    if (!o) return Promise.reject(new Error('没有这一档'));
    var jobs = [api('/_gate/prefs', { method: 'POST', body: JSON.stringify({ prefs: { approval_mode: o.m } }) })];
    if (MODEL_THREAD) jobs.push(api('/v1/threads/' + encodeURIComponent(MODEL_THREAD), {
      method: 'PATCH', body: JSON.stringify({ auto_approve: o.a, permission_posture: o.p }),
    }));
    return Promise.all(jobs);
  }
  /** 审批方式选择弹层 —— 对话上方的标签、高级设置那一行**共用这一个**。
   *  onDone(o)：改成功后回调（高级设置用它就地刷新那一行的文字）。 */
  function openApprovalPicker(onDone) {
    openLayer('审批方式', function (body) {
      body.innerHTML =
        '<div class="ab-tip" style="margin:0 0 12px">AI 执行操作前是否先问你。改完当前会话立刻生效，以后的项目也按这个来。</div>' +
        '<div id="ap-list"><div class="ab-tip">正在读…</div></div><div class="ab-msg" id="ap-msg"></div>';
      var list = body.querySelector('#ap-list'), msgEl = body.querySelector('#ap-msg');
      currentThread().then(function (th) {
        var cur = th ? String(th.permission_posture || '') : '';
        list.innerHTML = APPROVAL_OPTS.map(function (o) {
          var on = cur === o.p;
          return '<button class="ab-menu-item" data-i="' + o.m + '"' + (on ? ' style="border-color:var(--action)"' : '') + '>' +
            o.t + (on ? '（当前）' : '') + '<small>' + o.d + '</small></button>';
        }).join('');
        list.querySelectorAll('button[data-i]').forEach(function (b) {
          b.onclick = function () {
            var o = APPROVAL_OPTS.filter(function (x) { return x.m === b.getAttribute('data-i'); })[0];
            if (!o) return;
            b.disabled = true;
            // 两边一起写：① 这条会话（立刻生效、标签跟着对）② 账号偏好（以后的会话和项目）
            applyApprovalChoice(o)
              .then(function (rs) {
                b.disabled = false;
                if (rs[0] && !rs[0].ok) { msg(msgEl, (rs[0].body && rs[0].body.error) || '没存下来', false); return; }
                paintFact('permission', o.t);
                if (typeof onDone === 'function') { try { onDone(o); } catch (e) { /* 回调出错不影响已保存 */ } }
                msg(msgEl, '已改为「' + o.t + '」', true);
                setTimeout(closeLayer, 900);
              })
              .catch(function (e) { b.disabled = false; msg(msgEl, '没改成功：' + ((e && e.message) || e), false); });
          };
        });
      });
    });
  }
  var MODE_TEXT = { agent: '工作', plan: '计划', operate: '运维' };   // operate 只留给旧会话显示（不再给选项）
  // 2026-09-19 老板「运维模式可以去掉」：只给客户「工作 / 计划」两种。
  //   「运维」是 fleet 编排那套（老板已拍不搬）—— 选项里摆着它只会让人多问一句。
  //   「计划」是真管用的：引擎里 `AppMode::Plan => allow_shell:false` + `ShellPolicy::None`，
  //   AI 只能看、只能说，**动不了客户的项目**。
  var MODE_OPTS = [
    { v: 'agent', t: '工作', d: '直接改文件、跑命令' },
    { v: 'plan', t: '计划', d: '先出方案，先不动手' },
  ];
  function bindModeChip() {
    var c = factChipEl('模式');
    if (!c || c.getAttribute('data-asbudy-mode')) return;
    c.setAttribute('data-asbudy-mode', '1');
    c.title = '模式 —— 点这里改';
    c.style.cursor = 'pointer';
    c.addEventListener('click', openModePicker);
  }
  function openModePicker() {
    openLayer('模式', function (body) {
      body.innerHTML = '<div class="ab-tip" style="margin:0 0 12px">决定它在这次会话里的工作方式。</div>' +
        '<div id="md-list"><div class="ab-tip">正在读…</div></div><div class="ab-msg" id="md-msg"></div>';
      var list = body.querySelector('#md-list'), msgEl = body.querySelector('#md-msg');
      currentThread().then(function (th) {
        var cur = th ? String(th.mode || '') : '';
        list.innerHTML = MODE_OPTS.map(function (o) {
          var on = cur === o.v;
          return '<button class="ab-menu-item" data-v="' + o.v + '"' + (on ? ' style="border-color:var(--action)"' : '') + '>' +
            o.t + (on ? '（当前）' : '') + '<small>' + o.d + '</small></button>';
        }).join('');
        list.querySelectorAll('button[data-v]').forEach(function (b) {
          b.onclick = function () {
            var v = b.getAttribute('data-v');
            if (!MODEL_THREAD) { msg(msgEl, '先选一个会话', false); return; }
            b.disabled = true;
            // 走门卫而不是直接打引擎（2026-09-19）：门卫会校验「这条对话属于你能用的项目」，
            // 并把这个人的选择**记下来** → 下次新建对话沿用同一个模式。
            fetch('/_gate/thread-mode', {
              method: 'POST', credentials: 'same-origin',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({ thread: MODEL_THREAD, mode: v }),
            })
              .then(function (r) {
                return r.json().catch(function () { return {}; })
                  .then(function (j) { return { ok: r.ok, body: j }; });
              })
              .then(function (r) {
                b.disabled = false;
                if (!r.ok) { msg(msgEl, (r.body && r.body.error) || '没改成', false); return; }
                paintFact('模式', MODE_TEXT[v] || v);
                msg(msgEl, '已切到「' + (MODE_TEXT[v] || v) + '」', true);
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
      // ⚠️ 按 data-fact 认，不按文字认 —— 「模型商」也以「模型」开头，按文字认会把它认成模型那个
      if (c.getAttribute('data-fact') !== '模型') continue;
      c.setAttribute('data-asbudy-model', '1');
      c.title = '切换模型';
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
      localizeFacts();
      bindPermissionChip();
      bindModeChip();
      new MutationObserver(function () {
        bindModelChip(); bindProviderChip(); localizeFacts(); bindPermissionChip(); bindModeChip();
        // characterData：官方若**只改文本不换节点**（我们只盯 childList 时收不到），
        // 那条错值就永远留在页面上 —— 2026-09-22 补上，兜住这一类。
      }).observe(facts, { childList: true, subtree: true, characterData: true });
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
        var html = '<div class="ab-tip">平台协助排查问题时，需以你的视角进入（看到的就是你当前的界面）。<br>' +
          '<b>仅你可开启此开关，管理员无法代开</b>；可随时关闭。</div>';
        html += '<div style="margin:12px 0 14px;padding:10px 12px;border:1px solid ' + (on ? 'var(--live)' : 'var(--line)') +
          ';border-radius:8px">当前状态：<b style="color:' + (on ? 'var(--live)' : 'var(--text-dim)') + '">' +
          (on ? '已开启' : '未开启') + '</b>' +
          (on && st.until ? '<div style="color:var(--text-dim);font-size:13.5px;margin-top:4px">有效期到 ' +
            esc(new Date(st.until).toLocaleString()) + '</div>' : '') + '</div>';
        if (on) {
          html += '<button class="ab-menu-item" id="ab-c-off">关闭平台协助<small>关闭后平台将无法进入</small></button>';
        } else {
          html += '<button class="ab-menu-item" id="ab-c-on24">开启 24 小时<small>适合单次问题排查</small></button>' +
                  '<button class="ab-menu-item" id="ab-c-on168">开启 7 天<small>长期协助（可随时关闭）</small></button>';
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
        html += '<button class="ab-menu-item" id="ab-m-staff">员工管理<small>添加员工、分配项目、设置额度</small></button>';
      }
      if (role === 'admin') {
        html += '<button class="ab-menu-item" id="ab-m-users">客户管理<small>添加客户、转移项目归属</small></button>';
        html += '<button class="ab-menu-item" id="ab-m-tokens">用量明细<small>每个账号用了多少 token、值多少钱</small></button>';
      }
      html += '<button class="ab-menu-item" id="ab-m-proj">项目管理<small>查看和删除项目</small></button>';
      // 项目文件（2026-09-19 老板：「留「我的资料」，「项目文件」收到「我的」里」）——
      //   手机上侧栏地方不够（实测树区只有 102px），而树能长到几百项；弹层里地方大、还能搜。
      //   桌面侧栏照旧保留（地方够、鼠标操作树更顺）。
      html += '<button class="ab-menu-item" id="ab-m-files">项目文件<small>看当前项目里的文件，也能按名字找</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-mem">AI 的记忆<small>查看和清除 AI 记住的内容</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-skills">它会做什么<small>内置技能：做 PPT / 表格 / 文档 / PDF / 图表…</small></button>';
      // 回收站 / 退回（2026-09-19 老板：「手机端左侧栏塞了太多太多东西，很多完全可以收纳进「我的」里」）——
      //   这两个是**兜底功能**（偶尔来看一眼），没必要天天占侧栏的地方。手机侧栏已经不再摆它们，
      //   桌面侧栏照旧保留（地方够，多一个显眼入口比多一次点击好）。两边打开的是**同一份面板**。
      // ⚠️ 面板函数在 asbudy-files.js 里（另一个 IIFE）—— 只能走 window.__asbudyPanels。
      html += '<button class="ab-menu-item" id="ab-m-recycle">回收站<small>删掉的项目与文件，可以还原</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-undo">退回<small>回到某个时间点（此后的改动会撤销）</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-space">空间<small>存储用量与配额</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-adv">高级设置<small>代码仓库 / 模型服务 / 只读模式 / 用量统计</small></button>';
      if (role === 'customer') {
        html += '<button class="ab-menu-item" id="ab-m-consent">平台协助<small>允许平台协助排查问题（仅你可开启，可随时关闭）</small></button>';
      }
      html += '<button class="ab-menu-item" id="ab-m-account">我的账号<small>姓名、登录账号与归属</small></button>';
      html += '<button class="ab-menu-item" id="ab-m-pw">修改密码</button>';
      html += '<button class="ab-menu-item" id="ab-m-logout">退出登录</button>';
      if (installEvt) html += '<button class="ab-menu-item" id="ab-m-install">装到桌面<small>安装为桌面应用</small></button>';
      body.innerHTML = html;
      abTipPanel(body, 'my-menu', '此处的设置会应用到<b>你的所有项目</b>（含以后新建的）。');
      var bStaff = body.querySelector('#ab-m-staff');
      if (bStaff) bStaff.onclick = function () { openStaff(role === 'admin' ? 'admin' : ME.user); };
      var bUsers = body.querySelector('#ab-m-users');
      if (bUsers) bUsers.onclick = openUsers;
      var bTokens = body.querySelector('#ab-m-tokens');
      if (bTokens) bTokens.onclick = openTokenUsage;
      body.querySelector('#ab-m-proj').onclick = openProjects;
      body.querySelector('#ab-m-files').onclick = function () { openSidePanel('项目文件', 'proj'); };
      body.querySelector('#ab-m-mem').onclick = openMemory;
      body.querySelector('#ab-m-skills').onclick = openSkills;
      body.querySelector('#ab-m-recycle').onclick = function () { openSidePanel('回收站', 'recycle'); };
      body.querySelector('#ab-m-undo').onclick = function () { openSidePanel('退回', 'undo'); };
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
    });
  }

  /** 把侧栏那些自造面板（回收站 / 退回）搬进弹层打开（2026-09-19）。
   *  面板逻辑本身在 asbudy-files.js，通过 window.__asbudyPanels 拿到 —— 只是把**渲染目标**
   *  换成弹层里这个容器，所以侧栏版与弹层版永远是一份代码。 */
  function openSidePanel(title, which) {
    openLayer(title, function (body) {
      var host = document.createElement('div');
      host.className = 'ab-panel-host';
      body.appendChild(host);
      var fn = (window.__asbudyPanels || {})[which];
      if (!fn) { host.innerHTML = '<div class="ab-tip">这个面板还没就绪，刷新一下页面试试。</div>'; return; }
      fn(host);
    });
  }

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
        ? '添加员工账号、设置项目额度与可见项目。<br>查看客户员工请前往「客户管理」。'
        : '添加员工账号、设置项目额度与可见项目。<br>员工创建的项目归<b>你名下</b>（可见可管）；删除员工时项目转回你名下，<b>项目不会被删除</b>。';
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
            if (!staff.length) { listEl.innerHTML = '<div class="ab-tip">暂无员工。</div>'; return; }
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
                ' ｜ 空间配额：' + (s.quotaMb ? esc(Math.round(s.quotaMb / 1024 * 10) / 10) + ' G' : '不限') +
                (s.projects && s.projects.length ? ' ｜ 已自建：' + esc(s.projects.join('、')) : '') + '<br>' +
                '可看项目：' + (granted.length ? esc(granted.join('、')) : '<span style="color:var(--human)">未分配</span>') + '</div></div></div>';
              var acts = document.createElement('div');
              acts.style.cssText = 'display:flex;gap:7px;margin-top:10px;flex-wrap:wrap';
              var bEdit = document.createElement('button'); bEdit.className = 'ab-btn ghost sm'; bEdit.type = 'button'; bEdit.textContent = '编辑';
              bEdit.onclick = function () { openStaffForm(s, owner, projects, staff, reopen); };
              var bDel = document.createElement('button'); bDel.className = 'ab-btn danger sm'; bDel.type = 'button'; bDel.textContent = '删除';
              bDel.onclick = function () {
                if (!confirm('删除员工「' + (s.name || s.user) + '」？\n其创建的项目将转回你名下（项目本身保留）。')) return;
                api('/_gate/staff', { method: 'DELETE', body: JSON.stringify({ user: s.user }) }).then(function (r) {
                  if (r.ok) { refresh(); } else { alert(r.body.error || '删除失败'); }
                });
              };
              // 「进 ta 的视角」—— 下级的东西不并排铺在我这儿（附三 §13 原则④）
              var bView = document.createElement('button'); bView.className = 'ab-btn ghost sm'; bView.type = 'button'; bView.textContent = '进入对方视角';
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
        '<div class="ab-row"><label>项目额度</label><input class="ab-input" id="f-quota" type="number" min="0" max="50" value="' + (isNew ? 2 : esc(rec.quota)) + '" style="max-width:110px"><span style="color:var(--text-dim);font-size:13.5px">可创建项目数上限</span></div>' +
        '<div class="ab-row"><label>空间配额</label><input class="ab-input" id="f-quotamb" type="number" min="0" step="0.5" value="' + (rec && rec.quotaMb != null ? esc(Math.round(rec.quotaMb / 1024 * 10) / 10) : 0) + '" style="max-width:130px"><span style="color:var(--text-dim);font-size:13.5px">该成员可用的空间上限（计入你的配额）；0 = 不限制</span></div>' +
        '<div style="margin:12px 0 6px;color:var(--text);font-size:14px">可访问的项目' + (isNew ? '（建完再分配也行）' : '') + '</div>' +
        '<div id="f-projs">' + (projects.length
          ? projects.map(function (p) {
              var on = !isNew && (rec.grants || []).indexOf(p.key) >= 0;
              return '<label class="ab-chk"><input type="checkbox" value="' + esc(p.key) + '"' + (on ? ' checked' : '') + '> ' + esc(p.name) + ' <span style="color:var(--text-dim);font-size:13.5px">（' + esc(p.key) + '）</span></label>';
            }).join('')
          : '<div class="ab-tip">名下暂无项目。</div>') + '</div>' +
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
        // 单位 G。2026-09-17 老板拍板：「留空」这个状态不许存在 —— 留空直接拦下，
        //   要不定限制就显式填 0（界面上看得见的一个数）。
        if (quotaMbRaw === '') { msg(msgEl, '请填写空间配额（0 = 不限制）', false); return; }
        var gv = Number(quotaMbRaw);
        if (!(gv >= 0)) { msg(msgEl, '空间配额请填 0 或正数（单位 G）', false); return; }
        payload.quotaMb = Math.round(gv * 1024);
        if (owner) payload.owner = owner;
        if (pw) payload.password = pw;
        api('/_gate/staff', { method: 'POST', body: JSON.stringify(payload) }).then(function (r) {
          if (!r.ok) { msg(msgEl, r.body.error || '保存失败', false); return; }
          api('/_gate/staff/grant', { method: 'POST', body: JSON.stringify({ user: uname, grants: grants }) }).then(function (r2) {
            if (!r2.ok) { msg(msgEl, r2.body.error || '分配项目失败', false); return; }
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
  /** 给一个客户设「空间配额」（2026-09-17 老板：改成客户级总池子） */
  function openSpaceForm(u, onDone) {
    openLayer('空间配额 · ' + (u.name || u.user), function (body) {
      body.innerHTML =
        '<div class="ab-tip">该客户及其成员共用的总空间 —— 项目、文件、回收站合并计算。<br>单位 G；0 = 不限制。</div>' +
        '<div class="ab-row"><label>空间配额</label><input class="ab-input" id="sp-g" type="number" min="0" step="0.5" value="'
          + (u.quotaMb != null ? (Math.round(u.quotaMb / 1024 * 10) / 10) : 2) + '" style="max-width:110px">'
          + '<span style="color:var(--text-dim);font-size:13.5px">G（当前已用 ' + fmtSpace(u.spaceMb || 0) + '）</span></div>' +
        '<div style="display:flex;gap:8px;margin-top:16px"><button class="ab-btn" id="sp-save" type="button">保存</button>' +
        '<button class="ab-btn ghost" id="sp-cancel" type="button">取消</button></div><div class="ab-msg" id="sp-msg"></div>';
      var msgEl = body.querySelector('#sp-msg');
      body.querySelector('#sp-cancel').onclick = closeLayer;
      body.querySelector('#sp-save').onclick = function () {
        var raw = body.querySelector('#sp-g').value.trim();
        if (raw === '') { msg(msgEl, '请填写空间配额（0 = 不限制）', false); return; }
        var g = Number(raw);
        if (!(g >= 0)) { msg(msgEl, '请输入 0 或正数', false); return; }
        api('/_gate/users', { method: 'POST', body: JSON.stringify({ user: u.user, quotaMb: Math.round(g * 1024) }) })
          .then(function (r) {
            if (!r.ok) { msg(msgEl, (r.body && r.body.error) || '保存失败', false); return; }
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
      // 转出时的可选项：管理员自己 ＋ 其他客户账号（转给谁都不会丢）
      var adminUser = (r.body && r.body.admin) || 'admin';
      function ownerOptions(except) {
        var opts = [{ user: adminUser, name: '管理员（平台自己）' }].concat(
          users.filter(function (x) { return x.user !== except; })
            .map(function (x) { return { user: x.user, name: x.name || x.user }; }));
        return opts.map(function (o) {
          return '<option value="' + esc(o.user) + '">' + esc(o.name) + '</option>';
        }).join('');
      }
      openLayer('客户管理', function (body) {
        body.innerHTML =
          '<div class="ab-tip">客户账号 = 一个客户公司。客户老板登录后能自己给员工建账号、分项目。<br>删客户前要把它名下的项目先转给别人 —— 就在下面每张卡上转。</div>' +
          '<button class="ab-btn" id="ab-add-cust" type="button">+ 添加客户</button>' +
          '<div id="ab-ulist" style="margin-top:14px"></div>';
        var listEl = body.querySelector('#ab-ulist');
        users.forEach(function (u) {
          var card = document.createElement('div'); card.className = 'ab-card';
          // 名下项目：**显示项目名**，每个后面带一个「转给…」下拉。
          // ⚠️ 转必须在**这里**能做成（2026-09-17 老板：「删客户提示要先转走 —— 我怎么转？？」）：
          //   管理员的「项目管理」面板只看得到**自己名下**的项目（`canUse` 按 owner 过滤），
          //   而这里要转的恰恰是**别人名下**的 → 那边根本看不到它，提示等于指了条走不通的路。
          //   工作台不用转（删账号时连带删），所以不给下拉。
          var projs = u.projects || [];
          var projHtml = projs.length
            ? projs.map(function (pr) {
                var label = '<span style="white-space:nowrap">' + esc(pr.name || pr.key) + '</span>';
                if (pr.workbench) return label + '<span style="color:var(--text-faint);font-size:13px">（随账号一起删）</span>';
                return label + '<select class="ab-input ab-mvproj" data-key="' + esc(pr.key) + '"'
                  + ' style="max-width:132px;padding:3px 6px;font-size:13px;margin-left:6px"'
                  + ' title="把这个项目转给别的账号"><option value="">转给…</option>'
                  + ownerOptions(u.user) + '</select>';
              }).join('　')
            : '无';
          card.innerHTML = '<div class="ab-card-top"><div><div class="ab-n">' + esc(u.name || u.user) + '</div>' +
            '<div class="ab-s">账号：' + esc(u.user) + ' ｜ 名下项目：' + projHtml
              + ' ｜ 空间已用 ' + fmtSpace(u.spaceMb || 0) + ' / ' + (u.quotaMb ? fmtSpace(u.quotaMb) : '不限制') + '</div></div></div>';
          var acts = document.createElement('div'); acts.style.cssText = 'display:flex;gap:7px;margin-top:10px';
          var bSpace = document.createElement('button'); bSpace.className = 'ab-btn ghost sm'; bSpace.type = 'button'; bSpace.textContent = '设置配额';
          bSpace.onclick = function () { openSpaceForm(u, openUsers); };
          acts.appendChild(bSpace);
          var bStaff = document.createElement('button'); bStaff.className = 'ab-btn ghost sm'; bStaff.type = 'button'; bStaff.textContent = '看员工';
          bStaff.onclick = function () { openStaff(u.user); };
          acts.appendChild(bStaff);
          var bDel = document.createElement('button'); bDel.className = 'ab-btn danger sm'; bDel.type = 'button'; bDel.textContent = '删除';
          bDel.onclick = function () {
            // 口径跟后端一致：**工作台不算拦路的项目**（它随账号一起删）
            var blocking = projs.filter(function (x) { return !x.workbench; });
            var owns = blocking.length
              ? '\n\n⚠ 名下还有项目：' + blocking.map(function (x) { return x.name || x.key; }).join('、')
                + '\n请先用上面的「转给…」把它们转给别的账号。'
              : '';
            if (!confirm('删掉客户「' + (u.name || u.user) + '」？' + owns)) return;
            api('/_gate/users', { method: 'DELETE', body: JSON.stringify({ user: u.user }) }).then(function (r2) {
              if (!r2.ok) { alert((r2.body && r2.body.error) || '删不掉'); return; }
              openUsers();
            });
          };
          acts.appendChild(bDel);
          card.appendChild(acts);
          // 「转给…」下拉：选一个账号就转走（选完刷新面板）
          card.querySelectorAll('.ab-mvproj').forEach(function (sel) {
            sel.onchange = function () {
              var to = sel.value;
              if (!to) return;
              var pkey = sel.getAttribute('data-key');
              var toName = sel.options[sel.selectedIndex].textContent;
              if (!confirm('把「' + pkey + '」转给「' + toName + '」？')) { sel.value = ''; return; }
              api('/_gate/projects/owner', { method: 'POST', body: JSON.stringify({ key: pkey, owner: to }) })
                .then(function (r2) {
                  if (!r2.ok) { alert((r2.body && r2.body.error) || '转不了'); sel.value = ''; return; }
                  openUsers();
                });
            };
          });
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
        '<div class="ab-row"><label>空间配额</label><input class="ab-input" id="c-space" type="number" min="0" step="0.5" value="2" style="max-width:110px"><span style="color:var(--text-dim);font-size:13.5px">G，默认 2；0 = 不限制（该客户及其成员共用）</span></div>' +
        '<div style="display:flex;gap:8px;margin-top:16px"><button class="ab-btn" id="c-save" type="button">保存</button>' +
        '<button class="ab-btn ghost" id="c-cancel" type="button">取消</button></div><div class="ab-msg" id="c-msg"></div>';
      var msgEl = body.querySelector('#c-msg');
      body.querySelector('#c-cancel').onclick = closeLayer;
      body.querySelector('#c-save').onclick = function () {
        // 空间配额：默认 2 G，**留空不许存**（2026-09-17 老板：留空既不该默认成 2G、
        //   也不该默默变成不限制 —— 干脆不许留空，界面上永远是一个明确的数）
        var spaceRaw = body.querySelector('#c-space').value.trim();
        if (spaceRaw === '') { msg(msgEl, '请填写空间配额（默认 2 G；0 = 不限制）', false); return; }
        var spaceG = Number(spaceRaw);
        if (!(spaceG >= 0)) { msg(msgEl, '空间配额请填 0 或正数（单位 G）', false); return; }
        var payload = {
          user: body.querySelector('#c-user').value.trim(),
          displayName: body.querySelector('#c-name').value.trim(),
          password: body.querySelector('#c-pw').value,
          quotaMb: Math.round(spaceG * 1024),
        };
        api('/_gate/users', { method: 'POST', body: JSON.stringify(payload) }).then(function (r) {
          if (!r.ok) { msg(msgEl, r.body.error || '保存失败', false); return; }
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
      ? api('/_gate/users').then(function (r) {
          var us = (r.body && r.body.users) || [];
          // ⚠️ 名单里**必须带上管理员自己**（2026-09-17 修）：`/_gate/users` 只返客户和员工，
          //   不含 admin —— 而管理员自己名下项目的 owner = 'admin'，在下拉里匹配不到任何一项，
          //   浏览器就**默认选中第一个**（= 第一个客户）→ 界面凭空显示成「归属：测试客户」，
          //   实际归属一点没变（老板 2026-09-17 就是这么被误导的：「动画系统到底归谁？」）。
          //   后果不止「显示错」：这个下拉是可改的，碰一下就把项目真转走了。
          var meUser = (r.body && r.body.admin) || (ME && ME.user) || 'admin';
          return [{ user: meUser, name: '管理员（平台自己）' }].concat(us);
        }).catch(function () { return []; })
      : Promise.resolve([]);
    ownersP.then(function (owners) { openProjectsWith(owners); });
  }
  /** 给项目改名（2026-09-19）—— 弹层而不是 prompt()：老板定过界面文案要「专业、克制」，
   *  浏览器原生 prompt 又丑又会带域名头。文案只讲**点了会发生什么**。
   *  ⚠️ 成功后**重开整个项目管理**（不是调那块列表的 refresh）：弹层一关，
   *    它里面的 `#ab-plist` 就没了，refresh 里那句 `if (!el) return` 会直接吞掉刷新 ——
   *    客户看到的就是「点了保存、名字没变」（2026-09-19 回归测试当场抓到）。 */
  function openRenameProject(p, onDone) {
    openLayer('项目改名', function (body) {
      body.innerHTML =
        '<div class="ab-tip">只改显示名。项目里的文件、对话记录、设置都不受影响。</div>' +
        '<div class="ab-row"><label>名字</label>' +
        '<input class="ab-input" id="ab-re-in" maxlength="60" style="flex:1" value="' + esc(p.name) + '"></div>' +
        '<div class="ab-msg" id="ab-re-msg" style="display:none"></div>' +
        '<div style="display:flex;gap:8px;justify-content:flex-end;margin-top:14px">' +
        '<button class="ab-btn sm" id="ab-re-cancel" type="button">取消</button>' +
        '<button class="ab-btn sm" id="ab-re-ok" type="button">保存</button></div>';
      var inp = body.querySelector('#ab-re-in');
      var box = body.querySelector('#ab-re-msg');
      inp.focus(); inp.select();
      body.querySelector('#ab-re-cancel').onclick = closeLayer;
      body.querySelector('#ab-re-ok').onclick = function () {
        var v = inp.value;
        if (!v.trim()) { box.style.display = ''; msg(box, '名字不能是空的', false); return; }
        api('/_gate/projects/rename', { method: 'POST', body: JSON.stringify({ key: p.key, name: v }) })
          .then(function (r) {
            if (!r.ok) { box.style.display = ''; msg(box, (r.body && r.body.error) || '改不了', false); return; }
            closeLayer();
            if (onDone) onDone();
          });
      };
      inp.addEventListener('keydown', function (e) { if (e.key === 'Enter') body.querySelector('#ab-re-ok').click(); });
    });
  }

  function openProjectsWith(owners) {
    var isAdmin = !!(ME && ME.role === 'admin');
    openLayer('项目管理', function (body) {
      body.innerHTML = '<div class="ab-tip">项目平时不占资源，点进去才会启动。</div><div id="ab-plist">加载中…</div>';
      function refresh() {
        api('/_gate/projects').then(function (r) {
          var list = (r.body && r.body.projects) || [];
          var el = body.querySelector('#ab-plist');
          if (!el) return;
          if (!list.length) { el.innerHTML = '<div class="ab-tip">暂无项目。</div>'; return; }
          el.innerHTML = '';
          // 工作台跟项目**分开两块**（2026-09-16 老板要）—— 它俩不是一类东西：
          //   工作台 = 平台给你的干活地方（不绑项目）；项目 = 要给业务用的系统。
          //   以前混在一个列表里，客户分不清「哪个是我的系统、哪个只是干活的地方」。
          function cardOf(p) {
            var card = document.createElement('div'); card.className = 'ab-card';
            card.innerHTML = '<div class="ab-card-top"><div><div class="ab-n">' + esc(p.name) + '</div>' +
              '<div class="ab-s">' + (p.paused
                ? '<span style="color:var(--human)">已暂停（已释放资源）</span>'
                : '<span style="color:var(--live)">运行中</span>') +
              (p.editable ? '' : ' ｜ 此项目无独立运行环境') + '</div></div></div>';
            var acts = document.createElement('div'); acts.style.cssText = 'display:flex;gap:7px;margin-top:10px;flex-wrap:wrap';
            // 改名（2026-09-19 老板：「凡是用户能够通过 ai 设置的无风险事项，一律交给用户和 ai，
            //   不要写死」）—— 项目名只是 projects.json 里的一个**显示字段**：
            //   目录用的是 key（随机 ID），引擎 / 预览 / 对话 / 快照都不读它 ⇒ 改了不影响任何东西。
            //   所以它**不锁在平台侧**：用户在这里改，AI 走门卫的 /_gate/ai/projects/rename 也能改。
            //   （2026-09-19 之前根本没有这个入口 —— 老板让 AI 改项目名时，AI 编了两个不存在的菜单项。）
            var bRe = document.createElement('button');
            bRe.className = 'ab-btn sm';
            bRe.type = 'button';
            bRe.textContent = '改名';
            bRe.onclick = function () { openRenameProject(p, openProjects); };
            acts.appendChild(bRe);
            // 工作台不给删（2026-09-16 老板定 A）：它是平台给你的干活入口、不是客户的项目，
            //   删了连里面所有产出文件（PPT/Excel）一起没，也没地方补。后端同时拦着，两层。
            if (p.workbench) {
              var wbHint = document.createElement('div');
              wbHint.className = 'ab-s';
              wbHint.style.cssText = 'align-self:center;color:var(--text-dim)';
              wbHint.textContent = '工作台不可删除';
              acts.appendChild(wbHint);
            } else {
            var bDel = document.createElement('button');
            bDel.className = 'ab-btn sm';
            bDel.type = 'button';
            bDel.textContent = '删除';
            bDel.onclick = function () {
              // 2026-09-17（老板批准的项目回收站）：删除 = **放进回收站**，不真删 ——
              //   员工删项目不用等审批；要真删干净，去「回收站」里彻底删除。
              if (!confirm('删除项目「' + p.name + '」？\n\n· 它会从项目列表移除，并放进回收站，之后可以还原\n· 资料、对话记录与设置都会保留\n\n确定继续？')) return;
              api('/_gate/projects/delete', { method: 'POST', body: JSON.stringify({ key: p.key }) }).then(function (r2) {
                if (!r2.ok) { alert((r2.body && r2.body.error) || '删不掉'); return; }
                if (r2.body && r2.body.note) alert(r2.body.note);
                refresh();
              });
            };
            acts.appendChild(bDel);
            }
            // 管理员：转移项目归属
            if (isAdmin && owners.length) {
              var sel = document.createElement('select');
              sel.className = 'ab-input';
              sel.style.cssText = 'max-width:150px;padding:5px 8px;font-size:13.5px';
              sel.title = '转移项目归属';
              owners.forEach(function (o) {
                var op = document.createElement('option');
                op.value = o.user;
                op.textContent = o.name || o.user;
                if (o.user === p.owner) op.selected = true;
                sel.appendChild(op);
              });
              // 兜底：归属不在名单里（账号被删了 / 老数据）→ **如实显示**并选中它，
              //   绝不让浏览器自作主张选第一个 —— 那会在界面上写一个错的归属。
              if (!owners.some(function (o) { return o.user === p.owner; })) {
                var opx = document.createElement('option');
                opx.value = p.owner || '';
                opx.textContent = p.owner ? (p.owner + '（账号已不存在）') : '未设置归属';
                sel.insertBefore(opx, sel.firstChild);
                opx.selected = true;
              }
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
            return card;
          }
          function addGroup(title, hint, arr) {
            if (!arr.length) return;
            var h = document.createElement('div');
            h.style.cssText = 'margin:18px 0 7px';
            h.innerHTML = '<div style="font-size:14.5px;font-weight:600;color:var(--text-soft)">' + title + '</div>' +
              (hint ? '<div style="font-size:12.5px;color:var(--text-dim);margin-top:3px">' + hint + '</div>' : '');
            el.appendChild(h);
            arr.forEach(function (p) { el.appendChild(cardOf(p)); });
          }
          addGroup('我的工作台', '独立于项目的工作区 —— 做 PPT / 表格 / 文档、查资料',
            list.filter(function (p) { return p.workbench; }));
          addGroup('我的项目', '面向业务的系统',
            list.filter(function (p) { return !p.workbench; }));
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
      // ⚠️ 口径跟桌面的「我的空间」**必须是同一个接口**（`/_gate/space` —— 客户级总池子）。
      //   以前这里用 `/_gate/usage`，三个毛病（2026-09-17 老板问「那是什么」时实测出来的）：
      //     ① 它不过滤归属 → 员工点开能看到**别人名下的项目**；
      //     ② 它把**服务器总磁盘**吐给客户（违反硬规矩 ⑲）；
      //     ③ 口径是旧的「每个项目 1G」。
      //   接口那边也已同时收敛（取消磁盘输出 ＋ 按人过滤）。
      api('/_gate/space').then(function (r) {
        var el = body.querySelector('#ab-space');
        if (!el) return;
        if (!r.ok) { el.textContent = (r.body && r.body.error) || '读取失败，请稍后重试'; return; }
        var d = r.body || {};
        var s = d.summary || {};
        var pct = (!s.unlimited && s.quotaMb) ? Math.min(100, Math.max(s.usedMb > 0 ? 2 : 0, s.pct || 0)) : 0;
        var html = '<div class="ab-card"><div class="ab-n">空间</div>' +
          '<div class="ab-s">已用 ' + fmtMb(s.usedMb) + ' / ' +
          (s.unlimited ? '不限制' : (fmtMb(s.quotaMb) + '（剩余 ' + fmtMb(s.freeMb) + '）')) + '</div>' +
          (s.fromClient ? '<div class="ab-s">这是分配给你的额度（计入上级账号的空间）</div>' : '') +
          '<div style="height:6px;background:var(--line);border-radius:3px;margin-top:8px;overflow:hidden">' +
          '<div style="height:100%;width:' + pct + '%;background:' + ((s.pct || 0) >= 90 ? 'var(--danger)' : 'var(--live)') + '"></div></div></div>';
        var items = d.items || [];
        if (!items.length) {
          html += '<div class="ab-tip">还没有占用。</div>';
        } else {
          html += '<div class="ab-tip">项目、工作台、回收站、文件、引擎数据都从这一个额度里出。</div>';
          items.forEach(function (it) {
            html += '<div class="ab-card"><div class="ab-n">' + esc(it.name) + '</div>' +
              '<div class="ab-s">' + esc(it.group) + ' ｜ ' + fmtMb(it.mb) + '</div></div>';
          });
        }
        el.innerHTML = html;
      });
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
      body.innerHTML = '<div id="ab-skills"><div class="ab-tip">正在问它…</div></div><div id="ab-apps"></div>';
      abLoadApps(body);
      api('/v1/skills').then(function (r) {
        var el = body.querySelector('#ab-skills');
        if (!el) return;
        if (!r.ok) {
          el.innerHTML = '<div class="ab-tip">读不到清单：' + esc((r.body && r.body.error) || r.code) + '</div>';
          return;
        }
        var list = ((r.body || {}).skills || []);
        var known = {};
        var html = '<div class="ab-tip">以下为<b>内置技能</b>，无需手动选择：' +
          '直接说明需求，AI 会自动匹配。</div>' +
          // 2026-09-18：告诉客户「技能可以自己加」——引擎原生支持（项目里的 skills/ 目录），
          //   不需要平台代装（实测：建完引擎立刻认到，不用重启）。详见档案 §8.7 143。
          '<div class="ab-tip">想让 AI 会做别的事？直接跟它说「<b>帮我在项目里加一个技能，用来做 XX</b>」——' +
          '它会建在你自己的项目里，<b>只有你能用</b>。</div>';
        SKILL_GROUPS.forEach(function (g) {
          var rows = '';
          g[1].forEach(function (it) {
            var sk = null;
            for (var i = 0; i < list.length; i++) if (list[i].name === it[0]) { sk = list[i]; break; }
            if (!sk) return;                     // 引擎当前没装 → 不显示（表是死的，清单是活的）
            known[it[0]] = 1;
            rows += abSkillCard(it[1], it[2], sk);
          });
          if (rows) html += '<div style="margin:12px 0 6px;color:var(--text);font-size:14px">' + esc(g[0]) + '</div>' + rows;
        });
        var rest = [];
        for (var j = 0; j < list.length; j++) if (!known[list[j].name]) rest.push(list[j]);
        if (rest.length) {
          html += '<div style="margin-top:14px"><button class="ab-btn ghost sm" id="ab-sk-more" type="button">查看其余 ' +
            rest.length + ' 项</button></div><div id="ab-sk-rest" hidden>' +
            rest.map(function (s) { return abSkillCard(s.name, (s.description || '').slice(0, 120), s); }).join('') + '</div>';
        }
        html += '<div class="ab-tip" style="margin-top:14px">需要更多能力？告知平台即可，无需自行安装。</div>';
        el.innerHTML = html;
        var more = el.querySelector('#ab-sk-more');
        if (more) more.onclick = function () {
          var box = el.querySelector('#ab-sk-rest');
          var wasHidden = box.hidden;
          box.hidden = !wasHidden;
          more.textContent = wasHidden ? '收起' : ('查看其余 ' + rest.length + ' 项');
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
  /* ── 已装的外挂（2026-09-19 · 搬表：官方 `/v1/apps/*`，**只搬「看 + 启用/停用」**）──
   * 官方 web **完全没有这块**（runtime_web/ grep 零命中）—— 我们补空白。
   * 为什么**不给装新的**：装插件 = 把**外部代码**装进服务器、以客户身份执行。
   *   官方源码自己写明：插件声明的文件/网络清单**不是沙箱边界**，stdio MCP 是
   *   **以宿主用户权限起子进程**（`plugins/manifest.rs` 的 stdio_mcp_servers 注释原文）。
   *   所以「装 / 信任 / 撤销 / 更新」由平台做 —— 门卫那边也堵着（`BLOCKED_APP_PATHS`）。
   * ⚠️ 开关是**按账号各自一份**的（2026-09-19 实测：状态落在各自的
   *   `$HOME/.codewhale/plugins/state.json`，两个引擎的 HOME 各是各的）→ **你开关不影响别人**。
   * ⚠️ 启用前**必须先「看清它能碰什么」再确认** —— 官方就是这么设计的（引擎会拒
   *   "requires capability review before enablement"），所以这里先弹确认框，不静默开。
   */
  function abInvText(inv) {
    var parts = [], n = function (k) { return Number((inv || {})[k] || 0); };
    if (n('skills')) parts.push(n('skills') + ' 项技能');
    if (n('stdio_mcp_servers')) parts.push(n('stdio_mcp_servers') + ' 个外部工具（会在这台机器上起程序）');
    if (n('remote_mcp_servers')) parts.push(n('remote_mcp_servers') + ' 个联网的外部工具');
    if (n('commands')) parts.push(n('commands') + ' 条命令');
    if (n('agents')) parts.push(n('agents') + ' 个智能体');
    if (n('hooks')) parts.push(n('hooks') + ' 个自动动作');
    if (n('native')) parts.push(n('native') + ' 个本机程序');
    return parts.length ? parts.join(' · ') : '没有额外能力';
  }
  /** 这个外挂能不能在**这台机器**上用（它自己声明支持的平台） */
  function abHostOk(pf) {
    if (!pf || !pf.length) return true;          // 没声明 = 不限平台
    return pf.indexOf('linux') >= 0;
  }
  function abAppCard(x, acc) {
    var okHost = abHostOk(x.platforms);
    var state = x.enabled ? '已启用' : (okHost ? '已停用' : '本机用不了');
    var col = x.enabled ? 'var(--live)' : 'var(--text-dim)';
    var h = '<div class="ab-card">' +
      '<div class="ab-card-top"><span class="ab-n">' + esc(x.name) + '</span>' +
      '<span class="ab-s" style="color:' + col + ';margin:0">' + state + '</span></div>';
    if (x.desc) h += '<div class="ab-s">' + esc(String(x.desc).slice(0, 160)) + '</div>';
    h += '<div class="ab-s">能碰什么：' + esc(abInvText(x.inv)) + '</div>';
    if (!okHost) {
      // 本机用不了的就不给「启用」—— 点了必然报错（实测：引擎回 409 "does not apply to this host"）
      h += '<div class="ab-s">它只支持 ' + esc(x.platforms.join(' / ')) +
           '，在这台机器上启动了也没用，所以不提供开启。</div>';
    } else {
      h += '<div style="margin-top:9px"><button class="ab-btn' + (x.enabled ? ' ghost' : '') +
           '" data-app-act="' + (x.enabled ? 'disable' : 'enable') +
           '" data-app-acc="' + esc(acc) + '" data-app-sel="' + esc(x.sel) +
           '" data-app-name="' + esc(x.name) + '">' + (x.enabled ? '停用' : '启用') + '</button></div>';
    }
    return h + '</div>';
  }
  function abLoadApps(body) {
    var el = body.querySelector('#ab-apps');
    if (!el) return;
    el.innerHTML = '<div class="ab-tip" style="margin-top:16px">正在读已装的外挂…</div>';
    api('/_gate/apps').then(function (r) {
      if (!body.querySelector('#ab-apps')) return;      // 面板已被关掉
      if (!r.ok || !r.body || r.body.ok === false) { el.innerHTML = ''; return; }
      var groups = ((r.body || {}).accounts || []).filter(function (g) {
        return g && g.items && g.items.length;
      });
      if (!groups.length) { el.innerHTML = ''; return; }   // 一个外挂都没有就不摆这一块
      var html = '<div class="ab-tip" style="margin-top:18px"><b>已装的外挂</b>（插件）—— ' +
        '它们给 AI 加额外的本事。这里只能看和开关，<b>装新的由平台来做</b>。</div>';
      groups.forEach(function (g) {
        (g.items || []).forEach(function (x) { html += abAppCard(x, g.account); });
      });
      el.innerHTML = html;
    });
  }
  /** 启用 / 停用一个外挂。**启用前先让客户看清它能碰什么**（官方语义：先审查再启用）。 */
  function abAppAct(btn) {
    var acc = btn.getAttribute('data-app-acc'), sel = btn.getAttribute('data-app-sel');
    var nm = btn.getAttribute('data-app-name'), act = btn.getAttribute('data-app-act');
    var card = btn.closest('.ab-card');
    var invLine = card ? ((card.querySelectorAll('.ab-s')[1] || {}).textContent || '') : '';
    var go = function () {
      var old = btn.textContent; btn.disabled = true; btn.textContent = act === 'enable' ? '正在开启…' : '正在停用…';
      api('/_gate/apps/act', {
        method: 'POST',
        body: JSON.stringify({ account: acc, id: sel, action: act }),
      }).then(function (r) {
        btn.disabled = false; btn.textContent = old;
        if (r.ok && r.body && r.body.ok !== false) {
          notify(nm + (act === 'enable' ? '：已开启' : '：已停用'));
          var p = document.querySelector('#ab-apps');
          if (p) abLoadApps(document);          // 传 document 就行 —— 它内部只查 #ab-apps
          return;
        }
        var why = (r.body && (r.body.error || r.body.message)) || ('HTTP ' + r.code);
        if (r.body && r.body.detail) {
          try { var d = JSON.parse(r.body.detail); why = (d.error && d.error.message) || why; } catch (e) {}
        }
        notify('没成功：' + why);
      });
    };
    if (act === 'enable') {
      confirmAb('开启「' + nm + '」',
        '<p>开启后，它会给 AI 加上这些本事：</p><p style="margin:10px 0"><b>' +
        esc((invLine || '').replace(/^能碰什么：/, '')) + '</b></p>' +
        '<p>这些是它自己声明的。它会在<b>你自己的项目</b>里跑，不影响别人。</p>', go);
    } else { go(); }
  }
  // 事件委托绑在 document 上（**只绑一次** —— 面板会反复重建，绑在里面会累积）
  document.addEventListener('click', function (e) {
    var b = e.target && e.target.closest ? e.target.closest('[data-app-act]') : null;
    if (b) { e.preventDefault(); abAppAct(b); }
  });

  /** 一个最简单的确认框（两个按钮）。刻意不再造一套弹层 —— 用浏览器原生 confirm 的面子最稳。 */
  function confirmAb(title, html, onOk) {
    var wrap = document.createElement('div');
    wrap.setAttribute('role', 'dialog');
    wrap.style.cssText = 'position:fixed;inset:0;z-index:9999;background:rgba(0,0,0,.55);' +
      'display:flex;align-items:center;justify-content:center;padding:24px';
    wrap.innerHTML = '<div style="background:var(--surface);border:1px solid var(--line);' +
      'border-radius:12px;padding:18px 20px;max-width:560px;width:100%;color:var(--text)">' +
      '<div style="font-size:16px;margin-bottom:10px">' + esc(title) + '</div>' +
      '<div class="ab-s" style="line-height:1.7">' + html + '</div>' +
      '<div style="display:flex;gap:10px;justify-content:flex-end;margin-top:16px">' +
      '<button class="ab-btn ghost" data-cf="no">取消</button>' +
      '<button class="ab-btn" data-cf="yes">确认开启</button></div></div>';
    wrap.addEventListener('click', function (e) {
      var k = e.target.getAttribute && e.target.getAttribute('data-cf');
      if (k === 'yes') { document.body.removeChild(wrap); onOk(); }
      else if (k === 'no' || e.target === wrap) { document.body.removeChild(wrap); }
    });
    document.body.appendChild(wrap);
  }

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
    if (m) { m.className = 'ab-msg'; m.textContent = '正在重启 AI 服务（约几秒）以使设置生效…'; }
    // 记忆也是**全局偏好**（2026-09-16 老板点名：「包括记忆也是」）——
    // 一次设置，名下所有项目都生效（没在跑的项目等下次打开时自动补）。
    api('/_gate/prefs', {
      method: 'POST',
      body: JSON.stringify({ prefs: { memory_enabled: on ? 'true' : 'false' } }),
    }).then(function (r) {
      if (!r.ok || (r.body && r.body.ok === false)) {
        if (btn) { btn.disabled = false; btn.textContent = on ? '开启记忆' : '关闭记忆'; }
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

  /* ══ 用量明细（平台管理员视角 · 2026-09-20 老板要）══════════════════════════
   * 老板原话：「想能够看到每个账号的 token 消耗明细，以便了解用户使用情况和为以后定价做准备」。
   *
   * 数据源：门卫 `/_gate/token-usage` ← 各账号**引擎自带的账本**（每次调模型按 provider
   *   报回来的 usage 落账）。所以这里显示的是**厂商口径的原始值**，不是前端估的。
   * ⚠️ 两条口径必须写在界面上（不然数字看着像账单、实际不是）：
   *   ① 金额是**引擎按内置价目表推算**的（含 DeepSeek 的峰谷/周末价），不是厂商账单；
   *   ② 「未归属」= 没有记到具体项目的会话（平台内部/测试对话），不是客户的项目。
   * ⚠️ 这是跨账号的平台视角，只给管理员（菜单里也只在 admin 下显示）。
   */
  var TU = { preset: 'all', since: '', until: '' };
  /* 时间区间（2026-09-20 老板要）。
   * ⚠️ 日期一律用 **UTC 日**（`toISOString` 就是 UTC）—— 与引擎的「一天」口径一致
   *   （引擎按 UTC 日分桶）。用本地时区算会让「今天」的合计与按天列表对不上。 */
  function tuUtcDay(shift) {
    var d = new Date();
    if (shift) d.setUTCDate(d.getUTCDate() + shift);
    return d.toISOString().slice(0, 10);
  }
  var TU_PRESETS = [['all', '全部'], ['today', '今天'], ['7d', '近 7 天'], ['30d', '近 30 天'], ['month', '本月']];
  function tuPresetRange(p) {
    if (p === 'today') return { since: tuUtcDay(0), until: tuUtcDay(0) };
    if (p === '7d') return { since: tuUtcDay(-6), until: tuUtcDay(0) };
    if (p === '30d') return { since: tuUtcDay(-29), until: tuUtcDay(0) };
    if (p === 'month') return { since: tuUtcDay(0).slice(0, 7) + '-01', until: tuUtcDay(0) };
    return { since: '', until: '' };
  }
  function tuRangeBar() {
    var h = '<div style="display:flex;flex-wrap:wrap;gap:6px;align-items:center;margin:0 0 6px">';
    TU_PRESETS.forEach(function (p) {
      h += '<button class="ab-btn ghost sm" data-range="' + p[0] + '" type="button"' +
        (TU.preset === p[0] ? ' style="border-color:var(--action);color:var(--text)"' : '') +
        '>' + p[1] + '</button>';
    });
    h += '<span class="ab-s" style="margin:0 2px">自定义</span>' +
      '<input type="date" class="ab-input" id="tu-since" style="width:auto;padding:5px 8px" value="' + esc(TU.since) + '">' +
      '<span class="ab-s" style="margin:0">~</span>' +
      '<input type="date" class="ab-input" id="tu-until" style="width:auto;padding:5px 8px" value="' + esc(TU.until) + '"></div>';
    h += '<div class="ab-s" style="margin:0 0 10px">区间按「引擎日」切（UTC，北京时间早 8 点换日）—— 与下面的按天列表完全对齐；全部就是全历史。</div>';
    return h;
  }
  function tuRefresh() { return '<button class="ab-btn ghost sm" id="tu-refresh" type="button">刷新</button>'; }
  function tuExportBtn() { return '<button class="ab-btn ghost sm" id="tu-export" type="button" style="margin-left:6px">导出 CSV</button>'; }
  function tuCsvCell(v) {
    var s = String(v == null ? '' : v);
    return /[",\n\r]/.test(s) ? '"' + s.replace(/"/g, '""') + '"' : s;
  }
  /** 导出当前视图（账号 × 天）为 CSV —— 老板 2026-09-23：「用量页导出 CSV」（为定价做准备）。
   *  ⚠️ 导的是**屏幕上这一份**（当前区间、当前数据），不重新拉接口 —— 所见即所得。
   *  ⚠️ 开头加 UTF-8 BOM：不加的话 Excel（Windows）按 GBK 解，中文全是乱码。
   *  ⚠️ 纯前端 Blob 下载，**不新增门卫端点** —— 少一处白名单 / 权限 / 登录态要维护。
   *     桌面窗口那个 iframe 没有 sandbox 属性，下载不会被拦（2026-09-23 查过源码并真机验过）。 */
  function tuExportCsv() {
    var d = TU.last || {};
    var usd = DISPLAY.cost_currency !== 'cny';
    var cur = usd ? 'USD' : 'CNY';
    var num = function (n) { return Math.round(Number(n) || 0); };
    var moneyOf = function (b) { b = b || {}; return Number(usd ? b.costUsd : b.costCny) || 0; };
    var rows = [['粒度', '客户', '账号', '名称', '角色', '日期(UTC)', '调用次数',
                 '输入token', '输出token', '缓存token', '推理token', '金额', '币种', '模型']];
    function push(gran, clientLabel, a, date, b) {
      rows.push([gran, clientLabel || '', a.account || '', a.label || '', a.roleZh || '', date || '',
        num(b.calls), num(b.inTok), num(b.outTok), num(b.cachedTok), num(b.reasonTok),
        moneyOf(b).toFixed(2), cur,
        (a.models || []).map(function (m) { return m.model; }).join(' ')]);
    }
    function one(clientLabel, a) {
      if (a.totals) push('账号合计', clientLabel, a, '', a.totals);
      (a.days || []).forEach(function (day) { push('按天', clientLabel, a, day.date, day); });
    }
    var clients = d.clients || [];
    if (clients.length) {
      clients.forEach(function (c) {
        (c.accounts || []).forEach(function (a) { one(c.label || c.key, a); });
      });
    } else {
      (d.accounts || []).forEach(function (a) { one('', a); });
    }
    var csv = '\ufeff' + rows.map(function (r) { return r.map(tuCsvCell).join(','); }).join('\r\n') + '\r\n';
    var name = '用量明细_' + (TU.since || '全部') + (TU.until ? ('_至_' + TU.until) : '') + '.csv';
    var url = URL.createObjectURL(new Blob([csv], { type: 'text/csv;charset=utf-8' }));
    var a2 = document.createElement('a');
    a2.href = url; a2.download = name;
    document.body.appendChild(a2);
    a2.click();
    setTimeout(function () { URL.revokeObjectURL(url); a2.remove(); }, 3000);
  }
  function tuN(n) { return String(Math.round(Number(n) || 0)).replace(/\B(?=(\d{3})+(?!\d))/g, ','); }
  function tuW(n) {
    n = Number(n) || 0;
    if (n >= 10000) { var w = n / 10000; return (w >= 100 ? String(Math.round(w)) : String(Math.round(w * 10) / 10)) + ' 万'; }
    return tuN(n);
  }
  /* 金额符号跟着「货币单位」偏好走（2026-09-22 修 —— 档案 §8.7 201：以前这里硬编码 ￥，
   *   于是同一个人的用量页里，「高级设置」那行显示 $ 而这里显示 ￥，两种口径。
   *   ⚠️ 偏好是**按人**的（DISPLAY.cost_currency 来自该账号的 /v1/config）；管理员看的是所有账号
   *   的用量，用他自己的偏好显示，与界面其余部分一致。 */
  function tuY(v) {
    var usd = DISPLAY.cost_currency !== 'cny';
    return (usd ? '$' : '￥') + (Number(v) || 0).toFixed(2);
  }
  /** 金额（给同时有 cny / usd 两个原值的场景挑一个）。
   *  引擎两个原值都报，直接按偏好取，不做汇率换算。 */
  function tuMoney(cny, usd) { return tuY(DISPLAY.cost_currency === 'cny' ? cny : (usd == null ? cny : usd)); }
  function tuLine(left, mid, right) {
    return '<div style="display:flex;gap:10px;align-items:baseline;padding:3px 0;font-size:13.5px;color:var(--text-dim)">' +
      '<span style="min-width:82px">' + esc(left) + '</span>' +
      '<span style="flex:1">' + mid + '</span>' +
      '<span style="min-width:70px;text-align:right;color:var(--text)">' + right + '</span></div>';
  }
  function tuAccountCard(a, nested) {
    var h = '<div class="ab-card"' + (nested ? ' style="margin:10px 0 0;padding:9px 11px"' : '') + '>';
    h += '<div class="ab-card-top"><b>' + esc(a.label || a.account) + '</b>' +
      '<span class="ab-s" style="margin:0">' + (a.isSelf ? '本账号' : esc(a.roleZh || '账号')) +
      ' · ' + esc(a.account) + (a.port ? ' · 端口 ' + a.port : '') +
      (a.model ? ' · ' + esc(a.model) : '') + '</span></div>';
    if (a.error || !a.totals) {
      h += '<div class="ab-s">读不到这个账号的用量：' + esc(a.error || '引擎没在跑') + '</div>';
      h += '<div class="ab-s">（账在引擎家的盘上，引擎起来就还在；不是丢了。）</div></div>';
      return h;
    }
    var t = a.totals;
    h += '<div class="ab-s" style="font-size:16px;color:var(--text)">' + tuMoney(t.costCny, t.costUsd) +
      '　<span style="font-size:13.5px">（调用 ' + tuN(t.calls) + ' 次）</span></div>';
    h += '<div class="ab-s">进 ' + tuW(t.inTok) + ' · 出 ' + tuW(t.outTok) +
      ' · 缓存命中 ' + tuW(t.cachedTok) + ' · 推理 ' + tuW(t.reasonTok) + '</div>';
    if (!t.complete) {
      h += '<div class="ab-s" style="color:var(--warning)">其中 ' + tuN(t.unpricedCalls) +
        ' 次调用缺定价凭据 —— token 照记，金额少算了一点。</div>';
    }
    if (a.days && a.days.length) {
      h += '<div class="ab-s" style="margin-top:10px;color:var(--text)">按天</div>';
      a.days.forEach(function (d) {
        h += tuLine(d.date,
          tuN(d.calls) + ' 次 · 进 ' + tuW(d.inTok) + ' · 出 ' + tuW(d.outTok) + ' · 缓存 ' + tuW(d.cachedTok),
          tuMoney(d.costCny, d.costUsd));
      });
    }
    if (a.projects && a.projects.length) {
      h += '<div class="ab-s" style="margin-top:10px;color:var(--text)">按项目</div>';
      a.projects.forEach(function (p) {
        var b = p.brief || {};
        h += tuLine(p.name || p.key || '未归属',
          tuN(b.calls) + ' 次 · 进 ' + tuW(b.inTok) + ' · 出 ' + tuW(b.outTok) + ' · 缓存 ' + tuW(b.cachedTok),
          tuMoney(b.costCny, b.costUsd));
      });
    }
    // 按模型（2026-09-23 老板要「为定价准备」）—— 引擎 `group_by=model` 的原值，不是我们估的。
    //   一个模型时也照常出一行（不特判：「只有一个模型就不显示」会让老板以为坏了）。
    if (a.models && a.models.length) {
      h += '<div class="ab-s" style="margin-top:10px;color:var(--text)">按模型</div>';
      a.models.forEach(function (m) {
        var b = m.brief || {};
        h += tuLine(m.model || '（未标注）',
          tuN(b.calls) + ' 次 · 进 ' + tuW(b.inTok) + ' · 出 ' + tuW(b.outTok) + ' · 缓存 ' + tuW(b.cachedTok),
          tuMoney(b.costCny, b.costUsd));
      });
    }
    h += '</div>';
    return h;
  }
  /** 客户行（可点开）—— 客户级消耗 = **客户自己的账号 ＋ 名下员工账号**。
   *  老板 2026-09-20：「客户一行／自己账号消耗的、客户级账号消耗的；客户行点击后展开名下所有
   *  账号（包括自己账号）的明细」—— 所以行上先把两部分拆开写清楚，再点开展明细。 */
  function tuClientCard(c) {
    var t = c.totals || {};
    var h = '<div class="ab-card" style="padding:0;overflow:hidden">';
    h += '<div data-cli="' + esc(c.key) + '" style="display:flex;gap:8px;align-items:baseline;padding:11px 13px 4px;cursor:pointer">' +
      '<span class="tu-caret" style="min-width:12px;color:var(--text-dim)">▸</span>' +
      '<b>' + esc(c.label) + '</b>' +
      '<span class="ab-s" style="margin:0">' + esc(c.roleZh) + ' · ' + esc(c.key) + '</span>' +
      '<span style="flex:1"></span>' +
      '<span style="color:var(--text)">' + tuMoney(t.costCny, t.costUsd) + '</span>' +
      '<span class="ab-s" style="margin:0;min-width:56px;text-align:right">' + tuN(t.calls) + ' 次</span></div>';
    h += '<div class="ab-s" style="padding:0 13px 6px 33px">进 ' + tuW(t.inTok) + ' · 出 ' + tuW(t.outTok) +
      ' · 缓存命中 ' + tuW(t.cachedTok) + ' · 推理 ' + tuW(t.reasonTok) + '</div>';
    h += '<div class="ab-s" style="padding:0 13px 10px 33px">' +
      '本账号 ' + tuMoney(c.ownCny, c.ownUsd) + '（' + tuN(c.ownCalls) + ' 次）　·　' +
      '名下员工 ' + tuMoney(c.staffCny, c.staffUsd) + '（' + tuN(c.staffCalls) + ' 次）</div>';
    h += '<div data-body="' + esc(c.key) + '" hidden style="border-top:1px solid var(--line);padding:10px 13px 12px">';
    h += '<div class="ab-s" style="margin:0 0 2px">名下账号明细（' + c.accounts.length + ' 个）</div>';
    c.accounts.forEach(function (a) { h += tuAccountCard(a, true); });
    h += '</div></div>';
    return h;
  }
  function tuRender(d) {
    var g = d.grand || {};
    // 口径说明每次都要看得见（不能用 abTipPanel —— 那个只看一次就不再显示）
    var h = '<div class="ab-tip" style="border:1px solid rgba(106,174,242,.4);background:var(--action-soft);border-radius:8px;padding:8px 11px;margin:0 0 12px">' +
      '每个账号一套引擎，数据来自引擎自带的用量账本（厂商返回的原值）。' +
      '金额是按引擎内置价目表推算的，<b>不是厂商账单</b>；「未归属」是没有记到具体项目的对话（平台内部/测试）。' +
      (d.cached ? '<br>本次为 60 秒内的缓存结果，点「刷新」可强制重读。' : '') + '</div>';
    h += tuRangeBar() + '<div class="ab-card">';
    h += '<div class="ab-card-top"><b>全部账号合计</b>' + tuRefresh() + tuExportBtn() + '</div>';
    h += '<div class="ab-s" style="font-size:16px;color:var(--text)">' + tuMoney(g.costCny, g.costUsd) +
      '　<span style="font-size:13.5px">（调用 ' + tuN(g.calls) + ' 次）</span></div>';
    h += '<div class="ab-s">进 ' + tuW(g.inTok) + ' · 出 ' + tuW(g.outTok) + ' · 缓存命中 ' + tuW(g.cachedTok) +
      ' · 推理 ' + tuW(g.reasonTok) + '</div>';
    if (!g.complete) h += '<div class="ab-s" style="color:var(--warning)">含 ' + tuN(g.unpricedCalls) + ' 次缺定价凭据的调用（金额略少算）</div>';
    h += '</div>';
    // 客户级（老板 2026-09-20 要的层级）：先客户行，点开展开名下账号
    var clients = d.clients || [];
    if (clients.length) {
      h += '<div class="ab-s" style="margin:14px 0 6px;color:var(--text)">按客户（点一下展开名下的账号）</div>';
      clients.forEach(function (c) { h += tuClientCard(c); });
    } else {
      (d.accounts || []).forEach(function (a) { h += tuAccountCard(a); });
    }
    return h;
  }
  function openTokenUsage() {
    openLayer('用量明细', function (body) {
      function load(force) {
        body.innerHTML = '<div class="ab-tip">正在读各账号用量…</div>';
        var qs = [];
        if (TU.since) qs.push('since=' + encodeURIComponent(TU.since));
        if (TU.until) qs.push('until=' + encodeURIComponent(TU.until));
        if (force) qs.push('force=1');
        api('/_gate/token-usage' + (qs.length ? '?' + qs.join('&') : '')).then(function (r) {
          if (!r.ok) {
            body.innerHTML = '<div class="ab-tip">读不到：' + esc((r.body && r.body.error) || ('HTTP ' + r.code)) + '</div>';
            return;
          }
          TU.last = r.body;
          body.innerHTML = tuRender(r.body);
          var b = body.querySelector('#tu-refresh');
          if (b) b.onclick = function () { load(true); };
          var bx = body.querySelector('#tu-export');
          if (bx) bx.onclick = tuExportCsv;
          // 时间区间：快捷按钮 ＋ 自定义起止（2026-09-20 老板要）
          Array.prototype.forEach.call(body.querySelectorAll('[data-range]'), function (btn) {
            btn.onclick = function () {
              TU.preset = btn.getAttribute('data-range');
              var rg = tuPresetRange(TU.preset);
              TU.since = rg.since; TU.until = rg.until;
              load(false);
            };
          });
          var iS = body.querySelector('#tu-since'), iU = body.querySelector('#tu-until');
          if (iS) iS.onchange = function () { TU.preset = 'custom'; TU.since = iS.value; load(false); };
          if (iU) iU.onchange = function () { TU.preset = 'custom'; TU.until = iU.value; load(false); };
          // 客户行：点一下展开 / 收起名下的账号明细（老板 2026-09-20 要的交互）
          Array.prototype.forEach.call(body.querySelectorAll('[data-cli]'), function (row) {
            row.onclick = function () {
              var key = row.getAttribute('data-cli');
              var bx = body.querySelector('[data-body="' + key + '"]');
              if (!bx) return;
              bx.hidden = !bx.hidden;
              var car = row.querySelector('.tu-caret');
              if (car) car.textContent = bx.hidden ? '▸' : '▾';
            };
          });
        });
      }
      load(false);
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
            (e.stale ? ' · <span style="color:var(--human)">来源文件改过了，可能过期</span>' : '') +
            '</div></div>';
        }).join('') : ('<div class="ab-tip">' + (key ? '无匹配结果。' : '暂无记忆内容。') + '</div>');
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
            el.innerHTML = '<div class="ab-tip" style="color:var(--danger)">读不到记忆：' +
              esc(mem.error || ('HTTP ' + rs[1].code)) + '</div>';
            return;
          }
          if (cfg.memory_enabled === false) {
            el.innerHTML =
              '<div class="ab-tip">这个项目的「记忆」<b>还没打开</b>。</div>' +
              '<div class="ab-tip">开启后，AI 会记录对话中的偏好与习惯，' +
              '并在后续对话中自动带入（仅最近数十条）。<b>可随时查看、清空或关闭，' +
              '无需联系平台。</b></div>' +
              '<button class="ab-btn" id="mem-on" type="button">打开记忆</button><div class="ab-msg" id="mem-msg"></div>';
            body.querySelector('#mem-on').onclick = function () {
              var btn = this;
              abSetMemory(true, btn, load);
            };
            return;
          }
          var html =
            '<div class="ab-tip">AI 在对话中记住的内容（最近 32 条会带入对话，请勿作为资料库使用）。' +
            '仅作用于<b>本项目</b>。</div>' +
            '<div class="ab-row"><input class="ab-input" id="mem-q" placeholder="搜索（例如「客户」）" value="' + esc(q) + '"></div>';
          html += '<div id="mem-list"></div>';
          html += '<div class="ab-tip" style="margin-top:12px">此处仅支持<b>整批清空</b>，暂不支持单条删除。' +
            '如需删除某条记忆，可在对话中告知 AI（例如「忘掉关于报表格式的偏好」）。</div>';
          html += '<div style="display:flex;gap:8px;flex-wrap:wrap;align-items:center">' +
            '<button class="ab-btn danger sm" id="mem-clear-ws" type="button">清空本项目记忆</button>' +
            '<button class="ab-btn danger sm" id="mem-clear-global" type="button">清空通用记忆</button>' +
            '<button class="ab-btn ghost sm" id="mem-reload" type="button">刷新</button>' +
            '<button class="ab-btn ghost sm" id="mem-off" type="button" style="margin-left:auto">关闭记忆</button></div>' +
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
            if (!n) { msg(body.querySelector('#mem-msg'), '「' + label + '」目前是空的。', false); return; }
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
            if (!confirm('关闭记忆？\n关闭后 AI 不再记录新内容，也不再带入对话。\n（已记录的内容会保留；如需一并清除，请点击上方「清空」）')) return;
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
      Promise.all([api('/_gate/advanced'), api('/_gate/model-key'), api('/v1/config'), api('/_gate/repo'), api('/_gate/prefs')]).then(function (rs) {
        var r = rs[0];
        var mk = rs[1].body || {};
        var cfg = (rs[2] && rs[2].body) || {}
        var repo = (rs[3] && rs[3].body) || {};
        // ⚠️ 审批档从**平台侧**（`/_gate/prefs`）读，**不能**从引擎 `/v1/config` 读（2026-09-22 修，
        //   档案 §8.7 201 第②条衍生出的真 bug）—— 平台侧存的是三档（bypass/auto/suggest），
        //   而引擎 `approval_policy` **不认 bypass**（写前得映射成 auto，见门卫）
        //   ⇒ 引擎读出来永远是映射后的值。后果（实测）：客户选了「完全访问」，
        //   对话上方的「审批」标签说「完全访问」（读线程 posture，对的），
        //   而这里却说「自动审核」—— 同一件事两处不一致（老板一眼就能看到）。
        //   写路径本来就是平台侧（`setCfg` → `/_gate/prefs`），显示也得同源才自洽。
        var am = (rs[4] && rs[4].body && rs[4].body.prefs && rs[4].body.prefs.approval_mode) || 'auto';
        var cur = cfg.cost_currency === 'cny' ? 'cny' : 'usd';
        // 2026-09-19：官方 /config 里本来就有的显示类键（以前只接了一半，客户调不了）
        var diffsMode = (cfg.inline_diffs === 'summary' || cfg.inline_diffs === 'off') ? cfg.inline_diffs : 'full';
        // thinking_preview_lines 暂时可能读不到（引擎还没暴露）→ 退回官方默认 2
        var thinkLines = (typeof cfg.thinking_preview_lines === 'number') ? cfg.thinking_preview_lines : 2;
        // 思考强度（推理级别）：只认官方 DeepSeek 路由那 5 档（见 EFFORT_OPTS）；读到别的值就显「自动」
        var effortVals = EFFORT_OPTS.map(function (o) { return o.v; });
        var effort = effortVals.indexOf(cfg.reasoning_effort) >= 0 ? cfg.reasoning_effort : 'auto';
        var loc = cfg.locale || 'auto';
        var el = body.querySelector('#ab-adv');
        if (!el) return;
        if (!r.ok) {
          el.innerHTML = '<div class="ab-tip">' + esc((r.body && r.body.error) || '读不到') + '</div>';
          return;
        }
        var d = r.body || {};
        var u = d.usage;
        el.innerHTML =
          '<div class="ab-tip">以下设置会应用到<b>你的所有项目</b>（含以后新建的）；只有「只读模式」是个例外（它按项目走，下面单说）。</div>' +
          '<div class="ab-row"><label>模型服务</label><span class="ab-input" style="cursor:default;color:var(--text-dim);overflow:hidden;text-overflow:ellipsis;white-space:nowrap">' +
            esc(mk.provider || '—') + ' · ' + esc(mk.model || '—') + ' · ' + (mk.hasKey ? '已配置密钥' : '未配置密钥') +
          '</span><button class="ab-btn ghost sm" id="adv-mk" type="button" style="flex:0 0 auto">修改</button></div>' +
          '<div class="ab-row"><label>代码仓库</label><span class="ab-input" style="cursor:default;color:var(--text-dim);overflow:hidden;text-overflow:ellipsis;white-space:nowrap">' +
            esc(repoSummary(repo)) +
          '</span><button class="ab-btn ghost sm" id="adv-repo" type="button" style="flex:0 0 auto">设置</button></div>' +
          '<div style="margin:14px 0 6px;color:var(--text);font-size:14px">执行前</div>' +
          // ★ 2026-09-19（老板：「两个入口合并成一个」）：这里从**下拉**改成「值 + 修改」——
          //   选项定义只有一份（APPROVAL_OPTS）、写路径只有一条（applyApprovalChoice），
          //   跟**对话上方那个「审批」标签**完全是同一个设置；点「修改」就地展开三档
          //   （不换层：`openLayer` 是单层的，弹层会把高级设置顶掉 —— 客户会莫名其妙被弹出去）。
          '<div class="ab-row"><label>审批方式</label><span class="ab-input" id="adv-approval-val" style="cursor:default;color:var(--text-dim)">' +
            esc(approvalTextOf(am)) +
          '</span><button class="ab-btn ghost sm" id="adv-approval" type="button" style="flex:0 0 auto">修改</button></div>' +
          '<div id="adv-approval-list" style="display:none;margin:-4px 0 10px 78px"></div>' +
          '<div class="ab-tip" style="margin:-4px 0 10px 78px">也可以直接点对话上方那排小标签里的「审批」—— 两处是同一个设置。改完当前会话立刻生效，以后新建的项目也按这个来。选「完全访问」后，AI 改文件、执行命令不再询问。</div>' +
          '<div style="margin:14px 0 6px;color:var(--text);font-size:14px">思考</div>' +
          '<div class="ab-row"><label>推理级别</label><select class="ab-input" id="adv-effort">' +
            EFFORT_OPTS.map(function (o) {
              return '<option value="' + o.v + '"' + (effort === o.v ? ' selected' : '') + '>' + esc(o.t) + '</option>';
            }).join('') +
          '</select></div>' +
          '<div class="ab-tip" style="margin:-2px 0 10px 0">AI 回答前先想多久。调高：难题更稳，但更慢、也更费额度；调低：答得快，简单任务够用。改完下一轮对话生效。</div>' +
          '<div style="margin:14px 0 6px;color:var(--text);font-size:14px">显示</div>' +
          '<label class="ab-chk"><input type="checkbox" id="adv-think"' + (cfg.show_thinking ? ' checked' : '') + '> 显示思考过程</label>' +
          '<label class="ab-chk"><input type="checkbox" id="adv-think-exp"' + (cfg.thinking_default_expanded ? ' checked' : '') + '> 默认展开思考过程</label>' +
          '<label class="ab-chk"><input type="checkbox" id="adv-tools"' + (cfg.show_tool_details ? ' checked' : '') + '> 显示文件与命令明细</label>' +
          '<label class="ab-chk"><input type="checkbox" id="adv-calm"' + (cfg.calm_mode ? ' checked' : '') + '> 安静模式</label>' +
          // 2026-09-22：这个开关以前**完全无效**（档案 §8.7 201：值写进了引擎，网页零消费方）。
          //   照官方语义（`tui/history.rs:469-480` 的 match 顺序）修好后，它**只在「显示文件与命令明细」开着时**起作用
          //   —— 不把真实条件写给客户，客户在默认档下勾它仍会觉得「没用」。
          '<div class="ab-tip" style="margin:-2px 0 10px 0">开了「显示文件与命令明细」后，工具卡也只露前几行、不铺满屏幕。想看全就点卡上的「查看回执」。</div>' +
          '<label class="ab-chk"><input type="checkbox" id="adv-think-bg"' + (cfg.thinking_highlight !== false ? ' checked' : '') + '> 思考内容加底色</label>' +
          '<div class="ab-row"><label>文件改动显示</label><select class="ab-input" id="adv-diffs">' +
            '<option value="full"' + (diffsMode === 'full' ? ' selected' : '') + '>完整改动（红绿对照）</option>' +
            '<option value="summary"' + (diffsMode === 'summary' ? ' selected' : '') + '>只显示行数统计</option>' +
            '<option value="off"' + (diffsMode === 'off' ? ' selected' : '') + '>不显示</option>' +
          '</select></div>' +
          (typeof cfg.thinking_preview_lines === 'number'
            ? '<div class="ab-row"><label>思考预览</label><select class="ab-input" id="adv-think-lines">' +
                [0, 1, 2, 3, 5, 10].map(function (n) {
                  return '<option value="' + n + '"' + (thinkLines === n ? ' selected' : '') + '>' +
                    (n === 0 ? '不显示（只看标题）' : n + ' 行') + '</option>';
                }).join('') +
              '</select></div>'
            // 引擎还没把 thinking_preview_lines 暴露出来时，这个控件**不出现** ——
            //   否则客户一改就会撞到引擎的 400（「未知配置键」）。引擎更新后自动出现。
            : '') +
          '<div class="ab-row"><label>AI 回复语言</label><select class="ab-input" id="adv-locale">' +
            LOCALE_OPTS.map(function (o) {
              return '<option value="' + o.v + '"' + (loc === o.v ? ' selected' : '') + '>' + esc(o.t) + '</option>';
            }).join('') +
          '</select></div>' +
          '<div class="ab-tip" style="margin:-2px 0 10px 0">AI 回复与思考用哪种语言。简体中文与 English 是逐字校准过的；其余语言由模型自己拿捧（引擎只对简体中文 / 日本語 / Português / Tiếng Việt 有官方强化）。</div>' +
          '<div class="ab-row"><label>状态行显示</label><span class="ab-input" id="adv-statusline-val" style="cursor:default;color:var(--text-dim)">' +
            esc(statuslineSummary()) +
          '</span><button class="ab-btn ghost sm" id="adv-statusline" type="button" style="flex:0 0 auto">修改</button></div>' +
          '<div id="adv-statusline-list" style="display:none;margin:-4px 0 10px 78px"></div>' +
          '<div class="ab-tip" style="margin:-4px 0 10px 78px">底部状态行显示哪几项。窗口窄了不够摆时，没关掉的也会按重要性自动少显示几个（最先让出的是输出速度）——「模型」与「记性」两项永不丢。</div>' +
          '<div class="ab-tip" style="margin:-2px 0 10px 78px">这几项都是官方本来就有的设置（页面上文字仍为中文）。</div>' +
          '<label class="ab-chk"><input type="checkbox" id="adv-compact"' + (cfg.auto_compact ? ' checked' : '') + '> 聊天太长时自动帮我整理前面</label>' +
          '<div class="ab-tip" style="margin:2px 0 10px 0">自动整理会总结前面的内容 —— 部分细节会丢失（默认开启）。<br>⚠️ 关闭后请留意：长对话可能因超出模型上下文而中断。</div>' +
          '<div class="ab-row"><label>货币单位</label><select class="ab-input" id="adv-currency">' +
            '<option value="cny"' + (cur === 'cny' ? ' selected' : '') + '>人民币 ￥</option>' +
            '<option value="usd"' + (cur === 'usd' ? ' selected' : '') + '>美元 $</option>' +
          '</select></div>' +
          '<label class="ab-chk"><input type="checkbox" id="adv-ro"' + (d.previewReadOnly ? ' checked' : '') + '> 只读模式</label>' +
          // 2026-09-22：扫审计时抓到的语义错位（档案 §8.7 201）—— 这是**项目级**的
          //   （`projects.json` 的 `previewReadOnly`，见 server.js:3140），却跟一堆账号级项并排、
          //   头顶还写着「应用到你的所有项目」⇒ 客户会以为改一处就全局生效。如实写清楚。
          '<div class="ab-tip" style="margin:-2px 0 10px 0">只对<b>当前这个项目</b>生效（其余设置才是全部项目）：打开后，打开它的预览页只能看、不能操作。适合看正在跑的真实系统。</div>' +
          '<div style="margin:14px 0 6px;color:var(--text);font-size:14px">用量统计</div>' +
          (u
            ? '<div class="ab-card"><div class="ab-s">累计 ' + (DISPLAY.cost_currency === 'cny'
                ? '￥' + Number(u.costCny || 0).toFixed(2)
                : '$' + Number(u.costUsd || 0).toFixed(2)) + ' ｜ 改动 ' + (u.turns || 0) + ' 次<br>进 ' + Math.round((u.inTok || 0) / 1000) + 'K / 出 ' + Math.round((u.outTok || 0) / 1000) + 'K token</div></div>'
            : '<div class="ab-tip">这个项目还没有用量记录。</div>') +
          '<div class="ab-msg" id="adv-msg"></div>';
        var msgEl = el.querySelector('#adv-msg');
        abTipPanel(el, 'adv-approval', 'AI 执行操作前是否询问，可在此处设置。');
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
            // 审批档还牵着「当前这条会话」（引擎里是 permission_posture + auto_approve）：
            // 不同步的话，对话框上面那个「审批」小标签会一直显示旧值
            // —— 2026-09-17 老板就是照这个发现的（设了「小的自己做」还显示「每次询问」）。
            if (key === 'approval_mode') applyApprovalToThread(value);
            return true;
          });
        }
        /* 存成功后**当场**让网页版跟着变 —— 否则客户得刷新才看得到效果 */
        function syncDisplayPref(key, v) {
          if (key === 'cost_currency') DISPLAY.cost_currency = (v === 'cny' ? 'cny' : 'usd');
          else if (key === 'show_thinking' || key === 'thinking_default_expanded' || key === 'show_tool_details' || key === 'calm_mode' || key === 'thinking_highlight') {
            DISPLAY[key] = (v === 'true' || v === true);
            // 客户**自己**改「默认展开思考」→ 清掉「见过」的记账，让当前这批卡重新按新设置摆一次
            //   （否则刚被客户收起的那些卡，改了设置也还是收着的）。其余显示键不受影响。
            if (key === 'thinking_default_expanded') reasoningSeen = (typeof WeakSet === 'function') ? new WeakSet() : null;
          } else if (key === 'inline_diffs') {
            DISPLAY.inline_diffs = (v === 'summary' || v === 'off') ? v : 'full';
          } else if (key === 'thinking_preview_lines') {
            DISPLAY.thinking_preview_lines = Number(v) || 0;
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
        // 审批方式：不再自带一套下拉 —— 选项定义（APPROVAL_OPTS）与写路径（applyApprovalChoice）
        // 跟对话上方那个标签**完全是同一份**；这里只是就地把那三档展开（不换层）。
        var advApprovalBtn = el.querySelector('#adv-approval');
        var advApprovalList = el.querySelector('#adv-approval-list');
        if (advApprovalBtn && advApprovalList) advApprovalBtn.onclick = function () {
          if (advApprovalList.style.display !== 'none') { advApprovalList.style.display = 'none'; return; }
          advApprovalList.style.display = '';
          advApprovalList.innerHTML = APPROVAL_OPTS.map(function (o) {
            return '<button class="ab-menu-item" data-m="' + o.m + '">' + o.t + '<small>' + o.d + '</small></button>';
          }).join('');
          advApprovalList.querySelectorAll('button[data-m]').forEach(function (b) {
            b.onclick = function () {
              var o = APPROVAL_OPTS.filter(function (x) { return x.m === b.getAttribute('data-m'); })[0];
              if (!o) return;
              b.disabled = true;
              applyApprovalChoice(o).then(function (rs) {
                b.disabled = false;
                if (rs[0] && !rs[0].ok) { msg(el.querySelector('#adv-msg'), (rs[0].body && rs[0].body.error) || '没存下来', false); return; }
                var v = el.querySelector('#adv-approval-val');
                if (v) v.textContent = o.t;
                advApprovalList.style.display = 'none';
                paintFact('permission', o.t);
                msg(el.querySelector('#adv-msg'), '已改为「' + o.t + '」', true);
              }).catch(function (e) {
                b.disabled = false;
                msg(el.querySelector('#adv-msg'), '没改成功：' + ((e && e.message) || e), false);
              });
            };
          });
        };
        // 状态行显示哪几项（2026-09-20 · 照官方 `/statusline` 的多选；落 localStorage，不走引擎）
        var advStBtn = el.querySelector('#adv-statusline');
        var advStList = el.querySelector('#adv-statusline-list');
        if (advStBtn && advStList) {
          advStBtn.onclick = function () {
            if (advStList.style.display !== 'none') { advStList.style.display = 'none'; return; }
            advStList.style.display = 'block';
            var off = statuslineOff();
            advStList.innerHTML = STATUSLINE_SEGS.map(function (s) {
              return '<label class="ab-chk"><input type="checkbox" data-seg="' + s.k + '"' +
                (off.indexOf(s.k) < 0 ? ' checked' : '') + '> ' + esc(s.label) + '</label>';
            }).join('');
            advStList.querySelectorAll('input[data-seg]').forEach(function (box) {
              box.onchange = function () {
                var next = [];
                advStList.querySelectorAll('input[data-seg]').forEach(function (b) {
                  if (!b.checked) next.push(b.dataset.seg);
                });
                setStatuslineOff(next).then(function (good) {
                  var valEl = el.querySelector('#adv-statusline-val');
                  if (valEl) valEl.textContent = statuslineSummary();
                  // ⚠️ **不能直接叫 `metricsRender()`** —— 它长在「会话指标」那个**嵌套作用域**里，
                  //   而这段代码在顶层（跟 `statuslineSummary` 当初那个坑同源，实测踩过：
                  //   点了复选框、localStorage 变了、状态行纹丝不动）。
                  //   用事件把「该重画了」传回去（嵌套那侧监听）。
                  try { window.dispatchEvent(new CustomEvent('asbudy:statusline-change')); } catch (e1) { /* 没 window 就算了 */ }
                  // 存不成就实话实说（以前无条件报「已更新」—— 发到服务端失败也报成功就骗人了）
                  msg(el.querySelector('#adv-msg'), good ? '已更新状态行' : '没存下来：状态行设置没保存', good);
                });
              };
            });
          };
        }

        bindSel('adv-diffs', 'inline_diffs');
        bindSel('adv-locale', 'locale');
        bindSel('adv-effort', 'reasoning_effort');
        bindSel('adv-think-lines', 'thinking_preview_lines');
        bindChk('adv-think-bg', 'thinking_highlight');
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
          esc(label) + (p.ready ? '（已配置）' : '') + '</option>';
      }).join('');
      body.innerHTML =
        '<div class="ab-tip">使用你自己的模型服务：选择服务商、填写密钥。改一次，你名下所有项目都生效（以后新建的也自动带上）；留空项将保持不变（端点 / 模型名 / 密钥）。</div>' +
        (d.helper ? '' : '<div class="ab-tip" style="color:var(--human)">⚠️ 服务端还没装「模型密钥」帮手，现在保存不了 —— 让管理员跑一下安装脚本。</div>') +
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
        if (!confirm('把「' + nm + '」的配置（密钥 / 端点 / 模型名）全删掉？\n你名下所有项目都会跟着清掉；如果当前用的就是它，会自动回退到 DeepSeek。')) return;
        msg(msgEl, '正在清除…', true);
        api('/_gate/model-key', { method: 'POST', body: JSON.stringify({ provider: pid, clear: true }) }).then(function (r) {
          var b = r.body || {};
          if (!r.ok || b.ok === false) {
            msg(msgEl, (b.error || '清除失败') + '（若提示帮手是旧版，让管理员重装一下）', false);
            return;
          }
          body.querySelector('#mk-key').value = '';
          msg(msgEl, '已清除「' + nm + '」的配置（你名下所有项目）', true);
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
          // 2026-09-17：改成「按人」后，反馈要说清楚写到哪些项目了（不再是单个项目的结果）
          var applied = b.applied || [], skipped = b.skipped || [], failed = b.failed || [];
          var line = '已存进你的账号';
          line += applied.length ? ('，写到了 ' + applied.length + ' 个项目') : '，当前没有在运行的项目要写';
          if (skipped.length) line += '；' + skipped.length + ' 个没在运行，进去时会自动补上';
          if (failed.length) line += '；' + failed.length + ' 个没写进去（' + ((failed[0] || {}).error || '原因不明') + '）';
          msg(msgEl, line, failed.length === 0);
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
    if (!r) return '未连接（代码当前仅存于平台）';
    if (!r.remote) return r.helper === false ? '未连接（平台组件未就绪）' : '未连接（代码当前仅存于平台）';
    return String(r.remote).replace(/^[a-z]+:\/\//, '').replace(/^[^@/]*@/, '');
  }
  function repoWhen(iso) {
    var d = new Date(iso);
    if (isNaN(d.getTime())) return '';
    function p(n) { return (n < 10 ? '0' : '') + n; }
    return p(d.getMonth() + 1) + '-' + p(d.getDate()) + ' ' + p(d.getHours()) + ':' + p(d.getMinutes());
  }
  function repoStateLine(d) {
    if (!d.initialized) return '此项目尚无版本库 —— 保存地址时会自动创建并提交当前文件。';
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
      var html = '<div class="ab-tip">将项目代码连接到你自己的仓库，代码将不再仅存于平台。' +
        '连接后需明确指示「推送到我的仓库」才会推送。</div>';
      if (noHelper) {
        html += '<div class="ab-tip" style="color:var(--human)">⚠️ 服务端还没装「代码仓库」帮手，现在存不了 —— 让管理员跑一下安装脚本。</div>';
      }

      html += '<div class="ab-row"><label>用哪家</label><select class="ab-input" id="rp-platform">' +
        [['gitee', 'Gitee（码云）'], ['github', 'GitHub'], ['other', '其他 / 自己搭的']].map(function (x) {
          return '<option value="' + x[0] + '"' + (x[0] === plat ? ' selected' : '') + '>' + x[1] + '</option>';
        }).join('') + '</select></div>';
      html += '<div class="ab-row"><label>仓库地址</label><input class="ab-input" id="rp-remote" placeholder="git@gitee.com:你的账号/仓库.git" value="' + esc(d.remote || '') + '"></div>';
      html += '<div class="ab-tip" style="margin:-4px 0 10px 78px">在仓库页面复制 <b>SSH</b> 地址并粘贴到这里（也可使用 https:// 地址）。' +
        '请使用你自己的<b>私有</b>仓库 —— 平台不托管代码，也不会推送到其他位置。</div>';
      html += '<div style="display:flex;gap:8px;margin:-2px 0 6px 78px">' +
        '<button class="ab-btn sm" id="rp-save" type="button"' + (noHelper ? ' disabled' : '') + '>保存地址</button>' +
        '<button class="ab-btn ghost sm" id="rp-clear" type="button"' + (noHelper ? ' disabled' : '') + '>移除地址</button></div>';

      html += '<div style="margin:14px 0 6px;color:var(--text);font-size:14px">认证方式</div>';
      html += '<div class="ab-row"><label>方式</label><select class="ab-input" id="rp-auth">' +
        '<option value="deploy-key"' + (auth === 'deploy-key' ? ' selected' : '') + '>部署密钥（推荐）</option>' +
        '<option value="token"' + (auth === 'token' ? ' selected' : '') + '>访问令牌（https 地址用）</option>' +
        '</select></div>';

      var keyBox = '';
      if (d.hasKey && pub) {
        keyBox += '<label style="color:var(--text-dim);font-size:13.5px">公钥（粘贴到仓库）</label>' +
          '<textarea class="ab-input" id="rp-pub" readonly style="height:70px;font-family:ui-monospace,Menlo,Consolas,monospace;font-size:12.5px;margin:4px 0;resize:vertical">' + esc(pub) + '</textarea>' +
          '<div style="display:flex;gap:8px;margin-bottom:8px"><button class="ab-btn sm" id="rp-copy" type="button">复制公钥</button>' +
          '<button class="ab-btn danger sm" id="rp-delkey" type="button" style="margin-left:auto">删掉密钥</button></div>';
        if (d.fingerprint) keyBox += '<div class="ab-s" style="margin-bottom:6px">指纹 ' + esc(d.fingerprint) + '</div>';
        keyBox += '<div class="ab-tip">尚未添加？请在仓库「<b>设置 → 部署公钥（Deploy Keys）</b>」中添加，并勾选「允许写入」。' +
          '未勾选写入权限将导致推送被拒。</div>';
      } else {
        keyBox += '<div class="ab-tip">尚无密钥。点击下方按钮生成：<b>公钥</b>粘贴到仓库，<b>私钥</b>保留在项目内。</div>' +
          '<button class="ab-btn" id="rp-genkey" type="button"' + (noHelper ? ' disabled' : '') + '>生成密钥</button>';
      }
      html += '<div id="rp-keybox">' + keyBox + '</div>';

      var tokBox = '<div class="ab-row"><label>仓库用户名</label><input class="ab-input" id="rp-user" placeholder="登录仓库的账号名"></div>' +
        '<div class="ab-row"><label>访问令牌</label><input class="ab-input" id="rp-token" type="password" autocomplete="new-password" placeholder="' +
        (d.hasToken ? '已保存（如需更换请填写新值）' : '在仓库设置中生成') + '"></div>' +
        '<div style="display:flex;gap:8px"><button class="ab-btn sm" id="rp-savetoken" type="button"' + (noHelper ? ' disabled' : '') + '>保存令牌</button>' +
        (d.hasToken ? '<button class="ab-btn danger sm" id="rp-cleartoken" type="button" style="margin-left:auto">清除令牌</button>' : '') + '</div>' +
        '<div class="ab-tip" style="margin-top:8px">令牌仅存储于本项目内，不会写入对话，也不会回显。</div>';
      html += '<div id="rp-tokenbox"' + (auth === 'token' ? '' : ' hidden') + '>' + tokBox + '</div>';

      html += '<div style="margin:14px 0 6px;color:var(--text);font-size:14px">现在怎么样</div>';
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
        if (!confirm('移除仓库地址？代码将仅存于平台（本地版本库不受影响）。')) return;
        post({ action: 'clear', platform: body.querySelector('#rp-platform').value });
        syncState();
      };
      var bg = body.querySelector('#rp-genkey');
      if (bg) bg.onclick = function () { post({ action: 'genkey' }, '密钥生成好了 —— 复制公钥贴到你的仓库'); };
      var bc = body.querySelector('#rp-copy');
      if (bc) bc.onclick = function () {
        var ta = body.querySelector('#rp-pub');
        ta.select();
        var done = function () { msg(msgEl, '公钥已复制，请粘贴到仓库', true); };
        if (navigator.clipboard) navigator.clipboard.writeText(ta.value).then(done, function () { msg(msgEl, '复制失败，请手动选取复制', false); });
        else { try { document.execCommand('copy'); done(); } catch (e) { msg(msgEl, '复制失败，请手动选取复制', false); } }
      };
      var bd = body.querySelector('#rp-delkey');
      if (bd) bd.onclick = function () {
        if (!confirm('删除密钥？删除后需重新生成并粘贴到仓库才能推送。')) return;
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
          '<span class="ab-input" style="cursor:default;color:var(--text);overflow:hidden;text-overflow:ellipsis;white-space:nowrap">' + esc(v) + '</span></div>';
      }
      var html = '<div class="ab-tip">这是你的登录账号；「名字」仅用于显示。</div>';
      html += row('名字', me.name || me.user || '—');
      html += row('登录账号', me.user || '—');
      html += row('角色', roleTxt);
      if (me.role === 'staff') html += row('归属', me.ownerName ? ('归「' + me.ownerName + '」管') : '平台直接管');
      if (typeof me.quota === 'number') html += row('能建几个项目', me.quota + ' 个');
      if (me.viewAs) {
        html += '<div class="ab-tip" style="color:var(--human);border-color:rgba(246,196,83,.33)">' +
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
          else msg(msgEl, j.error || '改不了', false);
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
    /* ── 两个钟：本轮（「处理中 · 已用 N」）＋ 这个对话累计（官方那个「已运行 N」）─────
     * 【官方语义（照 `tui/tui/phase_strip.rs:1100` 的 `working_clock()`，别自己发明）】
     *   · turn 读数 = `{阶段词} {时长}` —— 答「正在干什么、多久了」；
     *   · session 读数 = `App::cumulative_turn_duration`（**已完成轮次时长之和**）＋当前这一轮；
     *     注释原文 *"It is model work, not wall clock since launch — an idle TUI does not
     *     claim to have been working"* ⇒ **不是「挂着多久」，是这个对话真干了多久**；
     *   · 门槛 `CLOCK_SESSION_FLOOR_SECS = 60`（不足 1 分钟不显示）·
     *     与 turn 读数相同时也不显示（会话第一轮，`#6041`）· 空闲时用暗色（钟停了）。
     *   · 文案照官方语言包 `locales/zh-Hans.json:1229` —— 「已运行{duration}」。
     * 【数据从哪来】`GET /v1/threads/{id}` 的 turns（每轮 `duration_ms`）＝已完成轮次之和；
     *   正在跑的那一轮用本地秒表补（就是原来那个「已用 N 秒」的钟）。
     * 【为什么合并】§12 M15：我们**已有**这个 tick，做这条时要合并，别摆出两个钟。
     */
    var doneMs = 0;        // 已完成轮次之和（毫秒）
    var doneFor = '';      // doneMs 读的是哪个会话
    var seenFor = '';      // 上次 paint 时看到的会话（用于惰性跟上切会话）

    function loadDone() {
      var tid = MODEL_THREAD;
      if (!tid) return;
      if (tid === doneFor) return;
      doneFor = tid;
      doneMs = 0;
      api('/v1/threads/' + encodeURIComponent(tid)).then(function (r) {
        var body = (r && r.body) || {};
        var turns = body.turns || [];
        var sum = 0;
        turns.forEach(function (t) {
          var ms = Number((t && t.duration_ms) || 0);
          // 兜底：没有 duration_ms 就用 started_at / ended_at 算
          if (!ms && t && t.started_at && t.ended_at) {
            ms = Math.max(0, new Date(t.ended_at) - new Date(t.started_at));
          }
          sum += ms;
        });
        doneMs = sum;
        paint();
      }).catch(function () { /* 读不到就先只显示本轮钟，不报错 */ });
    }
    function workedSec() {
      return Math.round((doneMs + (startedAt ? Date.now() - startedAt : 0)) / 1000);
    }
    function paint() {
      var el = ensureEl();
      if (!el) return;
      if (MODEL_THREAD !== seenFor) { seenFor = MODEL_THREAD; if (MODEL_THREAD !== doneFor) loadDone(); }
      var parts = [];
      var turnSec = startedAt ? Math.round((Date.now() - startedAt) / 1000) : 0;
      if (startedAt) parts.push('⏱ 处理中 · 已用 ' + fmt(turnSec));
      var w = workedSec();
      // 官方两条门槛：不足 60 秒不显示；与 turn 读数相同（第一轮）也不显示
      if (w >= 60 && !(startedAt && w === turnSec)) parts.push('已运行 ' + fmt(w));
      el.textContent = parts.join('　·　');
      el.hidden = parts.length === 0;
      // 空闲时暗一点（官方：钟停了；颜色降一档，但不隐藏）
      el.style.opacity = startedAt ? '' : '.7';
    }
    function start() {
      startedAt = Date.now();
      var el = ensureEl();
      if (el) el.hidden = false;
      loadDone();
      paint();
      if (tick) clearInterval(tick);
      tick = setInterval(paint, 500);
    }
    function stop() {
      if (startedAt) { doneMs += Date.now() - startedAt; startedAt = 0; }
      if (tick) { clearInterval(tick); tick = null; }
      // 轮刚结束 → 重新跟引擎对一次账（拿真值盖本地累加，避免长会话漂）
      doneFor = '';
      paint();
    }
    // 空闲时也低频跟一下：切会话 / 页面刚打开（那时 MODEL_THREAD 才被猴补 fetch 捕获）
    setInterval(function () { if (!startedAt) paint(); }, 3000);
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
        b.title = '收起侧栏';
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
      b1.title = '让 AI 重新作答（不影响项目文件）';
      var b2 = document.createElement('button');
      b2.type = 'button';
      b2.id = 'asbudy-undo-turn';   // 注意：不能叫 asbudy-undo —— 那是退回面板容器的 id（重复 id 会让 getElementById 拿到错的）
      b2.textContent = '↩ 撤销';
      b2.title = '移除最后一轮问答，提问将回到输入框（不影响项目文件）';
      var b3 = document.createElement('button');
      b3.type = 'button';
      b3.id = 'asbudy-compact';
      b3.textContent = '🗜 压缩';
      b3.title = '压缩当前对话以节省上下文（保留要点）';
      el.appendChild(b1);
      el.appendChild(b2);
      el.appendChild(b3);
      var ctxEl = document.createElement('span');
      ctxEl.id = 'asbudy-ctx';
      ctxEl.setAttribute('aria-live', 'polite');
      el.appendChild(ctxEl);
      // 会话指标：跟「记性」同排、放在它后面（它是有数据才出现的读数，不占固定位置）
      var metricsEl = document.createElement('span');
      metricsEl.id = 'asbudy-metrics';
      metricsEl.hidden = true;
      metricsEl.setAttribute('aria-live', 'polite');
      el.appendChild(metricsEl);
      try { metricsRender(metricsEl); } catch (e0) { /* 首次渲染失败不影响对话 */ }
      // 「正在做什么」状态行：放在最左 —— 先看到「它现在在干什么」，再看到别的
      var liveEl = document.createElement('span');
      liveEl.id = 'asbudy-live';
      liveEl.hidden = true;
      liveEl.setAttribute('aria-live', 'polite');
      el.insertBefore(liveEl, el.firstChild);
      wrap.parentNode.insertBefore(el, wrap);
      return el;
    }

    // ── 「记性 N%」：这次对话用了模型多少「记忆」（2026-09-15 · 老板要求）──
    // 为什么要门卫算：官方 CLI 状态栏有 `ctx NN%`，但那是引擎**进程内部状态**，web 拿不到；
    // 门卫复刻了引擎同一套窗口规则（按模型名查表），所以换模型会自动跟着变，不用人工设。
    // 不猜：门卫拿不到窗口或没数据时返回 available:false，这里就不显示。
    // ── 「正在做什么」实时状态（2026-09-18 老板：「不像 cli 版那样让人清楚知道进度」）──
    // CLI 的进度感来自底部那条**实时状态行**（在跑什么 / 跑了多久 / 按 Esc 能打断）；
    // web 以前只有一串「事后回执」。数据引擎早就推过来了 —— 这里接上。
    //
    // ★ 2026-09-19 老板：「我们对齐官方 cli 版就是了，不用创新」—— 词表与粒度全部改回 CLI 的：
    //   CLI 是 `LiveActivityKind`（`tui/underwater.rs:440-512` 的 `label()`），注释原文
    //   *"deliberately stays smaller than the tool taxonomy"* —— **刻意只按「族」分**：
    //     读/找类（ToolFamily::Read|Find）→ 读取中；验证类（Verify）→ 校验中；
    //     子代理 → 正在使用子代理；其余工具 → 使用工具中；思考 → 推理中；其余 → 工作中。
    //   文案取 CLI 自己的中文翻译（`localization/locales/zh-Hans.json`）：
    //     PhaseWorking=工作中 · PhaseReasoning=推理中 · PhaseReading=读取中 ·
    //     PhaseUsingTool=使用工具中 · PhaseSubagents=正在使用子代理 · PhaseVerifying=校验中 ·
    //     PhaseWaitingOnYou=等你处理 · ContextManualCompacting=正在压缩上下文…
    //   ⚠️ 别按具体工具名发明新词（我们原来那套「正在执行命令 / 正在写文件 / 正在查资料」
    //     就是自己分的 —— 也正是它跟转录里的工具卡撞车：那条报的是「具体工具 + 秒数」）。
    var LIVE = { active: false, since: 0, what: '' };
    // 族名表都收着：引擎给的名字加上 CLI 的规范名，两边都能认。
    var READ_TOOLS = ['read', 'read_file', 'ls', 'list_dir', 'glob', 'grep', 'grep_files', 'find',
      'file_search', 'cat', 'view_image', 'web', 'web_search', 'fetch', 'fetch_url', 'search',
      'registry_sync', 'git_status', 'git_diff', 'git_log', 'git_show', 'git_blame'];
    var VERIFY_TOOLS = ['run_tests', 'run_verifiers', 'task_gate_run', 'validate_data',
      'wait_for_dev_server', 'test'];
    var SUBAGENT_TOOLS = ['agent', 'subagent', 'workflow', 'fleet', 'dispatch'];
    function liveWordFor(name) {
      var n = String(name || '').toLowerCase();
      if (!n) return '工作中';
      if (VERIFY_TOOLS.indexOf(n) >= 0) return '校验中';
      if (SUBAGENT_TOOLS.indexOf(n) >= 0) return '正在使用子代理';
      if (READ_TOOLS.indexOf(n) >= 0) return '读取中';
      return '使用工具中';
    }
    /** 时长照 CLI 的 `format_elapsed_secs`（`crates/tui/src/elapsed.rs:22`）：
     *  <60s → `12s`；≥60s → `1m 15s`。格式也照 CLI：`{阶段词} {时长}`（空格，不加分隔符）。 */
    function fmtElapsed(secs) {
      secs = Math.max(0, Math.round(secs));
      if (secs < 60) return secs + 's';
      return Math.floor(secs / 60) + 'm ' + String(secs % 60).padStart(2, '0') + 's';
    }
    /** 阶段墨色 —— 照 CLI `underwater.rs:638 phase_ink()`：
     *  · Working / Verifying → `Active`（Seafoam 青绿）
     *  · Waiting / Approval  → `Waiting`（Signal Gold 黄）
     *  · Failed              → `Failure`（Rose 红）
     *  官方注释原文：*"Status-bar phase ink. Failure red is only `Failed`."*（**红只给失败**）。
     *  ⚠️ 这三个 CSS 变量与 CLI 的调色板**逐字同源**（2026-09-22 实测核对过，不是凡的）：
     *     `--status-live` #4FD1C5 = `palette/tokens.rs` 的 `WHALE_ACCENT_SECONDARY_RGB`（＝ `WHALE_LIVE`，
     *        而 `themes.rs` 里 `status_working: WHALE_LIVE`）
     *     `--status-human` #F6C453 = `WHALE_HUMAN_RGB`（＝ `accent_action`，`ChromeInk::Waiting` 用它）
     *     `--status-danger` #FF86B2 = `WHALE_ERROR_RGB`（＝ `error_fg`，`ChromeInk::Failure` 用它）
     *  ⇒ **不新造颜色**，直接用两边共用的那一套；带上 fallback 防变量缺失。 */
    function liveInk(what) {
      if (what === '失败') return 'var(--status-danger, #ff86b2)';
      if (what === '等你处理') return 'var(--status-human, #f6c453)';
      return 'var(--status-live, #4fd1c5)';
    }
    function liveTick() {
      var el = document.getElementById('asbudy-live');
      if (!el) return;
      if (!LIVE.active) { el.hidden = true; el.textContent = ''; return; }
      var secs = (Date.now() - LIVE.since) / 1000;
      el.hidden = false;
      el.textContent = LIVE.what + ' ' + fmtElapsed(secs);
      el.style.color = liveInk(LIVE.what);
    }
    function liveSet(what) {
      LIVE.active = true;
      LIVE.what = what;
      if (!LIVE.since) LIVE.since = Date.now();
      liveTick();
    }
    function liveStop(keepMs) {
      // 失败态要先露一下红才藏（CLI 的 `Failed` 是个常驻相位；网页上状态行在
      //  `turn.completed` 就要收，不过渡一下的话红只闪一帧就没了）。
      if (keepMs && keepMs > 0) {
        var mine = LIVE.what;
        setTimeout(function () {
          if (LIVE.what === mine) { LIVE.active = false; LIVE.since = 0; liveTick(); }
        }, keepMs);
        return;
      }
      LIVE.active = false;
      LIVE.since = 0;
      liveTick();
    }
    setInterval(liveTick, 1000);

    // ── 会话指标：`ttft 400ms · 38 平均 tok/s · ↓ 1.2K`（2026-09-19 搬 · 逐条照官方 CLI）──
    // 数据来源：引擎每次模型调用推的 `turn.usage`（payload 带 usage / duration_ms /
    //   first_token_ms / request_ms）。**官方前端订阅了它却没有处理分支** ——
    //   数据早就到浏览器了，只是没人接。这里接上。
    // 口径逐条照 CLI 的 `SessionMetrics`（`tui/session_metrics.rs:74` `record_model_call`）：
    //   ttft  = 本 session **报过 ttft 的那些调用**的平均值（`ttft_average`，:137）
    //   速率  = 本 session `output_tokens 总和 ÷ request_ms 总和`（`tokens_per_second`，:147）——
    //           只统计 request_ms > 0 的调用；ttft 样本为 0 / 秒数为 0 时**不显示**（绝不补 0）
    //   ↓     = **最近一次**调用的 output_tokens（`output_tokens(app)`，`ui/frame.rs:63`）
    // 显示词与格式也照官方（`tui/ui/frame.rs:268-292` ＋ zh-Hans：`ttft` / `平均 tok/s` / `↓`）。
    // ⚠️ 作用域差异（如实记着）：CLI 的累加器活在进程里，重开会话即清零；网页端对应
    //   「本次打开界面以来」—— 刷新页面即清零。历史轮次的累计值不在这一行（那看「用量」面板）。
    var METRICS = { ttftTotalMs: 0, ttftSamples: 0, rateTokens: 0, rateMs: 0, lastOutput: 0, cacheHit: 0, cacheMiss: 0 };

    /** 照 CLI `format_duration`（`session_metrics.rs:246`）：`0s` / `320ms` / `1.5s` / `11m46s` / `1h02m` */
    function fmtMetricsDur(ms) {
      ms = Math.max(0, Math.round(Number(ms) || 0));
      if (ms === 0) return '0s';
      if (ms < 1000) return ms + 'ms';
      var secs = Math.floor(ms / 1000);
      if (secs < 60) {
        var tenths = Math.floor((ms + 50) / 100);
        return Math.floor(tenths / 10) + '.' + (tenths % 10) + 's';
      }
      if (secs < 3600) return Math.floor(secs / 60) + 'm' + String(secs % 60).padStart(2, '0') + 's';
      return Math.floor(secs / 3600) + 'h' + String(Math.floor((secs % 3600) / 60)).padStart(2, '0') + 'm';
    }
    /** 照 CLI `format_tokens`（`session_metrics.rs:267`）：`842` / `12.3K` / `9.3M` / `1.2B` */
    function fmtMetricsTokens(n) {
      n = Math.max(0, Math.round(Number(n) || 0));
      var units = [[1e9, 'B'], [1e6, 'M'], [1e3, 'K']];
      for (var i = 0; i < units.length; i++) {
        var scale = units[i][0];
        if (n >= scale) {
          var scaled = n / scale;
          return (scaled >= 100 ? scaled.toFixed(0) : scaled.toFixed(1)) + units[i][1];
        }
      }
      return String(n);
    }
    /** 照 CLI `format_rate`（`session_metrics.rs:284`）：<10 保留一位小数，否则取整 */
    function fmtMetricsRate(rate) {
      return rate < 10 ? rate.toFixed(1) : rate.toFixed(0);
    }
    /* ── 段的「丢车保帅」优先级（2026-09-20 搬 · 照 CLI `InfoSegmentId::shed_priority`）──     * `infoline.rs:113-136` 的原始表：`Rate=9 > Cache=8 = Ttft=8 > OutputTokens=7 = BillingTier=7
     *   > Cost=6 > Balance=5 = Workspace=5 = GitBranch=5 > Goal=4 > Model=0 = Context=0`
     * **数值越大越先丢**；`0` 那两个**永不丢**。
     * 我们这根条上只有这 4 段（「模型」标签与「记性」在别的元素里，同属「永不丢」那一档）。 */
    var METRIC_SHED = { rate: 9, cache: 8, ttft: 8, output: 7 };

    /* ── 「状态行显示哪些段」（2026-09-20 · 照官方 CLI 的 `/statusline` 多选选择器）──
     * 官方那一项存 **`settings.toml`**（`commands/groups/config/config.rs:550`
     *   → `AppAction::OpenStatusPicker`，实现在 `ui/apply.rs:2135`）；
     * 而引擎的 `POST /v1/config` **白名单里没有状态行键** ⇒ 网页端只能落 **localStorage**
     *   （跟 tip 机制同一处；**别去写 settings.toml** —— 会撞 EACCES，见 server.js 里那条教训）。
     * ⚠️ 所以它**不跨浏览器**（换台机器/换个浏览器要重设）—— 界面上要跟客户说清楚。
     * 存的是「**关掉**的段」（默认全开 → 新搬的段自动出现，不用改存量设置）。 */
    /** 这一段放得下吗。
     *  ⚠️ 不能拿 `el.scrollWidth > el.clientWidth`：flex 会把子元素**压缩**，
     *     读到的 clientWidth 是被压之后的，等于没量。
     *  ⚠️ 判据不能写成「右边界 <= 容器右边界 - 2」—— 实测在正常宽度下指标位
     *     本来就贴着容器右边，那个写法会把**放得下**误判成放不下，一路把段丢光。
     *  两条判据：
     *    ① 它得和**前一个可见兄弟**在同一行 —— `#asbudy-msgbar` 是 `flex-wrap:wrap`，
     *       挤不下时它是**掉到第二行**而不是溢出。
     *       ⚠️ **判「同一行」不能拿 top 差固定值** —— 实测同一行里 `#asbudy-ctx`（top=67）
     *          和指标位（top=70）就差 **3px**（字号不同 + `align-items:center`）。
     *          当初写 `r.top <= ref.top + 2`，结果把**放得下**全判成放不下、
     *          一路把段丢到只剩一个（回归一下子红 10 条）。
     *          ⇒ 改用**垂直区间重叠**：指标的顶跑到前一个的底下面，才算换行。
     *    ② 别超出容器右边（flex-wrap 下一般不会，保险）。
     */
    function metricsFits(el) {
      var host = el.parentNode;
      if (!host || !host.getBoundingClientRect) return true;
      var r = el.getBoundingClientRect();
      if (!r.width) return true;                  // 还没布局（比如隐藏着）→ 不丢
      var rectOf = function (node) {
        return node && node.getBoundingClientRect ? node.getBoundingClientRect() : null;
      };
      var prev = el.previousElementSibling;
      while (prev && !(rectOf(prev) && rectOf(prev).width)) prev = prev.previousElementSibling;
      var ref = rectOf(prev) || rectOf(host);
      if (ref && r.top >= ref.bottom - 1) return false;  // ① 换行了
      var h = rectOf(host);
      return !h || r.right <= h.right + 1;               // ② 没超出容器
    }

    function metricsRender(el) {
      // ⚠️ `ensure()` 里调用时 msgbar 还没插进 DOM，`getElementById` 找不到它 ——
      //   所以允许把元素直接传进来（重建时再走 id 那条路）。
      if (!el) el = document.getElementById('asbudy-metrics');
      if (!el) return;
      // 先收集「有数据的段」（数组顺序即显示顺序；要丢时才按优先级挑）
      // 客户在「高级设置 → 状态行显示」里关掉的段，这里直接不收集
      var off = statuslineOff();
      var on = function (k) { return off.indexOf(k) < 0; };
      var segs = [];
      if (METRICS.ttftSamples > 0 && on('ttft')) {
        segs.push({ p: METRIC_SHED.ttft, t: 'ttft ' + fmtMetricsDur(Math.floor(METRICS.ttftTotalMs / METRICS.ttftSamples)) });
      }
      var secs = METRICS.rateMs / 1000;
      if (METRICS.rateTokens > 0 && secs > 0 && on('rate')) {
        segs.push({ p: METRIC_SHED.rate, t: fmtMetricsRate(METRICS.rateTokens / secs) + ' 平均 tok/s' });
      }
      if (METRICS.lastOutput > 0 && on('output')) {
        segs.push({ p: METRIC_SHED.output, t: '↓ ' + fmtMetricsTokens(METRICS.lastOutput) });
      }
      // ── `cache NN%` 提示词缓存命中率（2026-09-20 搬 · 照 CLI 的 `StatusItem::Cache`）──
      //   标签就是字面量 `cache`（`ui/frame.rs:309` 原文），跟 `ttft` 一样**不译**；
      //   百分比公式照 `session_metrics.rs:448-451`：`(hit*100 + total/2) / total` —— 四舍五入。
      //   ⚠️ **总数是 0 就不显示**（官方的判据是 `cache_total > 0`）—— 绝不补一个 0% 出来。
      var cacheTotal = METRICS.cacheHit + METRICS.cacheMiss;
      if (cacheTotal > 0 && on('cache')) {
        segs.push({ p: METRIC_SHED.cache, t: 'cache ' + Math.floor((METRICS.cacheHit * 100 + cacheTotal / 2) / cacheTotal) + '%' });
      }
      var paint = function (list) {
        el.textContent = list.map(function (s) { return s.t; }).join(' · ');
      };
      // 宽度不够就按优先级丢段，一直丢到放得下 —— **只丢这根条上的段**，
      //   「模型」与「记性」那一档（Model/Context=0）永远不在这份名单里。
      el.hidden = false;
      if (segs.length) {
        for (var guard = 0; guard < 8 && segs.length > 1; guard++) {
          paint(segs);
          if (metricsFits(el)) break;
          var worst = 0;
          for (var i = 1; i < segs.length; i++) if (segs[i].p > segs[worst].p) worst = i;
          segs.splice(worst, 1);
        }
      }
      paint(segs);
      el.hidden = segs.length === 0;
      el.title = segs.length
        ? '本次打开界面以来：ttft ＝ 平均首字延迟 · 平均 tok/s ＝ 平均输出速度 · ↓ ＝ 最近一轮输出 token · cache ＝ 前缀缓存命中率'
        : '';
    }

    // 状态行的「段开关」改了（高级设置里点的）→ 当场重画。
    // ⚠️ 必须走事件：那段 UI 在**顶层**、`metricsRender` 在这个嵌套作用域里 ——
    //   顶层直接调它会报 `not defined`（跟 `statuslineSummary` 当初那个坑同源）。
    if (typeof window !== 'undefined' && window.addEventListener) {
      window.addEventListener('asbudy:statusline-change', function () {
        try { metricsRender(); } catch (e) { /* 刷不动就算了 */ }
      });
    }

    // 窗口宽度变了要重算丢哪些段 —— 否则拖窄了不丢、拖宽了不回来。
    // 防抖 150ms（拖动过程中每帧都量一次会白算）。
    if (typeof window !== 'undefined' && window.addEventListener) {
      var metricsResizeTimer = null;
      window.addEventListener('resize', function () {
        if (metricsResizeTimer) clearTimeout(metricsResizeTimer);
        metricsResizeTimer = setTimeout(function () { try { metricsRender(); } catch (e) { /* 量不到就算了 */ } }, 150);
      });
    }
    /** 折进一条 `turn.usage` —— 与 CLI `record_model_call` 同一套加减法 */
    function metricsFold(p) {
      var u = (p && p.usage) || {};
      var out = Number(u.output_tokens) || 0;
      var ttft = (p && p.first_token_ms !== null && p.first_token_ms !== undefined)
        ? Number(p.first_token_ms) : null;
      if (ttft !== null && isFinite(ttft) && ttft >= 0) {
        METRICS.ttftTotalMs += ttft;
        METRICS.ttftSamples += 1;
      }
      var reqMs = (p && p.request_ms !== null && p.request_ms !== undefined)
        ? Number(p.request_ms) : null;
      if (reqMs !== null && isFinite(reqMs) && reqMs > 0) {
        METRICS.rateTokens += out;
        METRICS.rateMs += reqMs;
      }
      if (out > 0) METRICS.lastOutput = out;
      // 前缀缓存命中/未命中 —— 照官方口径累加（`displayed_total_cache_hit_tokens` 那两个）。
      //   ⚠️ 未命中**没上报**时，照官方脚注推算：`输入 − 命中`
      //   （zh-Hans 的 `CmdCacheFootnote`：「当提供商未单独上报未命中时，由『输入 − 命中』推算」）。
      //   命中本身没上报 ⇒ 这一轮**整个跳过**（不能拿 0 当命中，那会把命中率砸低）。
      var hitN = Number(u.prompt_cache_hit_tokens);
      if (isFinite(hitN) && hitN >= 0) {
        var missN = Number(u.prompt_cache_miss_tokens);
        if (!isFinite(missN)) {
          var inN = Number(u.input_tokens);
          missN = Math.max(0, (isFinite(inN) ? inN : hitN) - hitN);
        }
        METRICS.cacheHit += hitN;
        METRICS.cacheMiss += Math.max(0, missN);
      }
      metricsRender();
    }

    window.addEventListener('asbudy:activity', function (e) {
      var d = (e && e.detail) || {};
      var ev = d.event;
      var p = d.payload || {};
      if (ev === 'turn.started') { LIVE.since = Date.now(); liveSet('工作中'); return; }
      if (ev === 'turn.completed') {
        // 照 CLI 的 `ShellPhase::Failed`（`underwater.rs:427`）—— 整轮失败是**独立相位**，用量红表示。
        //   ⚠️ 不看 `item.failed`（那是**单个工具**失败：卡上自己会变红，不该把整条状态行也染红）。
        //   turn 的状态在 payload.turn.status（`completed` / `failed` / `cancelled`）。
        var tst = (p.turn && p.turn.status) || 'completed';
        if (tst === 'failed') { liveSet('失败'); liveStop(3000); return; }
        liveStop();
        return;
      }
      if (ev === 'item.started') {
        var toolName = (p.tool && p.tool.name) || '';
        var kind = (p.item && p.item.kind) || '';
        if (toolName) liveSet(liveWordFor(toolName));
        else if (kind === 'agent_reasoning') liveSet('推理中');
        else if (kind === 'context_compaction') liveSet('正在压缩上下文…');
        else liveSet('工作中');            // CLI：写回复也算 Working，不另立一个词
        return;
      }
      if (ev === 'tool_call.requested') {
        // 待确认的动态工具调用（等客户点）——CLI 的 `PhaseWaitingOnYou`（中文「等你处理」）
        liveSet('等你处理');
        return;
      }
      if (ev === 'tool_call.resolved') { if (LIVE.active) liveSet('使用工具中'); }
      if (ev === 'turn.usage') metricsFold(p);
    });

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
        el.style.color = hot ? 'var(--danger)' : 'var(--text-dim)';
        el.title = title;
        el.style.cursor = 'help';
      }
      if (!LAST_THREAD) {
        show('记性 —', '尚未开始对话 —— 发送消息后显示上下文占用');
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
          + '\n按当前对话内容估算'      // 口径：引擎按「现在要发给模型的消息」估的，不是计费数字
          + (hot ? '\n接近上限 —— 建议新建对话' : ''), hot);
      } catch (e) { show('记性 —', '暂时读不到'); }
    }
    setInterval(loadCtx, 15000);
    setTimeout(loadCtx, 3000);

    async function fire(kind) {
      if (!LAST_THREAD) { alert('请先发送一条消息'); return; }
      var labels = { retry: '重试', undo: '撤销', compact: '压缩' };
      // 退文件的地方随形态变：桌面形态下左侧那个「退回」面板被收起来了，得说桌面上的路
      var whereUndo = document.body.classList.contains('asb-desk')
        ? '要退文件，回桌面右键这个项目的图标选「退回」'
        : '要退文件，用左侧的「退回」';
      if (kind === 'undo' && !confirm('撤销最后这一轮？\n\n这一问一答会从对话里去掉，你那句话回到输入框（可以改了再发）。\n项目里的文件不受影响 —— ' + whereUndo + '。')) return;
      if (kind === 'compact' && !confirm('压缩当前对话？\n\n将保留要点，超长历史会被总结 —— 可节省上下文，但部分细节会丢失。')) return;
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
    // 官方界面切会话 / 切项目时会**整块重建 composer** —— 插在它前面这条（含「记性」）
    // 会被一起抹掉。而这段原来只在初始化时建一次 → 从此再也不回来（2026-09-18 老板报
    // 「有时显示有时不显示」）。改成盯着 DOM：不见了就补回来。
    // ⚠️ 只观察 childList（不看 characterData）：流式输出改的是文本节点，不触发；
    //    真触发时也只做一次 getElementById，再节流 300ms 才重建。
    if (window.MutationObserver && document.body) {
      var pending = false;
      new MutationObserver(function () {
        if (pending || document.getElementById('asbudy-msgbar')) return;
        pending = true;
        setTimeout(function () {
          pending = false;
          try { if (ensure()) { loadCtx(); metricsRender(); } } catch (e0) { /* 重建失败不影响对话 */ }
        }, 300);
      }).observe(document.body, { childList: true, subtree: true });
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
      'align-items:center;gap:10px;background:var(--surface);border:1px solid var(--line);border-radius:10px;padding:9px 12px;' +
      'box-shadow:0 8px 24px rgba(0,0,0,.5);color:var(--text-soft);font-size:13.5px;max-width:88vw;transition:opacity .3s}' +
      '#asbudy-tip b{color:var(--text)}#asbudy-tip button{background:none;border:0;color:var(--text-dim);cursor:pointer;font-size:13px;padding:0 2px}';
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
    d.style.cssText = 'border:1px solid rgba(106,174,242,.4);background:var(--action-soft);border-radius:8px;padding:8px 11px;margin:0 0 12px';
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
    '.ck-wrap{border:1px solid var(--line);background:var(--surface);border-radius:10px;padding:10px 12px}',
    '.ck-hd{display:flex;align-items:center;gap:9px;margin-bottom:8px}',
    '.ck-hd b{color:var(--text);font-size:13.5px;flex:none}',
    '.ck-bar{flex:1;height:5px;border-radius:3px;background:var(--line);overflow:hidden;display:block}',
    '.ck-bar i{display:block;height:100%;background:var(--live);transition:width .25s}',
    '.ck-x{background:none;border:0;color:var(--text-dim);font-size:12.5px;cursor:pointer;flex:none}',
    '.ck-x:hover{color:var(--text)}',
    '.ck-row{display:flex;align-items:center;gap:9px;width:100%;text-align:left;background:none;border:0;',
    'border-top:1px solid var(--surface-raised);padding:8px 0;cursor:pointer;color:var(--text-soft);font:inherit;font-size:13.5px}',
    '.ck-row:first-of-type{border-top:0}',
    '.ck-row:hover{color:var(--text)}',
    '.ck-box{flex:none;width:17px;height:17px;border:1px solid var(--line);border-radius:5px;font-size:12px;',
    'line-height:15px;text-align:center;color:var(--live)}',
    '.ck-row.on .ck-box{border-color:var(--live);background:var(--live-wash)}',
    '.ck-txt{flex:1}.ck-txt small{display:block;color:var(--text-dim);font-size:12.5px}',
    '.ck-go{color:var(--text-dim)}',
    '.ck-done{border-color:rgba(79,209,197,.4);background:var(--live-wash);color:var(--live);font-size:13.5px}',
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

  /* ── 矮可视视口（手机键盘弹出）：别让非对话行把对话区挤没 ──────────────────
   * 【2026-09-19 老板报】「AI 回复最后内容下方有一块 CSS 空白，手机点输入框后像是它挡住了输入框」。
   * 实测（390×844 仿真手机 · headless Chromium）：
   *   · 那块空白 = 官方两条样式叠出来的 68px —— `.transcript` 手机端 padding-bottom 44px
   *     （`styles.css` 媒体查询 `padding: 26px 16px 44px`）＋ 最后一条 `article` 的 margin-bottom 24px。
   *     正常态这是消息的呼吸感，**本身不是 bug**。
   *   · 真问题在键盘弹出：`.session` 是 5 行网格，可视高掉到 560px 时实测行高
   *     95(header) / 32(状态行) / **77(对话区)** / **212(上手卡)** / 144(输入区) —— 非对话行**一个都不缩**
   *     ⇒ 对话区自己上下内边距就要 26+44=70px ⇒ **内容可视 7px**；掉到 480px 时对话区 0px、
   *     AI 最后一行文字实测**可见 0 像素**，屏幕上剩下的正是那片空白。
   *   · 对照：把上手卡拿掉 → 同一视口对话区回到 209px（内容可视 139px）。
   * 所以这里**只做一件事**：视口矮的时候收掉上手卡 ＋ 收对话区的内边距，把地方还给对话。
   *
   * 判据用 `visualViewport.height`（跨 iOS/Android 的唯一真相）：
   *   iOS 键盘弹出**不改 layout viewport**，`@media (max-height)` 在它上面不生效；Android 两者都变。
   * 阈值 660 + 只在窄屏（≤800px，即手机布局）生效：
   *   · 小屏手机竖屏不弹键盘（iPhone SE 667）不触发 —— 上手卡照旧看得到；
   *   · 弹了键盘（~407）触发；键盘高度由系统决定、与我们收不收卡片无关 ⇒ **不会抖**，不用滞回。
   *   · 桌面窗口拉矮**不触发**：桌面是另一套 6 行网格，对话区那行本来就带 minmax(144px,1fr) 保底。
   * ⚠️ 与「用户主动点收起」是两回事：这里**不写 localStorage** —— 键盘收回去，卡片自己回来。
   * 只加一个 class，具体收什么写在下面这张样式里（以后按同样的理由要收别的行，也往这儿加）。
   */
  var vvCss = document.createElement('style');
  vvCss.id = 'ab-vv-short-css';
  vvCss.textContent = [
    'body.ab-vv-short #asbudy-checklist{display:none}',
    'body.ab-vv-short .transcript{padding-top:10px;padding-bottom:12px}',
  ].join('\n');
  document.head.appendChild(vvCss);
  (function () {
    var SHORT = 660;
    var narrow = window.matchMedia('(max-width: 800px)');
    function vv() {
      var h = (window.visualViewport && window.visualViewport.height) || window.innerHeight || 0;
      document.body.classList.toggle('ab-vv-short', narrow.matches && h > 0 && h < SHORT);
    }
    vv();
    if (window.visualViewport) {
      visualViewport.addEventListener('resize', vv);
      visualViewport.addEventListener('scroll', vv);   // iOS 上有时只发 scroll
    }
    window.addEventListener('resize', vv);
    window.addEventListener('orientationchange', vv);
    if (narrow.addEventListener) narrow.addEventListener('change', vv);
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
            '<span style="font-size:22px;line-height:1.1;color:var(--human)">⚠️</span>' +
            '<div>' +
              '<div style="color:var(--human);font-weight:600;margin-bottom:6px">当前视角：' + esc(nm) + '（' + esc(vw) + '）</div>' +
              '<div style="color:var(--text-soft);font-size:14px;line-height:1.65">' +
                '你现在看到和操作的，都是<b>这个人的项目与文件</b>；每一步都会留痕。' +
                '<div style="color:var(--text-dim);margin-top:8px">回到自己的界面：点侧栏项目名旁的 <b>⇄</b> → 「退出，回到我自己的」。</div>' +
              '</div>' +
            '</div>' +
          '</div>';
      });
    }, 700);
  })();

  /* 桌面「设置」走这条路进来：URL 带 ?settings=1 → 自动把「高级设置」摊开。
     为什么不重写一份设置页：这些面板已经在这里了，写第二份就是两份要维护的活；
     而设置本身（模型服务 / 记忆 / 审批 / 显示项）**都是按人的**，跟哪个项目无关。 */
  (function () {
    try {
      var sp = new URLSearchParams(location.search);
      if (sp.get('settings') !== '1') return;
      // 这个 iframe 是拿来「只当设置面板用」的 —— 挂个记号让样式把官方三栏（会话/对话/预览）
      // 收起来。不然老板看到的是「设置面板 + 三栏对话」一起冒出来（2026-09-17 报的）。
      // ⚠️ **只在桌面窗口里**（iframe）这么干（2026-09-21）：它不光收三栏，还会把浮层自己的
      //   标题＋「关闭」拆掉（外壳由桌面那个窗口提供）。单独访问（手机 / 直接打开）时没有那层外壳，
      //   拆了就变成「铺满屏幕、一个按钮都没有」—— 进去出不来。
      if (window.self !== window.top) document.body.classList.add('asb-settings-only');
      var tries = 0;
      var t = setInterval(function () {
        if (typeof openAdvanced === 'function' && document.getElementById('composer-input')) {
          clearInterval(t);
          setTimeout(openAdvanced, 400);
        }
        if (++tries > 40) clearInterval(t);   // 最多等 12 秒，等不到就不弹（别把界面卡成半成品）
      }, 300);
    } catch (e) { /* 参数读不到就算了 */ }
  })();

  /* 桌面「我的」走这条路进来：URL 带 ?my=1 → 自动把「我的」菜单摊开。
     跟 ?settings=1 同一个套路：不重写第二份，直接把外挂里已有的那个菜单打开。
     「我的」是按账号的（客户管理 / 员工 / 密码），跟哪个项目无关 ——
     桌面侧用名下第一个项目打开这个 iframe。 */
  (function () {
    try {
      var sp = new URLSearchParams(location.search);
      if (sp.get('my') !== '1') return;
      // 只当「我的」面板用：把官方三栏（会话 / 对话 / 预览）收起来，
      // 否则加载瞬间会先闪一下三栏再盖上层（跟 ?settings=1 同一个理由）。
      // ⚠️ 同样**只在桌面窗口里**（if­rame）—— 否则浮层会连自己的标题与「关闭」一起没掉（见上面那处注释）。
      if (window.self !== window.top) document.body.classList.add('asb-settings-only');
      var tries = 0;
      var t = setInterval(function () {
        if (typeof openMyMenu === 'function' && document.getElementById('composer-input')) {
          clearInterval(t);
          setTimeout(openMyMenu, 400);
        }
        if (++tries > 40) clearInterval(t);
      }, 300);
    } catch (e) { /* 参数读不到就算了 */ }
  })();

  /* 桌面形态：从桌面窗口打开的（?desk=1）给 body 挂个记号。
     样式表据此把我们 2026-09-14 塞进官方侧栏的那几块收起来（项目文件 / 我的资料 /
     退回）—— 那些是网页形态的遗留，桌面形态下它们有自己的去处（我的空间 / 窗口菜单），
     不该跟会话列表挤在一起。只是收起来，不是删功能：
     不带这个参数（手机、单独打开、直接访问项目）一切照旧。 */
  (function () {
    try {
      if (new URLSearchParams(location.search).get('desk') !== '1') return;
      document.body.classList.add('asb-desk');
    } catch (e) { /* 参数读不到就算了 */ }
  })();

  /* 桌面开始菜单「用量明细」走这条路进来：URL 带 `?usage=1` → 自动把用量面板打开。
     跟 `?my=1` / `?settings=1` 同一个套路：不重写第二份，直接把外挂里已有的那个面板摊开。
     （我 2026-09-20 加：老板说「我的」里没看到 —— 多给一个**一眼能看到**的入口。） */
  (function () {
    try {
      var sp = new URLSearchParams(location.search);
      if (sp.get('usage') !== '1') return;
      // ⚠️ 只有桌面窗口里才拆外壳（同 ?my=1 那处：单独访问时拆了会「进去出不来」）
      if (window.self !== window.top) document.body.classList.add('asb-settings-only');
      var tries = 0;
      var t = setInterval(function () {
        if (typeof openTokenUsage === 'function' && document.getElementById('composer-input')) {
          clearInterval(t);
          setTimeout(openTokenUsage, 400);
        }
        if (++tries > 40) clearInterval(t);
      }, 300);
    } catch (e) { /* 参数读不到就算了 */ }
  })();

  if (!bindLogo()) {
    var tries = 0;
    var t = setInterval(function () { if (bindLogo() || ++tries > 60) clearInterval(t); }, 400);
  }

  /* ── 发图片（2026-09-21 第 44 轮）────────────────────────────────
   * 引擎早就会收图（`POST /v1/threads/{id}/turns` 的 `images:[{mime,dataBase64}]`），
   * 官方网页没做入口。这里补上：按钮 / 粘贴 / 拖入三种取图方式，发送时塞进请求。
   *
   * 为什么不改官方代码：**拦 fetch** 就够了 —— 官方的流式、乐观 UI 全照旧，
   * 我们只在它发出 `/turns` 前把 images 拼进 body。
   *
   * ⚠️ 两条引擎侧的硬规矩（照抄引擎校验，不自己发明）：
   *   ① 模型必须是**明确支持图片**的那个（`image_input === 'supported'`）——
   *      `auto` 和不支持/未知的模型，引擎一律 400（`runtime_threads.rs:9125-9185`）。
   *   ② `prompt` 不能为空 ⇒ 只发图不打字会被拒，所以空文字时拦下来。
   */
  (function bindImageAttach() {
    var OK_TYPES = { 'image/png': 1, 'image/jpeg': 1, 'image/gif': 1, 'image/webp': 1 };
    var MAX_ONE = 4 * 1024 * 1024;    // 引擎单图上限
    var MAX_TOTAL = 5 * 1024 * 1024;  // 引擎单次总量上限
    var MAX_N = 10;                   // 引擎单次张数上限
    var pending = [];
    var capCache = {};
    var barEl = null;

    function b64(file) {
      return new Promise(function (res, rej) {
        var fr = new FileReader();
        fr.onload = function () {
          var s = String(fr.result || '');
          var i = s.indexOf(',');
          if (i < 0) { rej(new Error('读不出图片内容')); return; }
          res({ name: file.name || '图片', mime: file.type, dataBase64: s.slice(i + 1), size: file.size });
        };
        fr.onerror = function () { rej(new Error('读不出图片内容')); };
        fr.readAsDataURL(file);
      });
    }

    function totalSize() {
      var n = 0;
      for (var i = 0; i < pending.length; i++) n += pending[i].size;
      return n;
    }

    function note(text) {
      if (!barEl) return;
      var t = barEl.querySelector('.ab-img-note');
      if (t) t.textContent = text || '';
    }

    function paint() {
      if (!barEl) return;
      var strip = barEl.querySelector('.ab-img-strip');
      if (!pending.length) { barEl.hidden = true; strip.innerHTML = ''; return; }
      barEl.hidden = false;
      strip.innerHTML = pending.map(function (p, i) {
        return '<span class="ab-img-chip"><img alt="" src="data:' + p.mime + ';base64,' + p.dataBase64 + '">' +
          '<button type="button" title="去掉" data-del="' + i + '">×</button></span>';
      }).join('');
      strip.querySelectorAll('button[data-del]').forEach(function (b) {
        b.onclick = function () {
          pending.splice(Number(b.getAttribute('data-del')), 1);
          note('');
          paint();
        };
      });
    }

    async function addFiles(files) {
      var list = Array.prototype.slice.call(files || []);
      if (!list.length) return;
      note('');
      for (var i = 0; i < list.length; i++) {
        var f = list[i];
        if (!OK_TYPES[f.type]) { note('只支持 PNG / JPEG / GIF / WebP 图片'); continue; }
        if (f.size > MAX_ONE) { note('单张图片不能超过 4MB'); continue; }
        if (pending.length >= MAX_N) { note('一次最多 ' + MAX_N + ' 张'); break; }
        try {
          var one = await b64(f);
          if (totalSize() + one.size > MAX_TOTAL) { note('一次总共不能超过 5MB'); break; }
          pending.push(one);
        } catch (e) { note('读不出这张图'); }
      }
      paint();
      // 只发图不打字 → 引擎会 400（prompt 不能为空）。**选完图就提醒**，不等客户点发送 ——
      // 输入框空时官方那个发送按钮本来就是灰的，点了也不会有任何反应。
      var box = document.getElementById('composer-input');
      if (pending.length && box && !String(box.value || '').trim()) {
        note('图片要配一句话一起发送');
      }
    }
    // 给**父页面（桌面 desk.html）**留一个入口：桌面上把一个文件/文件夹图标拖到窗口上，
    //   父页面把它的**绝对路径**递进来 —— 走的是同一条「已带上：xxx」引用链路（不读内容、不判类型）。
    //   见 desk.html 的 handIconToWin。
    window.__asbudyPickFile = function (abs, name) {
      try {
        document.dispatchEvent(new CustomEvent('asbudy-file-picked', { detail: { path: abs, name: name } }));
      } catch (e) { /* 派发不出去就算了 */ }
    };

    /** 当前会话的模型支不支持看图 —— 问引擎要（不猜）。 */
    function modelSupportsImage() {
      var tid = MODEL_THREAD;
      if (!tid) return Promise.resolve('unknown');
      return api('/v1/threads/' + encodeURIComponent(tid)).then(function (r) {
        // ⚠️ 详情接口把会话**包在 `.thread` 里**（列表才是裸数组）—— 2026-09-22 就是漏了这层，
        //    model 永远读到空 ⇒ 被误判成「模型不支持图片」⇒ 按钮直接灰掉（老板报的正是这个）。
        //    同一个坑 `openModelPicker` 里早就踩过并写明了（`var th = body.thread || body`）。
        var t = (r && r.body && (r.body.thread || r.body)) || {};
        var model = String(t.model || '').trim();
        var pid = String(t.model_provider || t.model_provider_id || '').trim();
        // `auto` 引擎明确不收图（runtime_threads.rs:9128）⇒ 禁；
        // 但**读不到**只是我们没查清 —— 宁可让客户试（引擎会给出人话），也别把能用的按钮灰掉。
        if (model.toLowerCase() === 'auto') return 'unsupported';
        if (!model) return 'unknown';
        var key = pid + '|' + model;
        if (capCache[key]) return capCache[key];
        return loadProviderModels(pid).then(function (ms) {
          var hit = (ms || []).filter(function (m) { return m && m.id === model; })[0];
          var st = (hit && hit.image_input) || 'unknown';
          capCache[key] = st;
          return st;
        });
      }).catch(function () { return 'unknown'; });
    }

    function syncButton(cap) {
      var b = document.getElementById('asbudy-img-btn');
      if (!b) return;
      var bad = cap === 'unsupported';
      b.disabled = !!bad;
      b.title = bad ? '当前会话的模型不支持图片 —— 换一个支持图片的模型再发'
                    : '发图片（也可以直接粘贴或拖进来）';
      b.setAttribute('data-ab-img', cap || 'unknown');
    }

    function build() {
      var form = document.getElementById('composer');
      var actions = document.querySelector('.composer-actions');
      if (!form || !actions || document.getElementById('asbudy-img-btn')) return !!form && !!actions;
      var style = document.createElement('style');
      style.textContent = '.ab-img-bar{padding:6px 2px 0}.ab-img-strip{display:flex;flex-wrap:wrap;gap:6px}' +
        '.ab-img-chip{position:relative;display:inline-block;line-height:0}' +
        '.ab-img-chip img{width:56px;height:56px;object-fit:cover;border-radius:6px;border:1px solid var(--line)}' +
        '.ab-img-chip button{position:absolute;top:-6px;right:-6px;width:18px;height:18px;line-height:1;border-radius:50%;' +
        'border:1px solid var(--line);background:var(--surface-raised);color:var(--text);cursor:pointer;font-size:12px;padding:0}' +
        '.ab-img-note{font-size:12px;color:var(--text-soft)}';
      document.head.appendChild(style);

      barEl = document.createElement('div');
      barEl.className = 'ab-img-bar';
      barEl.hidden = true;
      barEl.innerHTML = '<div class="ab-img-strip"></div><div class="ab-img-note"></div>';
      form.insertBefore(barEl, form.querySelector('.composer-bar'));

      var pick = document.createElement('input');
      pick.type = 'file';
      pick.accept = 'image/png,image/jpeg,image/gif,image/webp';
      pick.multiple = true;
      pick.hidden = true;
      pick.onchange = function () { addFiles(pick.files); pick.value = ''; };
      form.appendChild(pick);

      var btn = document.createElement('button');
      btn.type = 'button';
      btn.id = 'asbudy-img-btn';
      btn.className = 'quiet-button';
      btn.textContent = '图片';
      btn.onclick = function () { pick.click(); };
      actions.insertBefore(btn, actions.firstChild);

      // 「图片」旁边再给一个入口：从**自己的空间目录**里选（2026-09-22 老板要的）——
      //   以前只能从本机 / 手机选，服务器上早就存在的图（桌面 / 我的资料）选不了。
      //   单独的按钮、不合并进「图片」：不改变原有入口的行为（避开和官方 / 已有测试的冲突）。
      var spaceBtn = document.createElement('button');
      spaceBtn.type = 'button';
      spaceBtn.id = 'asbudy-img-space-btn';
      spaceBtn.className = 'quiet-button';
      spaceBtn.textContent = '我的空间';
      spaceBtn.title = '从你的空间目录里选文件带上（图片也行）';
      spaceBtn.onclick = function () { openPickFromSpace(); };
      btn.parentNode.insertBefore(spaceBtn, btn.nextSibling);

      return true;
    }

    /* ── 从「我的空间」选文件 ─────────────────────────────────────────
       数据源就是账号根目录树：`GET /_gate/files?scope=root&dir=<相对路径>`，
       一次读一层（跟桌面的「文件夹窗口」同一个接口）。
       选中一份 → 走**已有的引用链路**（`asbudy-file-picked`）—— 输入框上方出现
       「已带上：xxx」，发送时把`（用这份：<绝对路径>）`拼进消息。
       **不在这里判类型、也不读内容**：文本 AI 自己 read_file、图片自己 view_image、
       表格/文档也自己找工具读 —— 2026-09-22 老板纠正（上一版只列图片，太窄）。 */
    function pickOneFromSpace(x) {
      if (!x.abs) { notify('拿不到「' + x.name + '」的位置'); return; }
      try {
        document.dispatchEvent(new CustomEvent('asbudy-file-picked', { detail: { path: x.abs, name: x.name } }));
      } catch (e) { /* 派发不出去就算了 */ }
      notify('已带上：' + x.name);
      var w = document.getElementById('asbudy-pick');
      if (w) w.remove();
    }

    function openPickFromSpace() {
      var old = document.getElementById('asbudy-pick');
      if (old) old.remove();
      var wrap = document.createElement('div');
      wrap.id = 'asbudy-pick';
      wrap.style.cssText = 'position:fixed;inset:0;z-index:100000;background:rgba(2,7,17,.62);'
        + 'display:flex;align-items:center;justify-content:center';
      wrap.innerHTML =
        '<div style="width:min(680px,92vw);max-height:78vh;display:flex;flex-direction:column;background:var(--surface,#0e1a30);'
        + 'border:1px solid var(--line);border-radius:12px;box-shadow:0 20px 60px rgba(0,0,0,.55);overflow:hidden">'
        + '<div style="display:flex;align-items:center;gap:10px;padding:12px 14px;border-bottom:1px solid var(--line)">'
        + '<strong style="font-size:14px;color:var(--text);flex:none">从我的空间选文件</strong>'
        + '<span id="asbudy-pick-crumb" style="font-size:12px;color:var(--text-soft);flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap"></span>'
        + '<button type="button" id="asbudy-pick-x" class="quiet-button">关闭</button></div>'
        + '<div id="asbudy-pick-list" style="padding:10px 12px;overflow:auto"></div></div>';
      document.body.appendChild(wrap);
      wrap.addEventListener('click', function (e) { if (e.target === wrap) wrap.remove(); });
      wrap.querySelector('#asbudy-pick-x').onclick = function () { wrap.remove(); };
      var listEl = wrap.querySelector('#asbudy-pick-list');
      var crumbEl = wrap.querySelector('#asbudy-pick-crumb');

      function rowBase() {
        var b = document.createElement('button');
        b.type = 'button';
        b.style.cssText = 'display:flex;align-items:center;gap:10px;width:100%;text-align:left;padding:8px 10px;margin:2px 0;'
          + 'background:transparent;border:1px solid transparent;border-radius:8px;color:var(--text);cursor:pointer;font:inherit';
        b.onmouseenter = function () { b.style.background = 'var(--hover)'; };
        b.onmouseleave = function () { b.style.background = 'transparent'; };
        return b;
      }

      function load(dir) {
        crumbEl.textContent = dir ? ('/' + dir) : '账号根目录';
        listEl.innerHTML = '<div style="color:var(--text-soft);font-size:13px;padding:8px">读取中…</div>';
        fetch('/_gate/files?scope=root&dir=' + encodeURIComponent(dir || ''), { credentials: 'same-origin' })
          .then(function (r) { return r.ok ? r.json() : Promise.reject(new Error('读不到这个目录')); })
          .then(function (d) {
            var items = (d && d.files) || [];
            listEl.innerHTML = '';
            if (dir) {
              var up = rowBase();
              up.innerHTML = '<span style="flex:1">.. 返回上一层</span>';
              up.onclick = function () { load(dir.split('/').slice(0, -1).join('/')); };
              listEl.appendChild(up);
            }
            var dirs = items.filter(function (x) { return x.isDir; });
            var files = items.filter(function (x) { return !x.isDir; });
            if (!dirs.length && !files.length) {
              listEl.innerHTML = '<div style="color:var(--text-soft);font-size:13px;padding:8px">这个位置没有文件</div>';
              return;
            }
            dirs.forEach(function (x) {
              var row = rowBase();
              row.innerHTML = '<span style="flex:1">' + esc(x.name) + '/</span>';
              row.onclick = function () { load(dir ? dir + '/' + x.name : x.name); };
              listEl.appendChild(row);
            });
            files.forEach(function (x) {
              var row = rowBase();
              row.innerHTML = '<span style="flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">' + esc(x.name) + '</span>'
                + '<span style="font-size:12px;color:var(--text-soft);flex:none">带上</span>';
              row.onclick = function () { pickOneFromSpace(x); };
              listEl.appendChild(row);
            });
          })
          .catch(function (e) {
            listEl.innerHTML = '<div style="color:var(--text-soft);font-size:13px;padding:8px">' + esc(e.message) + '</div>';
          });
      }
      load('');
    }

    // 粘贴：焦点在输入框时贴图
    document.addEventListener('paste', function (e) {
      try {
        var dt = e.clipboardData;
        if (!dt || !dt.items) return;
        var files = [];
        for (var i = 0; i < dt.items.length; i++) {
          var it = dt.items[i];
          if (it.kind === 'file' && OK_TYPES[it.type]) {
            var f = it.getAsFile();
            if (f) files.push(f);
          }
        }
        if (files.length) { e.preventDefault(); addFiles(files); }
      } catch (err) { /* 粘贴拿不到就算了 */ }
    });

    // 拖进来
    document.addEventListener('dragover', function (e) {
      try {
        if (e.dataTransfer && Array.prototype.some.call(e.dataTransfer.types || [], function (t) { return t === 'Files'; })) {
          e.preventDefault();
        }
      } catch (err) {}
    });
    document.addEventListener('drop', function (e) {
      try {
        var t = e.target;
        if (!t || !t.closest || !t.closest('#composer')) return;
        if (!e.dataTransfer || !e.dataTransfer.files || !e.dataTransfer.files.length) return;
        e.preventDefault();
        addFiles(e.dataTransfer.files);
      } catch (err) {}
    });

    // 只发图不打字 → 引擎会 400（prompt 不能为空），这里拦住说清楚
    document.addEventListener('click', function (e) {
      var t = e.target;
      if (!t || !t.closest || !t.closest('#send-message')) return;
      if (!pending.length) return;
      var box = document.getElementById('composer-input');
      if (box && !String(box.value || '').trim()) {
        e.preventDefault();
        e.stopPropagation();
        note('图片要配一句话一起发（引擎不接受只有图片的消息）');
      }
    }, true);

    // 发送时把图拼进 /turns 的请求体
    var prevFetch = window.fetch;
    window.fetch = function (url, opt) {
      var next = opt;
      try {
        var u = typeof url === 'string' ? url : ((url && url.url) || '');
        var meth = String((opt && opt.method) || 'GET').toUpperCase();
        if (pending.length && meth === 'POST' && /\/v1\/threads\/[^\/?]+\/turns$/.test(u) && opt && typeof opt.body === 'string') {
          var body = JSON.parse(opt.body);
          if (!body.images || !body.images.length) {
            body.images = pending.map(function (p) { return { mime: p.mime, dataBase64: p.dataBase64 }; });
            next = Object.assign({}, opt, { body: JSON.stringify(body) });
          }
        }
      } catch (err) { next = opt; }
      var sent = prevFetch.call(this, url, next);
      try {
        if (next !== opt && sent && sent.then) {
          sent.then(function (r) {
            if (r && r.ok) { pending = []; note(''); paint(); }
            else { note('图片没能发出去 —— 当前模型可能不支持图片'); }
          }).catch(function () {});
        }
      } catch (err) {}
      return sent;
    };

    // 进页面 / 换会话时刷新按钮状态
    var lastTid = '';
    function tick() {
      if (!build()) return;
      if (MODEL_THREAD !== lastTid) {
        lastTid = MODEL_THREAD;
        capCache = {};                       // 换了会话 → 能力可能不同，重查
        modelSupportsImage().then(syncButton);
      }
      // 按钮必须总有能力标记（会话还没选出来时是 unknown —— 不猜，也不假装支持）
      var bb = document.getElementById('asbudy-img-btn');
      if (bb && !bb.getAttribute('data-ab-img')) syncButton('unknown');
    }
    if (!tick()) {
      var n = 0;
      var t = setInterval(function () { if (tick() || ++n > 60) clearInterval(t); }, 400);
    } else {
      var n2 = 0;
      var t2 = setInterval(function () { if (++n2 > 150) { clearInterval(t2); return; } tick(); }, 1000);
    }
  })();
})();
