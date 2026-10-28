//! 轻量多语言（任务 3）：九种语言字典 + 运行时切换。
//!
//! 方案：UI 文案持有 `Signal<String>`，语言切换时由 `apply` 统一重写信号，
//! 控件（label/button/tabs/菜单）下一帧自动跟随（windui 动态文案机制）。
//! 语言名称一律用本语言显示：简体中文 / 繁體中文 / English / 日本語 / 한국어 /
//! Русский / Deutsch / Français / Español。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    ZhHans,
    ZhHant,
    En,
    Ja,
    Ko,
    Ru,
    De,
    Fr,
    Es,
}

impl Lang {
    pub const ALL: [Lang; 9] = [
        Lang::ZhHans,
        Lang::ZhHant,
        Lang::En,
        Lang::Ja,
        Lang::Ko,
        Lang::Ru,
        Lang::De,
        Lang::Fr,
        Lang::Es,
    ];

    /// 本语言名称（下拉显示用）。
    pub fn native(&self) -> &'static str {
        match self {
            Lang::ZhHans => "简体中文",
            Lang::ZhHant => "繁體中文",
            Lang::En => "English",
            Lang::Ja => "日本語",
            Lang::Ko => "한국어",
            Lang::Ru => "Русский",
            Lang::De => "Deutsch",
            Lang::Fr => "Français",
            Lang::Es => "Español",
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Lang::ZhHans => "zh-Hans",
            Lang::ZhHant => "zh-Hant",
            Lang::En => "en",
            Lang::Ja => "ja",
            Lang::Ko => "ko",
            Lang::Ru => "ru",
            Lang::De => "de",
            Lang::Fr => "fr",
            Lang::Es => "es",
        }
    }

    pub fn from_code(s: &str) -> Lang {
        match s {
            "zh-Hant" => Lang::ZhHant,
            "en" => Lang::En,
            "ja" => Lang::Ja,
            "ko" => Lang::Ko,
            "ru" => Lang::Ru,
            "de" => Lang::De,
            "fr" => Lang::Fr,
            "es" => Lang::Es,
            _ => Lang::ZhHans,
        }
    }

    fn col(&self) -> usize {
        match self {
            Lang::ZhHans => 0,
            Lang::ZhHant => 1,
            Lang::En => 2,
            Lang::Ja => 3,
            Lang::Ko => 4,
            Lang::Ru => 5,
            Lang::De => 6,
            Lang::Fr => 7,
            Lang::Es => 8,
        }
    }
}

/// 翻译表：key × 9 语言（列顺序 = Lang::col）。
pub fn tr(lang: Lang, key: &str) -> String {
    let col = lang.col();
    for row in TABLE {
        if row.0 == key {
            return row.1[col].to_string();
        }
    }
    key.to_string()
}

