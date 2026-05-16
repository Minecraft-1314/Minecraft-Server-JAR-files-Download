import os
import re
import time
import json
import locale
import threading

from PyQt6.QtCore import (
    Qt, QThread, pyqtSignal, QStandardPaths, QSettings
)
from PyQt6.QtWidgets import (
    QApplication, QMainWindow, QWidget, QVBoxLayout, QHBoxLayout,
    QLabel, QLineEdit, QPushButton, QProgressBar, QTableWidget,
    QTableWidgetItem, QHeaderView, QPlainTextEdit, QComboBox,
    QFileDialog, QCheckBox, QAbstractItemView,
    QStyleFactory, QStatusBar, QFrame, QSplitter, QDialog
)
from PyQt6.QtGui import QFont, QColor, QPalette

import requests

try:
    import darkdetect
    _DARKDETECT_AVAILABLE = True
except ImportError:
    _DARKDETECT_AVAILABLE = False

MANIFEST_URL = "https://launchermeta.mojang.com/mc/game/version_manifest.json"
CACHE_FILE = os.path.join(QStandardPaths.writableLocation(
    QStandardPaths.StandardLocation.CacheLocation), "manifest_cache.json")
CACHE_DURATION = 1800


class I18n:
    translations = {
        "en": {
            "title": "Minecraft Server Downloader",
            "subtitle": "Download all release Minecraft server JARs with one click",
            "download_dir": "Download Directory",
            "browse": "Browse",
            "version_list": "Version List",
            "search": "Search version...",
            "version": "Version",
            "status": "Status",
            "select": "Select",
            "downloaded": "Downloaded",
            "not_downloaded": "Not downloaded",
            "select_all": "Select All",
            "deselect_all": "Deselect All",
            "start_download": "Start Download",
            "pause": "Pause",
            "resume": "Resume",
            "stop": "Stop",
            "refresh": "Refresh",
            "overall_progress": "Overall Progress",
            "current_file_progress": "Current File Progress",
            "speed": "Speed",
            "remaining_time": "Remaining",
            "log": "Log",
            "no_version_selected": "No versions selected for download.",
            "download_running": "Download task is already running.",
            "error_manifest": "Failed to fetch version manifest: {error}",
            "error_meta": "Failed to fetch metadata for {ver}: {error}",
            "error_download": "Download failed for {jar}: {error}",
            "no_server_url": "{ver} has no server download URL.",
            "downloading": "Downloading {jar} ...",
            "downloaded": "{jar} completed.",
            "skipped": "{jar} already exists, skipped.",
            "stopped": "Download stopped.",
            "interrupted": "Interrupted {jar}.",
            "all_done": "All tasks completed.",
            "paused": "Paused",
            "continued": "Continued",
            "select_dir": "Select Directory",
            "language": "Language",
            "theme": "Theme",
            "dark": "Dark",
            "light": "Light",
            "zh": "中文",
            "en": "English",
            "ready": "Ready",
            "disk_space_error": "Insufficient disk space. Required: {required:.2f} MB, Available: {free:.2f} MB",
            "selected_count": "Selected: {count}",
            "dialog_error": "Error",
            "dialog_warning": "Warning",
            "dialog_info": "Information",
        },
        "zh": {
            "title": "Minecraft 服务器下载器",
            "subtitle": "一键下载所有正式版 Minecraft 服务器 JAR 文件",
            "download_dir": "下载目录",
            "browse": "浏览",
            "version_list": "版本列表",
            "search": "搜索版本...",
            "version": "版本号",
            "status": "状态",
            "select": "选择",
            "downloaded": "已下载",
            "not_downloaded": "未下载",
            "select_all": "全选未下载",
            "deselect_all": "取消全选",
            "start_download": "开始下载",
            "pause": "暂停",
            "resume": "继续",
            "stop": "停止",
            "refresh": "刷新列表",
            "overall_progress": "总下载进度",
            "current_file_progress": "当前文件进度",
            "speed": "速度",
            "remaining_time": "剩余时间",
            "log": "下载日志",
            "no_version_selected": "没有需要下载的版本。",
            "download_running": "下载任务已在运行中。",
            "error_manifest": "无法获取版本清单: {error}",
            "error_meta": "获取 {ver} 元数据失败: {error}",
            "error_download": "{jar} 下载失败: {error}",
            "no_server_url": "{ver} 无服务器下载地址。",
            "downloading": "正在下载 {jar} ...",
            "downloaded": "{jar} 完成。",
            "skipped": "{jar} 已存在，跳过。",
            "stopped": "下载已停止。",
            "interrupted": "中断 {jar}。",
            "all_done": "所有任务完成。",
            "paused": "已暂停",
            "continued": "已继续",
            "select_dir": "选择目录",
            "language": "语言",
            "theme": "主题",
            "dark": "暗色",
            "light": "明亮",
            "zh": "中文",
            "en": "English",
            "ready": "就绪",
            "disk_space_error": "磁盘空间不足。需要: {required:.2f} MB，可用: {free:.2f} MB",
            "selected_count": "已选: {count}",
            "dialog_error": "错误",
            "dialog_warning": "警告",
            "dialog_info": "提示",
        }
    }

    @classmethod
    def get_text(cls, key, lang="en", **kwargs):
        text = cls.translations.get(lang, cls.translations["en"]).get(key, key)
        if kwargs:
            text = text.format(**kwargs)
        return text


