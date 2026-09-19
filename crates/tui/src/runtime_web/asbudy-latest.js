/* AsBudy「回到最新」浮动按钮（外挂 · 2026-09-18）
 *
 * 为什么做：CLI/TUI 一直有 jump-to-latest —— 源码里就是
 * `crates/tui/src/tui/widgets/mod.rs` 的 `jump_to_latest_button`（配上 Ctrl+End），
 * 而且**只在「有滚动条、你往上翻了」的时候才出现**。官方 **web 没有这个**，
 * 老板对比出来的（原话：「我现在用的 cli 版就有」）。照那个精神补上。
 *
 * 为什么挂在 body 上、不进 #transcript：
 *   官方渲染会 reconcile 对话区的子元素（`app.mjs` 的 `reconcileChildren(dom.transcript, …)`），
 *   塞进 transcript 里的节点会被它删掉。所以按钮挂外面，自己按 composer 的位置定位。
 *
 * 行为：默认隐藏 → 往上翻历史时出现 → 点一下回到最新（平滑滚动）→ 到底后自动隐藏。
 *
 * ⚠️「在不在最新」**不自己量像素**（2026-09-19 改）—— 官方 CLI 里按钮显隐与
 *   transcript 的滚动状态是**同一个真相**：`!app.viewport.transcript_scroll.is_at_tail()`
 *   （`crates/tui/src/tui/widgets/mod.rs`）。web 侧同一个真相由 app.mjs 的 `app.transcriptFollow`
 *   持有，通过 `asbudy:tail` 事件广播出来。以前这里自己量 120px 像素差，跟主逻辑各判一套 ——
 *   手机键盘把对话区压矮时会与主逻辑不一致。
 */
(function () {
  var t = null, btn = null, mo = null;
  // 还没收到事件时先按几何估一个（app.mjs 初始化就会广播一次）
  var atTail = true;

  function ensure() {
    if (btn && document.body.contains(btn)) return true;
    t = document.getElementById('transcript');
    if (!t || !document.body) return false;
    atTail = t.scrollHeight - t.scrollTop - t.clientHeight <= 16;

    var st = document.createElement('style');
    st.textContent = [
      // 配色取官方 runtime_web/styles.css 的语义变量（Ocean 深海那套），不另创
      '.ab-jump{position:fixed;right:34px;z-index:60;width:36px;height:36px;border-radius:50%;',
      'border:1px solid var(--line-strong,rgba(72,215,255,.24));background:var(--surface-raised,#172945);',
      'color:var(--text-soft,#b6c0d4);font:inherit;font-size:17px;line-height:1;cursor:pointer;',
      'display:flex;align-items:center;justify-content:center;',
      'box-shadow:0 4px 16px rgba(0,0,0,.45);transition:opacity .15s ease,color .15s ease}',
      '.ab-jump:hover{color:var(--text,#f6f2e8);border-color:var(--action,#6aaef2)}',
    ].join('');
    document.head.appendChild(st);

    btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'ab-jump';
    btn.title = '回到最新';
    btn.setAttribute('aria-label', '回到最新');
    btn.textContent = '↓';
    btn.hidden = true;
    btn.addEventListener('click', function () { toBottom(); });
    document.body.appendChild(btn);

    t.addEventListener('scroll', sync, { passive: true });
    addEventListener('resize', place);
    // 新消息让内容变高时不会触发 scroll 事件 → 盯一下子元素变化（重新定位按钮）
    try { mo = new MutationObserver(sync); mo.observe(t, { childList: true, subtree: true }); } catch (e) { /* 老浏览器就算了 */ }
    // 唯一的真相来源：app.mjs 广播的「在不在最新」（照官方 CLI 的 is_at_tail）
    document.addEventListener('asbudy:tail', function (e) {
      if (e && e.detail && typeof e.detail.atTail === 'boolean') { atTail = e.detail.atTail; sync(); }
    });
    sync();
    return true;
  }

  /** 贴着输入框上方 —— 用 composer 的真实位置算，不写死像素（输入框会长高、小屏会换行） */
  function place() {
    if (!btn) return;
    var c = document.querySelector('.composer-wrap');
    var h = c ? (innerHeight - c.getBoundingClientRect().top) : 96;
    btn.style.bottom = Math.round(h + 14) + 'px';
  }

  /** 回底（按钮与键盘路线共用） */
  function toBottom() {
    if (!t) return;
    try { t.scrollTo({ top: t.scrollHeight, behavior: 'smooth' }); }
    catch (e) { t.scrollTop = t.scrollHeight; }
    // 平滑滚动走的不是一步到位，sync 会在滚动过程中反复纠正，这里先给个即时的反馈
    setTimeout(sync, 400);
  }

  /** 回顶 */
  function toTop() {
    if (!t) return;
    try { t.scrollTo({ top: 0, behavior: 'smooth' }); }
    catch (e) { t.scrollTop = 0; }
    setTimeout(sync, 400);
  }

  function editableFocused() {
    var el = document.activeElement;
    if (!el) return false;
    var tag = String(el.tagName || '').toLowerCase();
    return tag === 'input' || tag === 'textarea' || el.isContentEditable === true;
  }

  /* 键盘路线（2026-09-19 搬）—— CLI 那个按钮背后本来就有键位：
   *   `End` / `Ctrl+End` → 回底（`tui/ui/handlers.rs:187`，`KeyCode::End => scroll_to_bottom()`）
   *   `Alt+Shift+G`      → 回底（`tui/ui/event_loop.rs:6237`，`Char('G')` ＋ alt 修饰）
   *   `Home` / `Ctrl+Home` → 回顶 · `Alt+G` → 回顶（同文件 `Char('g')` 分支）
   * ⚠️ 网页上**裸 `End` / `Home` 不接管**：那在输入框里是「光标到行尾 / 行首」，
   *   抢过来会毁掉正在打字的人。只接管 `Ctrl+End` / `Ctrl+Home`，且
   *   **焦点在可编辑元素里时仍然不抢**（原生编辑动作优先）。
   *   `Alt+G` / `Alt+Shift+G` 不产生字符输入，任何时候都接管。 */
  document.addEventListener('keydown', function (e) {
    if (!t || e.defaultPrevented || e.isComposing) return;
    var key = e.key;
    if (e.altKey && !e.ctrlKey && !e.metaKey && (key === 'g' || key === 'G')) {
      e.preventDefault();
      if (e.shiftKey) toBottom(); else toTop();
      return;
    }
    if (e.ctrlKey && !e.altKey && !e.metaKey && (key === 'End' || key === 'Home')) {
      if (editableFocused()) return;
      e.preventDefault();
      if (key === 'End') toBottom(); else toTop();
    }
  }, true);

  function sync() {
    if (!btn || !t || !document.body.contains(btn)) { return; }
    place();
    // 在最新 == 不显示（跟 CLI 的 is_at_tail 一致）；内容不够长（没滚动条）也不显示，
    // 照 CLI 的 `jump_to_latest_button_rect(area, scrollbar.is_some())`。
    btn.hidden = atTail || t.scrollHeight <= t.clientHeight;
  }

  if (!ensure()) {
    var tries = 0;
    var iv = setInterval(function () { if (ensure() || ++tries > 60) clearInterval(iv); }, 400);
  }
  // 切回页面立刻对一次（后台标签页里的事件不可靠）
  document.addEventListener('visibilitychange', function () { if (!document.hidden) sync(); });
})();