// ---------- 键与译文 ----------
// 顺序：简体 / 繁體 / EN / 日 / 한 / Ру / De / Fr / Es
#[rustfmt::skip]
static TABLE: &[(&str, [&str; 9])] = &[
    // 导航
    ("nav.dashboard", ["仪表盘","儀表盤","Dashboard","ダッシュボード","대시보드","Панель","Übersicht","Tableau de bord","Panel"]),
    ("nav.processes", ["进程","進程","Processes","プロセス","프로세스","Процессы","Prozesse","Processus","Procesos"]),
    ("nav.services",  ["所有服务","所有服務","Services","サービス","서비스","Службы","Dienste","Services","Servicios"]),
    ("nav.logs",      ["日志","日誌","Logs","ログ","로그","Журналы","Logs","Journaux","Registros"]),
    ("nav.api",       ["对外接口","對外接口","Interfaces","インターフェース","인터페이스","Интерфейсы","Schnittstellen","Interfaces","Interfaces"]),
    ("nav.docs",      ["使用说明","使用說明","Guide","ガイド","안내","Руководство","Anleitung","Guide","Guía"]),
    ("nav.help",      ["帮助","幫助","Help","ヘルプ","도움말","Справка","Hilfe","Aide","Ayuda"]),
    ("nav.settings",  ["设置","設定","Settings","設定","설정","Настройки","Einstellungen","Réglages","Ajustes"]),
    // 通用动作
    ("c.add",      ["＋ 添加进程","＋ 新增進程","＋ Add Process","＋ 追加","＋ 추가","＋ Добавить","＋ Hinzufügen","＋ Ajouter","＋ Añadir"]),
    ("c.start",    ["启动","啟動","Start","開始","시작","Запустить","Start","Démarrer","Iniciar"]),
    ("c.stop",     ["停止","停止","Stop","停止","정지","Остановить","Stop","Arrêter","Detener"]),
    ("c.check",    ["检查","檢查","Check","チェック","검사","Проверить","Prüfen","Vérifier","Comprobar"]),
    ("c.edit",     ["编辑","編輯","Edit","編集","편집","Изменить","Bearbeiten","Modifier","Editar"]),
    ("c.delete",   ["删除","刪除","Delete","削除","삭제","Удалить","Löschen","Supprimer","Eliminar"]),
    ("c.copy",     ["复制","複製","Copy","コピー","복사","Копировать","Kopieren","Copier","Copiar"]),
    ("c.open",     ["打开","打開","Open","開く","열기","Открыть","Öffnen","Ouvrir","Abrir"]),
    ("c.save",     ["保存","儲存","Save","保存","저장","Сохранить","Speichern","Enregistrer","Guardar"]),
    ("c.cancel",   ["取消","取消","Cancel","キャンセル","취소","Отмена","Abbrechen","Annuler","Cancelar"]),
    ("c.confirm",  ["确认删除","確認刪除","Delete","削除する","삭제 확인","Удалить","Löschen","Supprimer","Eliminar"]),
    ("c.query",    ["查询","查詢","Query","検索","조회","Запрос","Abfragen","Requête","Consultar"]),
    ("c.prev",     ["上一页","上一頁","Prev","前へ","이전","Назад","Zurück","Préc.","Anterior"]),
    ("c.next",     ["下一页","下一頁","Next","次へ","다음","Далее","Weiter","Suiv.","Siguiente"]),
    // 状态
    ("s.running",  ["运行中","執行中","Running","実行中","실행 중","Работает","Läuft","En cours","Activo"]),
    ("s.stopped",  ["已停止","已停止","Stopped","停止","중지","Остановлен","Gestoppt","Arrêté","Detenido"]),
    ("s.error",    ["异常","異常","Error","異常","오류","Ошибка","Fehler","Erreur","Error"]),
    ("s.starting", ["启动中","啟動中","Starting","開始中","시작 중","Запуск","Startet","Démarrage","Iniciando"]),
    ("s.stopping", ["停止中","停止中","Stopping","停止中","중지 중","Остановка","Stoppt","Arrêt","Deteniendo"]),
    // 仪表盘
    ("d.cpu",    ["CPU 使用率","CPU 使用率","CPU Usage","CPU 使用率","CPU 사용률","ЦП","CPU-Last","CPU","CPU"]),
    ("d.mem",    ["内存","記憶體","Memory","メモリ","메모리","Память","Speicher","Mémoire","Memoria"]),
    ("d.disk",   ["磁盘","磁碟","Disk","ディスク","디스크","Диск","Festplatte","Disque","Disco"]),
    ("d.net",    ["网络","網路","Network","ネットワーク","네트워크","Сеть","Netzwerk","Réseau","Red"]),
    ("d.gpu",    ["GPU（暂不支持）","GPU（暫不支援）","GPU (N/A)","GPU（未対応）","GPU(미지원)","ГП (нет данных)","GPU (n. v.)","GPU (indisp.)","GPU (n/d)"]),
    ("d.procs",  ["已配置进程","已設定進程","Processes","プロセス","프로세스","Процессы","Prozesse","Processus","Procesos"]),
    ("d.running",["运行中进程","執行中進程","Running","実行中","실행 중","Работает","Aktiv","En cours","Activos"]),
    ("d.svcs",   ["Web 服务","Web 服務","Web Services","Webサービス","Web 서비스","Web-сервисы","Web-Dienste","Services web","Servicios web"]),
    ("d.api",    ["接口监听","介面監聽","API Status","API状態","API 상태","API-статус","API-Status","État API","Estado API"]),
    ("d.hint",   ["仪表盘为只读概览；操作入口在「进程」页","儀表盤為唯讀概覽；操作入口在「進程」頁","Read-only overview; manage in Processes","読み取り専用。操作はプロセス画面で","읽기 전용 개요","Обзор только для чтения","Nur-Lese-Übersicht","Vue d'ensemble (lecture seule)","Resumen de solo lectura"]),
    // 进程页
    ("p.empty",  ["还没有配置进程。点击「添加进程」，或只填「系统观察名」观察系统进程。","還沒有設定進程。點擊「新增進程」，或只填「系統觀察名」觀察系統進程。","No processes yet. Add one, or set an observe name to watch system processes.","プロセス未設定。追加するか、観察名でシステムプロセスを監視。","프로세스가 없습니다. 추가하거나 관찰 이름을 설정하세요.","Процессов нет. Добавьте или укажите имя наблюдения.","Keine Prozesse. Hinzufügen oder beobachten.","Aucun processus. Ajoutez-en.","Sin procesos todavía."]),
    ("p.auto",   ["状态自动刷新","狀態自動重新整理","Auto refresh","自動更新","자동 새로고침","Автообновление","Auto-Aktualisierung","Actualisation auto","Actualización auto"]),
    ("p.delwarn",["⚠ 该进程正在运行：删除仅移除配置，建议先停止。","⚠ 該進程正在執行：刪除僅移除設定，建議先停止。","⚠ Running: deleting only removes config; stop it first.","⚠ 実行中：設定のみ削除。停止を推奨。","⚠ 실행 중: 설정만 삭제됨. 먼저 중지 권장.","⚠ Работает: удаление уберёт только конфиг.","⚠ Läuft: nur Konfig wird entfernt.","⚠ En cours : suppression de la config.","⚠ Activo: solo borra la config."]),
    ("p.deltitle",["删除进程配置","刪除進程設定","Delete process config","プロセス設定を削除","프로세스 설정 삭제","Удалить конфигурацию","Prozesskonfig löschen","Supprimer la config","Borrar config"]),
    // 服务页
    ("sv.hint",  ["集中展示由黄瓜管理的运行中 Web 服务地址。","集中展示由黃瓜管理的執行中 Web 服務位址。","Managed running web services.","管理中の実行Webサービス。","관리 중인 실행 Web 서비스.","Управляемые веб-сервисы.","Verwaltete Web-Dienste.","Services web gérés.","Servicios web gestionados."]),
    ("sv.empty", ["当前没有运行中的 Web 服务。","目前沒有執行中的 Web 服務。","No running web services.","実行中のサービスなし。","실행 중인 서비스 없음.","Нет активных сервисов.","Keine aktiven Dienste.","Aucun service actif.","Sin servicios activos."]),
    ("sv.copy",  ["复制 URL","複製 URL","Copy URL","URLをコピー","URL 복사","Копировать URL","URL kopieren","Copier l'URL","Copiar URL"]),
    // 日志页
    ("l.search", ["搜索消息体（子串）","搜尋訊息內容（子串）","Search message","メッセージ検索","메시지 검색","Поиск в сообщении","Meldung suchen","Rechercher","Buscar mensaje"]),
    ("l.tag",    ["标签（子串）","標籤（子串）","Tag","タグ","태그","Тег","Tag","Étiquette","Etiqueta"]),
    ("l.level",  ["级别：","級別：","Level:","レベル:","레벨:","Уровень:","Stufe:","Niveau:","Nivel:"]),
    ("l.follow", ["跟随","跟隨","Follow","追従","따라가기","Следить","Folgen","Suivre","Seguir"]),
    ("l.empty",  ["无匹配行。调整过滤条件后点击「查询」。","無符合行。調整過濾後點「查詢」。","No matches. Adjust filters and query.","一致なし。条件を調整して検索。","일치 없음. 조건 조정 후 조회.","Нет совпадений.","Keine Treffer.","Aucun résultat.","Sin coincidencias."]),
    // 对外接口页
    ("a.listen", ["监听状态（仅 127.0.0.1）","監聽狀態（僅 127.0.0.1）","Listening (127.0.0.1 only)","リッスン状態（127.0.0.1）","수신 상태(127.0.0.1)","Слушает (127.0.0.1)","Lauscht (127.0.0.1)","Écoute (127.0.0.1)","Escuchando (127.0.0.1)"]),
    ("a.token",  ["本地令牌（Authorization: Bearer … 或 ?token=…）","本地權杖（Authorization: Bearer … 或 ?token=…）","Local token (Bearer / ?token=)","ローカルトークン","로컬 토큰","Локальный токен","Lokales Token","Jeton local","Token local"]),
    ("a.showhide",["显示/隐藏","顯示/隱藏","Show/Hide","表示/非表示","표시/숨기기","Показать","Zeigen/Verbergen","Afficher/Masquer","Mostrar/Ocultar"]),
    ("a.copytok",["复制令牌","複製權杖","Copy token","トークンをコピー","토큰 복사","Копировать токен","Token kopieren","Copier le jeton","Copiar token"]),
    ("a.skill",  ["Skill 一键安装","Skill 一鍵安裝","Install Skill","Skill インストール","Skill 설치","Установить Skill","Skill installieren","Installer Skill","Instalar Skill"]),
    ("a.skillbtn",["安装 skill","安裝 skill","Install","インストール","설치","Установить","Installieren","Installer","Instalar"]),
    ("a.mcp",    ["MCP 接入","MCP 接入","MCP Access","MCP 接続","MCP 연결","Доступ MCP","MCP-Zugang","Accès MCP","Acceso MCP"]),
    ("a.ai",     ["复制 AI 配置提示","複製 AI 設定提示","Copy AI setup prompt","AI設定プロンプトをコピー","AI 설정 프롬프트 복사","Скопировать промпт","AI-Prompt kopieren","Copier l'invite AI","Copiar prompt AI"]),
    ("a.aihint", ["复制完整配置提示词（含令牌与端口）发给任意 AI 助手，即可自动完成接入。","複製完整設定提示詞（含權杖與埠）發給任意 AI 助手，即可自動完成接入。","Copy the full setup prompt (token & ports included) to any AI assistant.","完全な設定プロンプト（トークン・ポート含む）をAIに渡すだけで接続完了。","전체 설정 프롬프트(토큰·포트 포함)를 AI에게 복사해 전달.","Передайте полный промпт (токен и порты) любому ИИ-агенту.","Vollständigen Prompt (Token & Ports) an einen KI-Assistenten übergeben.","Donnez l'invite complète (jeton et ports) à une IA.","Entrega el prompt completo (token y puertos) a una IA."]),
    // 设置页
    ("st.appearance",["外观","外觀","Appearance","外観","화면","Оформление","Erscheinungsbild","Apparence","Apariencia"]),
    ("st.theme",    ["主题","主題","Theme","テーマ","테마","Тема","Design","Thème","Tema"]),
    ("st.light",    ["浅色","淺色","Light","ライト","라이트","Светлая","Hell","Clair","Claro"]),
    ("st.dark",     ["深色","深色","Dark","ダーク","다크","Тёмная","Dunkel","Sombre","Oscuro"]),
    ("st.system",   ["跟随系统","跟隨系統","System","システム","시스템","Системная","System","Système","Sistema"]),
    ("st.font",     ["字体","字型","Font family","フォント","글꼴","Шрифт","Schrift","Police","Fuente"]),
    ("st.fontdef",  ["默认（平台）","預設（平台）","Default (platform)","既定（プラットフォーム）","기본(플랫폼)","По умолчанию","Standard","Par défaut","Predeterminada"]),
    ("st.fontsize", ["字体缩放","字體縮放","Font scale","文字サイズ","글자 크기","Масштаб шрифта","Schriftgröße","Taille","Escala"]),
    ("st.bg",       ["窗口背景色（含透明度）","視窗背景色（含透明度）","Window background (alpha)","ウィンドウ背景色","창 배경색","Фон окна","Fensterfarbe","Fenêtre (alpha)","Fondo de ventana"]),
    ("st.bghint",   ["透明度只影响应用内绘制层","透明度只影響應用內繪製層","Alpha affects in-app layers only","透過度はアプリ内描画のみ","투명도는 앱 내 레이어에만 적용","Прозрачность — внутри окна","Alpha nur app-intern","Alpha interne à l'app","Alfa solo interna"]),
    ("st.tray",     ["系统托盘图标","系統工作列圖示","System tray icon","システムトレイ","시스템 트레이","Значок в трее","Tray-Symbol","Icône de barre","Icono de bandeja"]),
    ("st.trayhint", ["开启后在托盘显示统计信息（Linux 桌面不支持时自动忽略；重启应用后生效）","開啟後在工作列顯示統計資訊（Linux 桌面不支援時自動忽略；重啟應用後生效）","Show tray stats (ignored on unsupported desktops; takes effect after restart)","有効化で統計を表示（非対応デスクトップでは無視、再起動で反映）","켜면 통계 표시(미지원 데스크톱은 무시, 재시작 후 적용)","Статистика в трее (перезапуск)","Statistik im Tray (Neustart)","Stats dans la barre (redémarrage)","Stats en bandeja (reiniciar)"]),
    ("st.lang",     ["语言","語言","Language","言語","언어","Язык","Sprache","Langue","Idioma"]),
    ("st.note",     ["设置即时保存到配置文件","設定即時儲存到設定檔","Saved to config instantly","設定は即時保存","즉시 저장됨","Сохраняется сразу","Sofort gespeichert","Enregistré aussitôt","Se guarda al instante"]),
    // 帮助页
    ("h.about",  ["关于黄瓜（cucumber）","關於黃瓜（cucumber）","About cucumber","cucumber について","cucumber 정보","О cucumber","Über cucumber","À propos","Acerca de"]),
    ("h.repo",   ["开源仓库","開源倉庫","Repository","リポジトリ","저장소","Репозиторий","Repository","Dépôt","Repositorio"]),
    ("h.author", ["作者","作者","Author","作者","작성자","Автор","Autor","Auteur","Autor"]),
    ("h.env",    ["环境信息","環境資訊","Environment","環境","환경","Среда","Umgebung","Environnement","Entorno"]),
    ("h.faq",    ["常见问题","常見問題","FAQ","よくある質問","자주 묻는 질문","Вопросы","FAQ","FAQ","Preguntas"]),
    ("h.diag",   ["快速诊断","快速診斷","Diagnostics","診断","진단","Диагностика","Diagnose","Diagnostics","Diagnóstico"]),
    ("h.diagbtn",["检查本地接口可达性","檢查本地介面可達性","Check local API","API到達確認","API 확인","Проверить API","API prüfen","Vérifier l'API","Comprobar API"]),
    // 更新
    ("u.title",  ["发现新版本","發現新版本","Update available","新バージョン","새 버전","Доступно обновление","Update verfügbar","Mise à jour","Nueva versión"]),
    ("u.now",    ["立即更新","立即更新","Update now","今すぐ更新","지금 업데이트","Обновить","Jetzt aktualisieren","Mettre à jour","Actualizar"]),
    ("u.later",  ["暂不更新","暫不更新","Later","後で","나중에","Позже","Später","Plus tard","Más tarde"]),
    ("u.skip",   ["跳过此版本","跳過此版本","Skip this version","この版をスキップ","이 버전 건너뛰기","Пропустить","Diese Version überspringen","Ignorer cette version","Omitir versión"]),
    ("u.gotodl", ["已打开下载页，请下载新版本安装包。","已開啟下載頁，請下載新版本安裝包。","Download page opened.","ダウンロードページを開きました。","다운로드 페이지를 열었습니다.","Открыта страница загрузок.","Downloadseite geöffnet.","Page de téléchargement ouverte.","Página de descargas abierta."]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_shape() {
        assert!(TABLE.len() > 70);
        for (k, row) in TABLE {
            assert_eq!(row.len(), 9, "key {k} 语言列数错误");
            assert!(row.iter().all(|s| !s.is_empty()), "key {k} 有空译文");
        }
    }

    #[test]
    fn native_names() {
        assert_eq!(Lang::En.native(), "English");
        assert_eq!(Lang::Ja.native(), "日本語");
        assert_eq!(Lang::Ko.native(), "한국어");
        assert_eq!(Lang::Ru.native(), "Русский");
        assert_eq!(Lang::De.native(), "Deutsch");
        assert_eq!(Lang::Fr.native(), "Français");
        assert_eq!(Lang::Es.native(), "Español");
        assert_eq!(Lang::from_code("zh-Hant").native(), "繁體中文");
    }

    #[test]
    fn tr_lookup() {
        assert_eq!(tr(Lang::En, "c.start"), "Start");
        assert_eq!(tr(Lang::ZhHans, "nav.dashboard"), "仪表盘");
        assert_eq!(tr(Lang::Ja, "l.follow"), "追従");
    }
}
