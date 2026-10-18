(function () {
  const TABLE = {
    "zh-cn": {
      "html.lang": "zh-CN",
      "app.title": "Minecraft Server 下载器",
      "app.ready": "准备就绪",
      "btn.language": "切换语言", "btn.theme": "切换主题", "btn.settings": "设置", "btn.import": "导入来源",
      "btn.search": "搜索版本", "btn.clearSearch": "清空搜索", "btn.refresh": "刷新列表", "btn.selectAll": "全选未下载",
      "btn.clearSelection": "清空选择", "btn.start": "开始下载", "btn.pause": "暂停", "btn.resume": "继续",
      "btn.stop": "停止", "btn.data": "数据目录", "btn.clearLog": "清空", "btn.choose": "浏览",
      "btn.cancel": "取消", "btn.save": "保存设置", "btn.importSave": "保存来源", "btn.close": "关闭",
      "search.placeholder": "搜索版本号", "list.versions": "版本列表", "list.empty": "还没有版本列表",
      "list.emptyCopy": "点击刷新，获取正式版服务器 JAR 清单。", "list.noMatch": "没有匹配的版本",
      "list.noMatchCopy": "换一个搜索词再试。", "detail.title": "下载工作区", "detail.copy": "选择版本并管理下载任务。",
      "progress.overall": "总进度", "progress.bytes": "字节进度", "status.ready": "准备就绪", "status.refreshing": "正在刷新版本列表…",
      "log.title": "下载日志", "log.copy": "只保留最近 300 条记录。",
      "mirror.official": "官方", "mirror.bmclapi": "BMCLAPI", "mirror.custom": "自定义",
      "btn.about": "关于",
      "about.title": "关于 Minecraft Server 下载器", "about.copy": "版本与运行环境信息。",
      "about.version": "版本", "about.runtime": "运行时 Rust", "about.installPath": "安装路径", "about.settingsPath": "配置文件",
      "settings.title": "设置", "settings.copy": "保存后才会写入本地配置。", "settings.directory": "下载目录", "settings.mirror": "镜像源",
      "settings.proxy": "代理", "settings.proxyHint": "http://host:port（留空不使用）", "settings.concurrency": "并发数",
      "settings.timeout": "超时（秒）", "settings.retry": "重试次数", "settings.language": "界面语言", "settings.theme": "主题",
      "settings.themeSystem": "跟随系统", "settings.themeLight": "浅色", "settings.themeDark": "深色",
      "lang.auto": "跟随系统", "lang.zh": "中文", "lang.en": "English",
      "import.title": "导入版本清单", "import.copy": "只需填写名称和版本清单地址，保存后即可使用。", "import.name": "名称",
      "import.address": "版本清单地址",
     
      "footer.source": "镜像源", "footer.config": "配置未保存",
      "downloaded": "已下载", "notDownloaded": "未下载", "selected": "已选", "speed": "速度", "remaining": "剩余",
      "msg.saved": "设置已保存", "msg.imported": "来源已保存", "msg.removed": "来源已移除", "msg.failed": "操作失败",
      "error.directoryRequired": "请先选择下载目录。",
      "error.urlBlocked": "该地址不安全或不允许访问。",
      "error.urlInvalid": "地址格式无效。",
      "error.nameRequired": "请填写名称。",
      "error.sourceMissing": "请选择有效的自定义来源。",
      "error.downloadNoSelection": "请先选择未下载版本。",
      "error.downloadNotRunning": "当前没有正在运行的下载任务。",
      "error.operationRunning": "上一个操作仍在进行。",
      "error.networkFailed": "网络请求失败，请稍后重试。",
      "msg.running": "下载进行中", "msg.noSelection": "请先选择未下载版本", "msg.cancelled": "操作已取消",
      "log.starting": "开始下载 {0}", "log.completed": "{0} 完成", "log.failed": "{0} 下载失败",
      "log.stopped": "下载已停止", "log.allDone": "所有任务完成", "log.finishedWithErrors": "完成，但有失败项目",
      "log.diskFull": "磁盘空间不足", "log.noUrl": "{0} 没有服务器下载地址", "log.metadataFailed": "{0} 元数据获取失败"
    },
    "en-us": {
      "html.lang": "en",
      "app.title": "Minecraft Server Downloader",
      "app.ready": "Ready",
      "btn.language": "Switch language", "btn.theme": "Toggle theme", "btn.settings": "Settings", "btn.import": "Import source",
      "btn.search": "Search versions", "btn.clearSearch": "Clear search", "btn.refresh": "Refresh list", "btn.selectAll": "Select all not downloaded",
      "btn.clearSelection": "Clear selection", "btn.start": "Start download", "btn.pause": "Pause", "btn.resume": "Resume",
      "btn.stop": "Stop", "btn.data": "Data folder", "btn.clearLog": "Clear", "btn.choose": "Browse",
      "btn.cancel": "Cancel", "btn.save": "Save settings", "btn.importSave": "Save source", "btn.close": "Close",
      "search.placeholder": "Search version", "list.versions": "Version list", "list.empty": "No version list yet",
      "list.emptyCopy": "Refresh to fetch the release server JAR manifest.", "list.noMatch": "No matching versions",
      "list.noMatchCopy": "Try another search term.", "detail.title": "Download workspace", "detail.copy": "Select versions and manage download tasks.",
      "progress.overall": "Overall progress", "progress.bytes": "Byte progress", "status.ready": "Ready", "status.refreshing": "Refreshing versions...",
      "log.title": "Download log", "log.copy": "Only the latest 300 entries are kept.",
      "mirror.official": "Official", "mirror.bmclapi": "BMCLAPI", "mirror.custom": "Custom",
      "btn.about": "About",
      "about.title": "About Minecraft Server Downloader", "about.copy": "Version and runtime information.",
      "about.version": "Version", "about.runtime": "Runtime Rust", "about.installPath": "Install path", "about.settingsPath": "Configuration file",
      "settings.title": "Settings", "settings.copy": "Settings are written only after you save.", "settings.directory": "Download directory", "settings.mirror": "Mirror source",
      "settings.proxy": "Proxy", "settings.proxyHint": "http://host:port (leave empty to disable)", "settings.concurrency": "Concurrency",
      "settings.timeout": "Timeout (seconds)", "settings.retry": "Retries", "settings.language": "Language", "settings.theme": "Theme",
      "settings.themeSystem": "Follow system", "settings.themeLight": "Light", "settings.themeDark": "Dark",
      "lang.auto": "System", "lang.zh": "中文", "lang.en": "English",
      "import.title": "Import version manifest", "import.copy": "Enter a name and manifest URL, then save to use it.", "import.name": "Name",
      "import.address": "Manifest URL",
     
      "footer.source": "Mirror", "footer.config": "Unsaved settings",
      "downloaded": "Downloaded", "notDownloaded": "Not downloaded", "selected": "Selected", "speed": "Speed", "remaining": "Remaining",
      "msg.saved": "Settings saved", "msg.imported": "Source saved", "msg.removed": "Source removed", "msg.failed": "Operation failed",
      "error.directoryRequired": "Choose a download directory first.",
      "error.urlBlocked": "That address is unsafe or not allowed.",
      "error.urlInvalid": "The address format is invalid.",
      "error.nameRequired": "Enter a name.",
      "error.sourceMissing": "Select a valid custom source.",
      "error.downloadNoSelection": "Select an undownloaded version first.",
      "error.downloadNotRunning": "No download is currently running.",
      "error.operationRunning": "The previous operation is still running.",
      "error.networkFailed": "The network request failed. Try again later.",
      "msg.running": "Download is running", "msg.noSelection": "Select a version first", "msg.cancelled": "Operation cancelled",
      "log.starting": "Starting {0}", "log.completed": "{0} completed", "log.failed": "{0} failed",
      "log.stopped": "Download stopped", "log.allDone": "All tasks completed", "log.finishedWithErrors": "Finished with failed items",
      "log.diskFull": "Not enough disk space", "log.noUrl": "{0} has no server URL", "log.metadataFailed": "{0} metadata failed"
    }
  };
  const SUPPORTED = ["zh-cn", "en-us"];
  let current = "zh-cn";
  function normalize(preference) {
    const value = String(preference || "auto").trim().toLowerCase();
    if (value.startsWith("zh")) return "zh-cn";
    if (value.startsWith("en")) return "en-us";
    return "auto";
  }
  function resolve(preference) {
    const direct = normalize(preference);
    if (direct !== "auto") return direct;
    const detected = normalize(navigator.language || "");
    return detected === "auto" ? "zh-cn" : detected;
  }
  function t(key, ...args) {
    const table = TABLE[current] || TABLE["zh-cn"];
    const text = table[key] === undefined ? (TABLE["zh-cn"][key] || key) : table[key];
    return text.replace(/\{(\d+)\}/g, (match, index) => args[index] === undefined ? match : String(args[index]));
  }
  function apply(preference) {
    current = resolve(preference);
    document.documentElement.lang = t("html.lang");
    document.title = t("app.title");
    document.querySelectorAll("[data-i18n]").forEach(node => { node.textContent = t(node.dataset.i18n); });
    document.querySelectorAll("[data-i18n-aria]").forEach(node => node.setAttribute("aria-label", t(node.dataset.i18nAria)));
    document.querySelectorAll("[data-i18n-title]").forEach(node => node.setAttribute("title", t(node.dataset.i18nTitle)));
    document.querySelectorAll("[data-i18n-placeholder]").forEach(node => node.setAttribute("placeholder", t(node.dataset.i18nPlaceholder)));
    document.dispatchEvent(new CustomEvent("i18n:applied", { detail: { language: current } }));
  }
  window.I18n = { t, apply, current: () => current, supported: SUPPORTED };
})();



