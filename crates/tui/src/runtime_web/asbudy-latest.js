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
 */
(function () {
  var t = null, btn = null, mo = null;

  function ensure() {
    if (btn && document.body.contains(btn)) return true;
    t = document.getElementById('transcript');
    if (!t || !document.body) return false;

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
    btn.addEventListener('click', function () {
      try { t.scrollTo({ top: t.scrollHeight, behavior: 'smooth' }); }
      catch (e) { t.scrollTop = t.scrollHeight; }
      // 平滑滚动走的不是一步到位，sync 会在滚动过程中反复纠正，这里先给个即时的反馈
      setTimeout(sync, 400);
    });
    document.body.appendChild(btn);

    t.addEventListener('scroll', sync, { passive: true });
    addEventListener('resize', place);
    // 新消息让内容变高时不会触发 scroll 事件 → 盯一下子元素变化
    try { mo = new MutationObserver(sync); mo.observe(t, { childList: true, subtree: true }); } catch (e) { /* 老浏览器就算了 */ }
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

  function sync() {
    if (!btn || !t || !document.body.contains(btn)) { return; }
    place();
    var near = t.scrollHeight - t.scrollTop - t.clientHeight < 120;   // 跟官方 wasNearBottom 同一个阈值
    btn.hidden = near || t.scrollHeight <= t.clientHeight;
  }

  if (!ensure()) {
    var tries = 0;
    var iv = setInterval(function () { if (ensure() || ++tries > 60) clearInterval(iv); }, 400);
  }
  // 切回页面立刻对一次（后台标签页里的事件不可靠）
  document.addEventListener('visibilitychange', function () { if (!document.hidden) sync(); });
})();