def get_system_language():
    try:
        lang, _ = locale.getdefaultlocale()
        if lang and lang.startswith("zh"):
            return "zh"
    except Exception:
        pass
    return "en"


def read_cache():
    if os.path.exists(CACHE_FILE):
        try:
            with open(CACHE_FILE, 'r', encoding='utf-8') as f:
                cache = json.load(f)
                if time.time() - cache.get("timestamp", 0) < CACHE_DURATION:
                    return cache.get("manifest")
        except Exception:
            pass
    return None


def write_cache(manifest):
    try:
        os.makedirs(os.path.dirname(CACHE_FILE), exist_ok=True)
        with open(CACHE_FILE, 'w', encoding='utf-8') as f:
            json.dump({"timestamp": time.time(), "manifest": manifest}, f)
    except Exception:
        pass


class DownloaderEngine:
    def __init__(self, download_dir, lang="en"):
        self.download_dir = download_dir
        self.lang = lang
        self.stop_event = threading.Event()
        self.pause_event = threading.Event()
        self.pause_event.set()
        self.local_files = set()
        self._scan_local_files()

    def _t(self, key, **kwargs):
        return I18n.get_text(key, self.lang, **kwargs)

    def _scan_local_files(self):
        self.local_files.clear()
        base = self.download_dir
        if not os.path.isdir(base):
            return
        for root, dirs, files in os.walk(base):
            for f in files:
                if f.endswith(".jar"):
                    self.local_files.add(f)

    @staticmethod
    def _sanitize_filename(name):
        return re.sub(r'[\\/*?:"<>|]', "", name)

    def get_version_list(self):
        manifest = read_cache()
        if manifest:
            return self._parse_manifest(manifest)

        try:
            resp = requests.get(MANIFEST_URL, timeout=30)
            resp.raise_for_status()
            manifest = resp.json()
            write_cache(manifest)
            return self._parse_manifest(manifest)
        except Exception as e:
            raise Exception(self._t("error_manifest", error=str(e)))

    def _parse_manifest(self, manifest):
        versions = []
        for entry in manifest.get("versions", []):
            if entry.get("type") != "release":
                continue
            v_id = entry["id"]
            safe_id = self._sanitize_filename(v_id)
            jar_name = f"minecraft_server_{safe_id}.jar"
            status = self._t("downloaded") if jar_name in self.local_files else self._t("not_downloaded")
            versions.append({
                "id": v_id,
                "safe_id": safe_id,
                "url": entry["url"],
                "jar_name": jar_name,
                "status": status,
                "releaseTime": entry.get("releaseTime", ""),
                "folder": self._get_folder_name(v_id)
            })
        versions.sort(key=lambda x: x["releaseTime"], reverse=True)
        return versions

    def _get_folder_name(self, version):
        parts = version.split(".")
        if len(parts) >= 2:
            return f"Minecraft {parts[0]}.{parts[1]} Server JAR files"
        return f"Minecraft {version} Server JAR files"

    @staticmethod
    def get_free_space(path):
        if os.name == 'nt':
            import ctypes
            free_bytes = ctypes.c_ulonglong(0)
            ctypes.windll.kernel32.GetDiskFreeSpaceExW(
                ctypes.c_wchar_p(path), None, None, ctypes.pointer(free_bytes))
            return free_bytes.value
        else:
            st = os.statvfs(path)
            return st.f_bavail * st.f_frsize

    def check_disk_space(self, selected_versions):
        total_size = sum(40 * 1024 * 1024 for _ in selected_versions)
        free = self.get_free_space(self.download_dir)
        return free >= total_size, free, total_size

    def download_selected(self, selected_versions, log_signal, progress_signal, speed_signal):
        total = len(selected_versions)
        completed = 0
        self.stop_event.clear()
        self.pause_event.set()

        enough, free, required = self.check_disk_space(selected_versions)
        if not enough:
            log_signal.emit(
                I18n.get_text("disk_space_error", self.lang,
                              required=required / (1024 * 1024),
                              free=free / (1024 * 1024)), True)
            progress_signal.emit(0, 0, 0)
            return False

        for ver in selected_versions:
            if self.stop_event.is_set():
                log_signal.emit(self._t("stopped"), False)
                break

            while not self.pause_event.is_set() and not self.stop_event.is_set():
                time.sleep(0.2)
            if self.stop_event.is_set():
                break

            ver_id = ver["id"]
            jar_name = ver["jar_name"]
            meta_url = ver["url"]

            log_signal.emit(self._t("downloading", jar=jar_name), False)

            try:
                meta_resp = requests.get(meta_url, timeout=30)
                meta_resp.raise_for_status()
                meta = meta_resp.json()
                download_url = meta.get("downloads", {}).get("server", {}).get("url")
                if not download_url:
                    log_signal.emit(self._t("no_server_url", ver=ver_id), True)
                    continue
            except Exception as e:
                log_signal.emit(self._t("error_meta", ver=ver_id, error=str(e)), True)
                continue

            target_dir = os.path.join(self.download_dir, ver["folder"])
            os.makedirs(target_dir, exist_ok=True)
            file_path = os.path.join(target_dir, jar_name)

            if os.path.exists(file_path):
                log_signal.emit(self._t("skipped", jar=jar_name), False)
                self.local_files.add(jar_name)
                completed += 1
                progress_signal.emit(completed, total, 0)
                continue

            try:
                with requests.get(download_url, stream=True, timeout=120) as r:
                    r.raise_for_status()
                    total_size = int(r.headers.get("content-length", 0))
                    downloaded = 0
                    start_time = time.time()
                    last_update = start_time

                    with open(file_path, "wb") as f:
                        for chunk in r.iter_content(chunk_size=8192):
                            if self.stop_event.is_set():
                                f.close()
                                os.remove(file_path)
                                log_signal.emit(self._t("interrupted", jar=jar_name), False)
                                progress_signal.emit(0, 0, 0)
                                return False

                            while not self.pause_event.is_set() and not self.stop_event.is_set():
                                time.sleep(0.2)

                            if self.stop_event.is_set():
                                f.close()
                                os.remove(file_path)
                                progress_signal.emit(0, 0, 0)
                                return False

                            f.write(chunk)
                            downloaded += len(chunk)
                            now = time.time()
                            if now - last_update > 0.5 or downloaded == total_size:
                                elapsed = now - start_time
                                speed = downloaded / elapsed if elapsed > 0 else 0
                                if total_size > 0:
                                    percent = int(downloaded / total_size * 100)
                                    progress_signal.emit(completed, total, percent)
                                remaining = (total_size - downloaded) / speed if speed > 0 else 0
                                speed_signal.emit(speed, remaining)
                                last_update = now

                log_signal.emit(self._t("downloaded", jar=jar_name), False)
                self.local_files.add(jar_name)
            except Exception as e:
                log_signal.emit(self._t("error_download", jar=jar_name, error=str(e)), True)
                if os.path.exists(file_path):
                    os.remove(file_path)
                progress_signal.emit(completed, total, 0)
                continue

            completed += 1
            progress_signal.emit(completed, total, 0)

        log_signal.emit(self._t("all_done"), False)
        progress_signal.emit(0, 0, 0)
        speed_signal.emit(0, 0)
        return True

    def pause(self):
        self.pause_event.clear()

    def resume(self):
        self.pause_event.set()

    def stop(self):
        self.stop_event.set()
        self.pause_event.set()


