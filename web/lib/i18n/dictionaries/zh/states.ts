import type { StatesDict } from "../types";

/**
 * Simplified-Chinese dictionary for shared surface states: empty, loading,
 * error, retry, recovery, not-found, and the connection banner.
 */
export const states: StatesDict = {
  loadingLabel: "加载中…",
  emptyTitle: "这里还没有内容",
  emptyBody: "暂时没有可显示的记录。这里不会用编造的内容填充。",
  errorTitle: "页面没有加载完成",
  errorBody: "中途出了问题。你的操作没有丢失；请重试，如果持续失败，请报告给我们。",
  retry: "重试",
  reload: "重新加载页面",
  homeLink: "返回首页",
  docsIndexLink: "打开文档目录",
  notFoundTitle: "谁还没打过错字呢。",
  notFoundBody: "这个页面还不存在。\n这款游戏也一样。",
  notFoundHomeLink: "返回基地",
  notFoundPosterAlt: "虚构游戏《Codwhale: Modern Whalefare》的海报，一只身穿战术装备的蓝鲸。",
  unavailableTitle: "实时记录尚未加载",
  unavailableBody: "数据源没有响应上一次刷新，或者此页面自构建以来尚未刷新。这里不会用编造的内容填充。",

  offlineTitle: "你已离线",
  offlineBody: "操作已暂停，直到网络恢复。此处显示的内容不会刷新。",
  reconnectingTitle: "正在重新连接…",
  reconnectingBody: "正在检查连接（第 {attempt} 次）。",
  degradedTitle: "连接不稳定",
  degradedBody: "服务器没有响应上一次检查。你看到的内容可能已过时。",
  onlineTitle: "已恢复在线",
  onlineBody: "连接已恢复。",
  retryNow: "立即重试",
  dismiss: "关闭",
  lastChecked: "上次检查 {time}",
};