class FetchVersionsThread(QThread):
    result_ready = pyqtSignal(object)
    error_occurred = pyqtSignal(str)

    def __init__(self, engine):
        super().__init__()
        self.engine = engine

    def run(self):
        try:
            versions = self.engine.get_version_list()
            self.result_ready.emit(versions)
        except Exception as e:
            self.error_occurred.emit(str(e))


class DownloadThread(QThread):
    log_signal = pyqtSignal(str, bool)
    progress_signal = pyqtSignal(int, int, int)
    speed_signal = pyqtSignal(float, float)
    finished_signal = pyqtSignal()
    error_signal = pyqtSignal(str)

    def __init__(self, engine, queue):
        super().__init__()
        self.engine = engine
        self.queue = queue

    def run(self):
        try:
            self.engine.download_selected(
                self.queue,
                self.log_signal,
                self.progress_signal,
                self.speed_signal
            )
        except Exception as e:
            self.error_signal.emit(str(e))
        finally:
            self.finished_signal.emit()


class CustomMessageBox(QDialog):
    def __init__(self, parent, icon_type, text):
        super().__init__(parent)
        lang = parent.lang if hasattr(parent, 'lang') else 'en'
        title_map = {
            "info": I18n.get_text("dialog_info", lang),
            "warning": I18n.get_text("dialog_warning", lang),
            "error": I18n.get_text("dialog_error", lang),
        }
        title = title_map.get(icon_type, "Info")
        self.setWindowTitle(title)
        self.setMinimumWidth(420)
        self.setModal(True)
        self.setWindowFlags(Qt.WindowType.Dialog | Qt.WindowType.CustomizeWindowHint |
                            Qt.WindowType.WindowTitleHint | Qt.WindowType.WindowCloseButtonHint)

        layout = QVBoxLayout(self)
        layout.setSpacing(18)
        layout.setContentsMargins(28, 28, 28, 28)

        hbox = QHBoxLayout()
        self.icon_label = QLabel()
        if icon_type == "info":
            self.icon_label.setText("ℹ️")
        elif icon_type == "warning":
            self.icon_label.setText("⚠️")
        elif icon_type == "error":
            self.icon_label.setText("❌")
        else:
            self.icon_label.setText("ℹ️")
        self.icon_label.setFont(QFont("Segoe UI", 30))
        self.icon_label.setFixedWidth(45)
        hbox.addWidget(self.icon_label)

        self.msg_label = QLabel(text)
        self.msg_label.setWordWrap(True)
        self.msg_label.setFont(QFont("Segoe UI", 12))
        hbox.addWidget(self.msg_label, 1)
        layout.addLayout(hbox)

        button_layout = QHBoxLayout()
        button_layout.addStretch()
        ok_btn = QPushButton("OK")
        ok_btn.clicked.connect(self.accept)
        ok_btn.setFixedWidth(90)
        ok_btn.setFixedHeight(34)
        ok_btn.setFont(QFont("Segoe UI", 10))
        button_layout.addWidget(ok_btn)
        layout.addLayout(button_layout)

        self.setAutoFillBackground(True)
        self.apply_parent_theme(parent)

    def apply_parent_theme(self, parent):
        if parent:
            self.setPalette(parent.palette())
            base_color = parent.palette().color(QPalette.ColorRole.Window).name()
            text_color = parent.palette().color(QPalette.ColorRole.WindowText).name()
            btn_bg = parent.palette().color(QPalette.ColorRole.Button).name()
            btn_text = parent.palette().color(QPalette.ColorRole.ButtonText).name()
            highlight = parent.palette().color(QPalette.ColorRole.Highlight).name()
            self.setStyleSheet(f"""
                QDialog {{
                    background-color: {base_color};
                    border: 1px solid {parent.palette().color(QPalette.ColorRole.Base).name()};
                    border-radius: 14px;
                }}
                QLabel {{
                    color: {text_color};
                }}
                QPushButton {{
                    background-color: {btn_bg};
                    color: {btn_text};
                    border: none;
                    border-radius: 8px;
                    padding: 6px 18px;
                    font-size: 13px;
                }}
                QPushButton:hover {{
                    background-color: {highlight};
                    color: white;
                }}
            """)


class MainWindow(QMainWindow):
    def __init__(self):
        super().__init__()
        self.settings = QSettings("MinecraftServerDownloader", "Settings")
        self.lang = self.settings.value("language", get_system_language())
        dark_default = self.is_system_dark()
        self.theme_dark = self.settings.value("dark_mode", dark_default, bool)

        download_path = self.settings.value("download_dir", "")
        if not download_path or not os.path.isdir(download_path):
            download_path = QStandardPaths.writableLocation(
                QStandardPaths.StandardLocation.DownloadLocation)
            if not download_path or not os.path.isdir(download_path):
                download_path = os.path.expanduser('~')

        self.engine = DownloaderEngine(download_dir=download_path, lang=self.lang)
        self.versions = []
        self.selected_versions = set()
        self.download_queue = []
        self.is_running = False
        self.is_paused = False
        self.fetch_thread = None
        self.download_thread = None

        self.setWindowTitle("Minecraft Server Downloader")
        self.setMinimumSize(960, 640)

        self.status_bar = QStatusBar()
        self.setStatusBar(self.status_bar)

        self.setup_ui()
        self.apply_theme(self.theme_dark)
        self.status_bar.showMessage(I18n.get_text("ready", self.lang))
        self.refresh_version_list()
        self.restore_geometry()
        self.show()

    def closeEvent(self, event):
        self.save_settings()
        super().closeEvent(event)

    def save_settings(self):
        self.settings.setValue("language", self.lang)
        self.settings.setValue("dark_mode", self.theme_dark)
        self.settings.setValue("download_dir", self.engine.download_dir)
        self.settings.setValue("geometry", self.saveGeometry())
        self.settings.setValue("splitter_state", self.splitter.saveState())

    def restore_geometry(self):
        geometry = self.settings.value("geometry")
        if geometry:
            self.restoreGeometry(geometry)
        else:
            self.showMaximized()
        splitter_state = self.settings.value("splitter_state")
        if splitter_state:
            self.splitter.restoreState(splitter_state)

    def is_system_dark(self):
        if _DARKDETECT_AVAILABLE:
            return darkdetect.isDark()
        return True

    def setup_ui(self):
        central_widget = QWidget()
        self.setCentralWidget(central_widget)
        main_layout = QVBoxLayout(central_widget)
        main_layout.setContentsMargins(0, 0, 0, 0)
        main_layout.setSpacing(0)

        content_widget = QWidget()
        content_layout = QVBoxLayout(content_widget)
        content_layout.setContentsMargins(24, 20, 24, 20)
        content_layout.setSpacing(16)

        header_layout = QHBoxLayout()
        self.title_label = QLabel()
        self.title_label.setFont(QFont("Segoe UI", 28, QFont.Weight.Bold))
        header_layout.addWidget(self.title_label)
        header_layout.addStretch()

        controls_layout = QHBoxLayout()
        self.theme_toggle = QPushButton()
        self.theme_toggle.setCheckable(True)
        self.theme_toggle.setFixedWidth(70)
        self.theme_toggle.setToolTip("Toggle dark/light mode")
        self.theme_toggle.toggled.connect(self.toggle_theme)
        controls_layout.addWidget(self.theme_toggle)

        self.lang_combo = QComboBox()
        self.lang_combo.addItem("English", "en")
        self.lang_combo.addItem("中文", "zh")
        self.lang_combo.setCurrentIndex(0 if self.lang == "en" else 1)
        self.lang_combo.currentIndexChanged.connect(self.change_language)
        self.lang_combo.setFixedWidth(110)
        controls_layout.addWidget(self.lang_combo)

        header_layout.addLayout(controls_layout)
        content_layout.addLayout(header_layout)

        self.subtitle_label = QLabel()
        self.subtitle_label.setFont(QFont("Segoe UI", 12))
        content_layout.addWidget(self.subtitle_label)

        dir_layout = QHBoxLayout()
        self.dir_label = QLabel()
        self.dir_label.setFont(QFont("Segoe UI", 10, QFont.Weight.Bold))
        dir_layout.addWidget(self.dir_label)
        self.dir_input = QLineEdit(self.engine.download_dir)
        self.dir_input.setReadOnly(True)
        dir_layout.addWidget(self.dir_input, 1)
        self.browse_button = QPushButton()
        self.browse_button.clicked.connect(self.select_directory)
        dir_layout.addWidget(self.browse_button)
        content_layout.addLayout(dir_layout)

        search_layout = QHBoxLayout()
        self.search_input = QLineEdit()
        self.search_input.setPlaceholderText("Search version...")
        self.search_input.textChanged.connect(self.filter_versions)
        search_layout.addWidget(self.search_input, 1)
        content_layout.addLayout(search_layout)

        self.splitter = QSplitter(Qt.Orientation.Vertical)
        table_container = QFrame()
        table_container.setFrameShape(QFrame.Shape.StyledPanel)
        table_layout = QVBoxLayout(table_container)
        table_layout.setContentsMargins(0, 0, 0, 0)

        self.table = QTableWidget()
        self.table.setColumnCount(3)
        self.table.setHorizontalHeaderLabels(["Version", "Status", "Select"])
        self.table.horizontalHeader().setSectionResizeMode(0, QHeaderView.ResizeMode.Stretch)
        self.table.horizontalHeader().setSectionResizeMode(1, QHeaderView.ResizeMode.Fixed)
        self.table.horizontalHeader().resizeSection(1, 150)
        self.table.horizontalHeader().setSectionResizeMode(2, QHeaderView.ResizeMode.Fixed)
        self.table.horizontalHeader().resizeSection(2, 80)
        self.table.verticalHeader().setVisible(False)
        self.table.setSelectionMode(QAbstractItemView.SelectionMode.NoSelection)
        self.table.setEditTriggers(QAbstractItemView.EditTrigger.NoEditTriggers)
        self.table.setAlternatingRowColors(True)
        table_layout.addWidget(self.table)

        log_container = QFrame()
        log_container.setFrameShape(QFrame.Shape.StyledPanel)
        log_layout = QVBoxLayout(log_container)
        log_layout.setContentsMargins(0, 0, 0, 0)
        self.log_text = QPlainTextEdit()
        self.log_text.setReadOnly(True)
        self.log_text.setMaximumBlockCount(500)
        self.log_text.setFrameShape(QFrame.Shape.NoFrame)
        log_layout.addWidget(self.log_text)

        self.splitter.addWidget(table_container)
        self.splitter.addWidget(log_container)
        self.splitter.setStretchFactor(0, 2)
        self.splitter.setStretchFactor(1, 1)
        content_layout.addWidget(self.splitter, 1)

        progress_layout = QVBoxLayout()
        self.overall_label = QLabel()
        self.overall_label.setFont(QFont("Segoe UI", 10, QFont.Weight.Bold))
        progress_layout.addWidget(self.overall_label)
        overall_row = QHBoxLayout()
        self.overall_progress = QProgressBar()
        self.overall_progress.setRange(0, 100)
        overall_row.addWidget(self.overall_progress, 1)
        self.overall_text = QLabel("0 / 0")
        overall_row.addWidget(self.overall_text)
        progress_layout.addLayout(overall_row)

        self.current_label = QLabel()
        self.current_label.setFont(QFont("Segoe UI", 10, QFont.Weight.Bold))
        progress_layout.addWidget(self.current_label)
        current_row = QHBoxLayout()
        self.current_progress = QProgressBar()
        self.current_progress.setRange(0, 100)
        current_row.addWidget(self.current_progress, 1)
        self.current_text = QLabel("0%")
        current_row.addWidget(self.current_text)
        progress_layout.addLayout(current_row)

        self.speed_label = QLabel()
        self.speed_label.setFont(QFont("Segoe UI", 9))
        progress_layout.addWidget(self.speed_label)
        content_layout.addLayout(progress_layout)

        button_layout = QHBoxLayout()
        self.btn_refresh = QPushButton()
        self.btn_refresh.clicked.connect(self.refresh_version_list)
        self.btn_select_all = QPushButton()
        self.btn_select_all.clicked.connect(self.select_all)
        self.btn_deselect_all = QPushButton()
        self.btn_deselect_all.clicked.connect(self.deselect_all)
        button_layout.addWidget(self.btn_refresh)
        button_layout.addWidget(self.btn_select_all)
        button_layout.addWidget(self.btn_deselect_all)
        button_layout.addStretch()

        self.btn_start = QPushButton()
        self.btn_start.clicked.connect(self.start_download)
        self.btn_pause = QPushButton()
        self.btn_pause.clicked.connect(self.toggle_pause)
        self.btn_pause.setEnabled(False)
        self.btn_stop = QPushButton()
        self.btn_stop.clicked.connect(self.stop_download)
        self.btn_stop.setEnabled(False)
        button_layout.addWidget(self.btn_start)
        button_layout.addWidget(self.btn_pause)
        button_layout.addWidget(self.btn_stop)
        content_layout.addLayout(button_layout)

        main_layout.addWidget(content_widget, 1)

        self.update_texts()
        self.update_theme_button_text()

    def show_message(self, icon_type, text):
        msg = CustomMessageBox(self, icon_type, text)
        msg.exec()

    def update_texts(self, refresh_table=False):
        t = lambda key: I18n.get_text(key, self.lang)
        self.title_label.setText(t("title"))
        self.subtitle_label.setText(t("subtitle"))
        self.dir_label.setText(t("download_dir"))
        self.browse_button.setText(t("browse"))
        self.search_input.setPlaceholderText(t("search"))
        self.table.setHorizontalHeaderLabels([t("version"), t("status"), t("select")])
        self.overall_label.setText(t("overall_progress"))
        self.current_label.setText(t("current_file_progress"))
        self.btn_refresh.setText(t("refresh"))
        self.btn_select_all.setText(t("select_all"))
        self.btn_deselect_all.setText(t("deselect_all"))
        self.btn_start.setText(t("start_download"))
        self.btn_pause.setText(t("pause") if not self.is_paused else t("resume"))
        self.btn_stop.setText(t("stop"))
        self.status_bar.showMessage(t("ready"))
        self.update_theme_button_text()
        if refresh_table:
            for row in range(self.table.rowCount()):
                item = self.table.item(row, 1)
                if item:
                    vid = self.table.item(row, 0).text()
                    v = next((v for v in self.versions if v["id"] == vid), None)
                    if v:
                        is_downloaded = v["jar_name"] in self.engine.local_files
                        item.setText(t("downloaded") if is_downloaded else t("not_downloaded"))
        self.update_selected_count()

    def update_theme_button_text(self):
        t = lambda key: I18n.get_text(key, self.lang)
        if self.theme_toggle.isChecked():
            self.theme_toggle.setText(t("light"))
        else:
            self.theme_toggle.setText(t("dark"))

    def update_selected_count(self):
        count = len(self.selected_versions)
        self.status_bar.showMessage(
            I18n.get_text("selected_count", self.lang, count=count) + "  " +
            I18n.get_text("ready", self.lang))

    def change_language(self, idx):
        self.lang = self.lang_combo.currentData()
        self.engine.lang = self.lang
        self.update_texts(refresh_table=True)
        self.update_theme_button_text()

    def select_directory(self):
        dir_path = QFileDialog.getExistingDirectory(
            self, "Select Directory", self.dir_input.text())
        if dir_path:
            self.dir_input.setText(dir_path)
            self.engine.download_dir = dir_path
            self.engine._scan_local_files()
            self.refresh_version_list()

    def refresh_version_list(self):
        if self.is_running:
            return
        self.status_bar.showMessage(I18n.get_text("refresh", self.lang) + "...")
        self.fetch_thread = FetchVersionsThread(self.engine)
        self.fetch_thread.result_ready.connect(self.on_versions_loaded)
        self.fetch_thread.error_occurred.connect(self.on_fetch_error)
        self.fetch_thread.start()

    def on_versions_loaded(self, versions):
        self.versions = versions
        self.selected_versions.clear()
        self.filter_versions()
        self.update_selected_count()

    def on_fetch_error(self, error_msg):
        self.show_message("error", error_msg)
        self.status_bar.showMessage(error_msg)

    def filter_versions(self):
        search = self.search_input.text().lower().strip()
        self.table.setRowCount(0)
        for v in self.versions:
            if search and search not in v["id"].lower():
                continue
            row = self.table.rowCount()
            self.table.insertRow(row)
            self.table.setItem(row, 0, QTableWidgetItem(v["id"]))
            self.table.setItem(row, 1, QTableWidgetItem(v["status"]))
            cb = QCheckBox()
            cb.setChecked(v["id"] in self.selected_versions)
            cb.stateChanged.connect(
                lambda state, vid=v["id"], cb=cb: self.toggle_selection(vid, cb.isChecked()))
            self.table.setCellWidget(row, 2, cb)
        self.update_selected_count()

    def toggle_selection(self, vid, checked):
        if checked:
            self.selected_versions.add(vid)
        else:
            self.selected_versions.discard(vid)
        self.update_selected_count()

    def select_all(self):
        self.selected_versions.clear()
        for v in self.versions:
            if v["status"] != I18n.get_text("downloaded", self.lang):
                self.selected_versions.add(v["id"])
        self.filter_versions()

    def deselect_all(self):
        self.selected_versions.clear()
        self.filter_versions()

    def start_download(self):
        if self.is_running:
            self.show_message("warning", I18n.get_text("download_running", self.lang))
            return
        self.download_queue = [v for v in self.versions
                               if v["id"] in self.selected_versions
                               and v["status"] != I18n.get_text("downloaded", self.lang)]
        if not self.download_queue:
            self.show_message("info", I18n.get_text("no_version_selected", self.lang))
            return
        self.is_running = True
        self.is_paused = False
        self.update_button_states(running=True)
        self.overall_progress.setValue(0)
        self.overall_text.setText(f"0 / {len(self.download_queue)}")
        self.current_progress.setValue(0)
        self.current_text.setText("0%")
        self.speed_label.setText("")

        self.download_thread = DownloadThread(self.engine, self.download_queue)
        self.download_thread.log_signal.connect(self.add_log)
        self.download_thread.progress_signal.connect(self.update_progress)
        self.download_thread.speed_signal.connect(self.update_speed)
        self.download_thread.finished_signal.connect(self.on_download_finished)
        self.download_thread.error_signal.connect(self.on_download_error)
        self.download_thread.start()

    def update_button_states(self, running):
        self.btn_refresh.setEnabled(not running)
        self.btn_select_all.setEnabled(not running)
        self.btn_deselect_all.setEnabled(not running)
        self.btn_start.setEnabled(not running)
        self.btn_pause.setEnabled(running)
        self.btn_stop.setEnabled(running)
        if running:
            self.btn_pause.setText(I18n.get_text("pause", self.lang))
        self.update_texts()

    def toggle_pause(self):
        if not self.is_running:
            return
        if self.is_paused:
            self.engine.resume()
            self.btn_pause.setText(I18n.get_text("pause", self.lang))
            self.is_paused = False
            self.status_bar.showMessage(I18n.get_text("continued", self.lang))
        else:
            self.engine.pause()
            self.btn_pause.setText(I18n.get_text("resume", self.lang))
            self.is_paused = True
            self.status_bar.showMessage(I18n.get_text("paused", self.lang))

    def stop_download(self):
        if not self.is_running:
            return
        self.engine.stop()
        self.btn_stop.setEnabled(False)
        self.status_bar.showMessage(I18n.get_text("stopped", self.lang))

    def on_download_finished(self):
        self.is_running = False
        self.is_paused = False
        self.update_button_states(running=False)
        self.refresh_version_list()
        self.status_bar.showMessage(I18n.get_text("all_done", self.lang))

    def on_download_error(self, error_msg):
        self.show_message("error", error_msg)

    def add_log(self, message, error=False):
        color = "red" if error else "inherit"
        self.log_text.appendHtml(
            f'<span style="color:{color};">[{time.strftime("%H:%M:%S")}] {message}</span>')

    def update_progress(self, completed, total, current_percent):
        if total > 0:
            self.overall_progress.setValue(int(completed / total * 100))
            self.overall_text.setText(f"{completed} / {total}")
        else:
            self.overall_progress.setValue(0)
            self.overall_text.setText("0 / 0")
        self.current_progress.setValue(current_percent)
        self.current_text.setText(f"{current_percent}%")

    def update_speed(self, speed, remaining):
        if speed <= 0:
            speed_str = "--"
        elif speed > 1024 * 1024:
            speed_str = f"{speed / (1024 * 1024):.1f} MB/s"
        elif speed > 1024:
            speed_str = f"{speed / 1024:.1f} KB/s"
        else:
            speed_str = f"{speed:.0f} B/s"

        if remaining > 0:
            mins = int(remaining // 60)
            secs = int(remaining % 60)
            remain_str = f"{mins}分{secs}秒"
        else:
            remain_str = "--"

        self.speed_label.setText(
            f"{I18n.get_text('speed', self.lang)}: {speed_str}  "
            f"{I18n.get_text('remaining_time', self.lang)}: {remain_str}")

    def toggle_theme(self, checked):
        self.theme_dark = checked
        self.apply_theme(checked)

    def apply_theme(self, dark):
        if dark:
            palette = QPalette()
            palette.setColor(QPalette.ColorRole.Window, QColor(18, 18, 18))
            palette.setColor(QPalette.ColorRole.WindowText, QColor(230, 230, 230))
            palette.setColor(QPalette.ColorRole.Base, QColor(30, 30, 30))
            palette.setColor(QPalette.ColorRole.AlternateBase, QColor(45, 45, 45))
            palette.setColor(QPalette.ColorRole.ToolTipBase, QColor(18, 18, 18))
            palette.setColor(QPalette.ColorRole.ToolTipText, QColor(230, 230, 230))
            palette.setColor(QPalette.ColorRole.Text, QColor(230, 230, 230))
            palette.setColor(QPalette.ColorRole.Button, QColor(50, 50, 50))
            palette.setColor(QPalette.ColorRole.ButtonText, QColor(230, 230, 230))
            palette.setColor(QPalette.ColorRole.BrightText, Qt.GlobalColor.red)
            palette.setColor(QPalette.ColorRole.Highlight, QColor(0, 200, 200))
            palette.setColor(QPalette.ColorRole.HighlightedText, Qt.GlobalColor.black)
            QApplication.instance().setPalette(palette)
            self.setStyleSheet("""
                QWidget { background-color: #121212; color: #e0e0e0; }
                QPushButton { background-color: #2c2c2c; border: none; border-radius: 8px; padding: 8px 16px; font-size: 13px; }
                QPushButton:hover { background-color: #3c3c3c; }
                QPushButton:pressed { background-color: #505050; }
                QPushButton:disabled { color: #707070; }
                QProgressBar { border: none; border-radius: 6px; background-color: #2c2c2c; text-align: center; height: 10px; }
                QProgressBar::chunk { background-color: #00bcd4; border-radius: 6px; }
                QLineEdit, QPlainTextEdit, QTableWidget { background-color: #1e1e1e; border: 1px solid #3c3c3c; border-radius: 6px; padding: 4px; color: #e0e0e0; }
                QLineEdit:focus { border-color: #00bcd4; }
                QHeaderView::section { background-color: #2c2c2c; padding: 4px; border: none; }
                QTableWidget { gridline-color: #3c3c3c; }
                QComboBox { background-color: #2c2c2c; border: 1px solid #3c3c3c; border-radius: 6px; padding: 4px 8px; color: #e0e0e0; }
                QComboBox::drop-down { border: none; }
                QStatusBar { background-color: #121212; color: #e0e0e0; }
            """)
            self.title_label.setStyleSheet("color: #00e5ff; border-bottom: 2px solid #00e5ff; padding-bottom: 4px;")
            self.subtitle_label.setStyleSheet("color: #aaaaaa;")
            self.theme_toggle.setChecked(True)
        else:
            QApplication.instance().setPalette(QApplication.style().standardPalette())
            self.setStyleSheet("""
                QWidget { background-color: #f5f5f5; color: #202020; }
                QPushButton { background-color: #ffffff; border: 1px solid #d0d0d0; border-radius: 8px; padding: 8px 16px; font-size: 13px; }
                QPushButton:hover { background-color: #e0e0e0; }
                QPushButton:pressed { background-color: #cccccc; }
                QPushButton:disabled { color: #909090; }
                QProgressBar { border: 1px solid #d0d0d0; border-radius: 6px; background-color: #ffffff; text-align: center; height: 10px; }
                QProgressBar::chunk { background-color: #0088cc; border-radius: 6px; }
                QLineEdit, QPlainTextEdit, QTableWidget { background-color: #ffffff; border: 1px solid #d0d0d0; border-radius: 6px; padding: 4px; color: #202020; }
                QLineEdit:focus { border-color: #0088cc; }
                QHeaderView::section { background-color: #f0f0f0; padding: 4px; border: none; }
                QTableWidget { gridline-color: #d0d0d0; }
                QComboBox { background-color: #ffffff; border: 1px solid #d0d0d0; border-radius: 6px; padding: 4px 8px; color: #202020; }
                QComboBox::drop-down { border: none; }
                QStatusBar { background-color: #f5f5f5; color: #202020; }
            """)
            self.title_label.setStyleSheet("color: #0055aa; border-bottom: 2px solid #0055aa; padding-bottom: 4px;")
            self.subtitle_label.setStyleSheet("color: #666666;")
            self.theme_toggle.setChecked(False)
        self.update_theme_button_text()


if __name__ == "__main__":
    app = QApplication([])
    app.setStyle(QStyleFactory.create("Fusion"))
    window = MainWindow()
    app.exec()