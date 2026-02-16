//! Message Bundles
//!
//! Localized message storage and lookup.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Message bundle
#[derive(Debug, Clone, Default)]
pub struct MessageBundle {
    /// Locale
    locale: String,
    /// Messages
    messages: BTreeMap<String, String>,
    /// Plural rules
    plurals: BTreeMap<String, PluralMessage>,
}

impl MessageBundle {
    /// Create new bundle
    pub fn new(locale: &str) -> Self {
        Self {
            locale: locale.to_string(),
            messages: BTreeMap::new(),
            plurals: BTreeMap::new(),
        }
    }

    /// Add message
    pub fn add(&mut self, key: &str, value: &str) {
        self.messages.insert(key.to_string(), value.to_string());
    }

    /// Add plural message
    pub fn add_plural(&mut self, key: &str, plural: PluralMessage) {
        self.plurals.insert(key.to_string(), plural);
    }

    /// Get message
    pub fn get(&self, key: &str) -> Option<&str> {
        self.messages.get(key).map(|s| s.as_str())
    }

    /// Get plural message
    pub fn get_plural(&self, key: &str, count: i64) -> Option<&str> {
        self.plurals.get(key).map(|p| p.get(count))
    }

    /// Has message
    pub fn has(&self, key: &str) -> bool {
        self.messages.contains_key(key)
    }

    /// Get all keys
    pub fn keys(&self) -> Vec<&str> {
        self.messages.keys().map(|s| s.as_str()).collect()
    }

    /// Locale
    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// Message count
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Is empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Merge with another bundle
    pub fn merge(&mut self, other: &MessageBundle) {
        for (key, value) in &other.messages {
            self.messages.insert(key.clone(), value.clone());
        }
        for (key, plural) in &other.plurals {
            self.plurals.insert(key.clone(), plural.clone());
        }
    }
}

/// Plural message
#[derive(Debug, Clone)]
pub struct PluralMessage {
    /// Zero form
    pub zero: Option<String>,
    /// One form (singular)
    pub one: String,
    /// Two form (dual)
    pub two: Option<String>,
    /// Few form
    pub few: Option<String>,
    /// Many form
    pub many: Option<String>,
    /// Other form (plural default)
    pub other: String,
}

impl PluralMessage {
    /// Create simple singular/plural
    pub fn simple(one: &str, other: &str) -> Self {
        Self {
            zero: None,
            one: one.to_string(),
            two: None,
            few: None,
            many: None,
            other: other.to_string(),
        }
    }

    /// Get appropriate form
    pub fn get(&self, count: i64) -> &str {
        match count.abs() {
            0 => self.zero.as_deref().unwrap_or(&self.other),
            1 => &self.one,
            2 => self.two.as_deref().unwrap_or(&self.other),
            3..=10 => self.few.as_deref().unwrap_or(&self.other),
            11..=99 => self.many.as_deref().unwrap_or(&self.other),
            _ => &self.other,
        }
    }
}

/// Message formatter
pub struct MessageFormatter {
    /// Escape HTML
    escape_html: bool,
}

impl MessageFormatter {
    /// Create new formatter
    pub fn new() -> Self {
        Self { escape_html: true }
    }

    /// Format message with arguments
    pub fn format(&self, template: &str, args: &BTreeMap<String, String>) -> String {
        let mut result = template.to_string();

        for (key, value) in args {
            let placeholder = alloc::format!("{{{}}}", key);
            let safe_value = if self.escape_html {
                self.escape(value)
            } else {
                value.clone()
            };
            result = result.replace(&placeholder, &safe_value);
        }

        result
    }

    /// Escape HTML entities
    fn escape(&self, text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        for c in text.chars() {
            match c {
                '<' => result.push_str("&lt;"),
                '>' => result.push_str("&gt;"),
                '&' => result.push_str("&amp;"),
                '"' => result.push_str("&quot;"),
                '\'' => result.push_str("&#39;"),
                _ => result.push(c),
            }
        }
        result
    }

    /// Set escape HTML
    pub fn set_escape_html(&mut self, escape: bool) {
        self.escape_html = escape;
    }
}

impl Default for MessageFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Standard browser messages
pub struct BrowserMessages;

impl BrowserMessages {
    // Navigation
    pub const BACK: &'static str = "navigation.back";
    pub const FORWARD: &'static str = "navigation.forward";
    pub const RELOAD: &'static str = "navigation.reload";
    pub const STOP: &'static str = "navigation.stop";
    pub const HOME: &'static str = "navigation.home";

    // Actions
    pub const COPY: &'static str = "action.copy";
    pub const PASTE: &'static str = "action.paste";
    pub const CUT: &'static str = "action.cut";
    pub const UNDO: &'static str = "action.undo";
    pub const REDO: &'static str = "action.redo";
    pub const SELECT_ALL: &'static str = "action.select_all";

    // Tabs
    pub const NEW_TAB: &'static str = "tabs.new";
    pub const CLOSE_TAB: &'static str = "tabs.close";
    pub const REOPEN_TAB: &'static str = "tabs.reopen";

    // Bookmarks
    pub const ADD_BOOKMARK: &'static str = "bookmarks.add";
    pub const REMOVE_BOOKMARK: &'static str = "bookmarks.remove";
    pub const EDIT_BOOKMARK: &'static str = "bookmarks.edit";

    // Errors
    pub const ERR_NOT_FOUND: &'static str = "error.not_found";
    pub const ERR_CONNECTION: &'static str = "error.connection";
    pub const ERR_CERTIFICATE: &'static str = "error.certificate";
    pub const ERR_TIMEOUT: &'static str = "error.timeout";

    // Dialogs
    pub const OK: &'static str = "dialog.ok";
    pub const CANCEL: &'static str = "dialog.cancel";
    pub const YES: &'static str = "dialog.yes";
    pub const NO: &'static str = "dialog.no";
    pub const SAVE: &'static str = "dialog.save";
    pub const CLOSE: &'static str = "dialog.close";
}

/// Create English message bundle
pub fn english_bundle() -> MessageBundle {
    let mut bundle = MessageBundle::new("en-US");

    // Navigation
    bundle.add(BrowserMessages::BACK, "Back");
    bundle.add(BrowserMessages::FORWARD, "Forward");
    bundle.add(BrowserMessages::RELOAD, "Reload");
    bundle.add(BrowserMessages::STOP, "Stop");
    bundle.add(BrowserMessages::HOME, "Home");

    // Actions
    bundle.add(BrowserMessages::COPY, "Copy");
    bundle.add(BrowserMessages::PASTE, "Paste");
    bundle.add(BrowserMessages::CUT, "Cut");
    bundle.add(BrowserMessages::UNDO, "Undo");
    bundle.add(BrowserMessages::REDO, "Redo");
    bundle.add(BrowserMessages::SELECT_ALL, "Select All");

    // Dialogs
    bundle.add(BrowserMessages::OK, "OK");
    bundle.add(BrowserMessages::CANCEL, "Cancel");
    bundle.add(BrowserMessages::YES, "Yes");
    bundle.add(BrowserMessages::NO, "No");
    bundle.add(BrowserMessages::SAVE, "Save");
    bundle.add(BrowserMessages::CLOSE, "Close");

    // Tabs
    bundle.add(BrowserMessages::NEW_TAB, "New Tab");
    bundle.add(BrowserMessages::CLOSE_TAB, "Close Tab");
    bundle.add(BrowserMessages::REOPEN_TAB, "Reopen Closed Tab");

    // Bookmarks
    bundle.add(BrowserMessages::ADD_BOOKMARK, "Add Bookmark");
    bundle.add(BrowserMessages::REMOVE_BOOKMARK, "Remove Bookmark");
    bundle.add(BrowserMessages::EDIT_BOOKMARK, "Edit Bookmark");

    // Errors
    bundle.add(BrowserMessages::ERR_NOT_FOUND, "Page not found");
    bundle.add(BrowserMessages::ERR_CONNECTION, "Unable to connect");
    bundle.add(
        BrowserMessages::ERR_CERTIFICATE,
        "Security certificate error",
    );
    bundle.add(BrowserMessages::ERR_TIMEOUT, "Connection timed out");

    bundle
}

/// Create Korean message bundle
pub fn korean_bundle() -> MessageBundle {
    let mut bundle = MessageBundle::new("ko-KR");

    // Navigation
    bundle.add(BrowserMessages::BACK, "뒤로");
    bundle.add(BrowserMessages::FORWARD, "앞으로");
    bundle.add(BrowserMessages::RELOAD, "새로고침");
    bundle.add(BrowserMessages::STOP, "중지");
    bundle.add(BrowserMessages::HOME, "홈");

    // Actions
    bundle.add(BrowserMessages::COPY, "복사");
    bundle.add(BrowserMessages::PASTE, "붙여넣기");
    bundle.add(BrowserMessages::CUT, "잘라내기");
    bundle.add(BrowserMessages::UNDO, "실행 취소");
    bundle.add(BrowserMessages::REDO, "다시 실행");
    bundle.add(BrowserMessages::SELECT_ALL, "전체 선택");

    // Dialogs
    bundle.add(BrowserMessages::OK, "확인");
    bundle.add(BrowserMessages::CANCEL, "취소");
    bundle.add(BrowserMessages::YES, "예");
    bundle.add(BrowserMessages::NO, "아니오");
    bundle.add(BrowserMessages::SAVE, "저장");
    bundle.add(BrowserMessages::CLOSE, "닫기");

    // Tabs
    bundle.add(BrowserMessages::NEW_TAB, "새 탭");
    bundle.add(BrowserMessages::CLOSE_TAB, "탭 닫기");
    bundle.add(BrowserMessages::REOPEN_TAB, "탭 다시 열기");

    // Bookmarks
    bundle.add(BrowserMessages::ADD_BOOKMARK, "북마크 추가");
    bundle.add(BrowserMessages::REMOVE_BOOKMARK, "북마크 삭제");
    bundle.add(BrowserMessages::EDIT_BOOKMARK, "북마크 편집");

    // Errors
    bundle.add(BrowserMessages::ERR_NOT_FOUND, "페이지를 찾을 수 없습니다");
    bundle.add(BrowserMessages::ERR_CONNECTION, "연결할 수 없습니다");
    bundle.add(BrowserMessages::ERR_CERTIFICATE, "보안 인증서 오류");
    bundle.add(BrowserMessages::ERR_TIMEOUT, "연결 시간 초과");

    bundle
}

/// Create Japanese message bundle
pub fn japanese_bundle() -> MessageBundle {
    let mut bundle = MessageBundle::new("ja-JP");

    bundle.add(BrowserMessages::BACK, "戻る");
    bundle.add(BrowserMessages::FORWARD, "進む");
    bundle.add(BrowserMessages::RELOAD, "再読み込み");
    bundle.add(BrowserMessages::STOP, "中止");
    bundle.add(BrowserMessages::HOME, "ホーム");

    bundle.add(BrowserMessages::COPY, "コピー");
    bundle.add(BrowserMessages::PASTE, "貼り付け");
    bundle.add(BrowserMessages::CUT, "切り取り");
    bundle.add(BrowserMessages::UNDO, "元に戻す");
    bundle.add(BrowserMessages::REDO, "やり直し");
    bundle.add(BrowserMessages::SELECT_ALL, "すべて選択");

    bundle.add(BrowserMessages::OK, "OK");
    bundle.add(BrowserMessages::CANCEL, "キャンセル");
    bundle.add(BrowserMessages::YES, "はい");
    bundle.add(BrowserMessages::NO, "いいえ");
    bundle.add(BrowserMessages::SAVE, "保存");
    bundle.add(BrowserMessages::CLOSE, "閉じる");

    bundle.add(BrowserMessages::NEW_TAB, "新しいタブ");
    bundle.add(BrowserMessages::CLOSE_TAB, "タブを閉じる");
    bundle.add(BrowserMessages::REOPEN_TAB, "タブを再び開く");

    bundle.add(BrowserMessages::ADD_BOOKMARK, "ブックマーク追加");
    bundle.add(BrowserMessages::REMOVE_BOOKMARK, "ブックマーク削除");
    bundle.add(BrowserMessages::EDIT_BOOKMARK, "ブックマーク編集");

    bundle.add(BrowserMessages::ERR_NOT_FOUND, "ページが見つかりません");
    bundle.add(BrowserMessages::ERR_CONNECTION, "接続できません");
    bundle.add(BrowserMessages::ERR_CERTIFICATE, "セキュリティ証明書エラー");
    bundle.add(BrowserMessages::ERR_TIMEOUT, "接続がタイムアウトしました");

    bundle
}

/// Create Chinese Simplified message bundle
pub fn chinese_simplified_bundle() -> MessageBundle {
    let mut bundle = MessageBundle::new("zh-CN");

    bundle.add(BrowserMessages::BACK, "后退");
    bundle.add(BrowserMessages::FORWARD, "前进");
    bundle.add(BrowserMessages::RELOAD, "刷新");
    bundle.add(BrowserMessages::STOP, "停止");
    bundle.add(BrowserMessages::HOME, "主页");

    bundle.add(BrowserMessages::COPY, "复制");
    bundle.add(BrowserMessages::PASTE, "粘贴");
    bundle.add(BrowserMessages::CUT, "剪切");
    bundle.add(BrowserMessages::UNDO, "撤销");
    bundle.add(BrowserMessages::REDO, "重做");
    bundle.add(BrowserMessages::SELECT_ALL, "全选");

    bundle.add(BrowserMessages::OK, "确定");
    bundle.add(BrowserMessages::CANCEL, "取消");
    bundle.add(BrowserMessages::YES, "是");
    bundle.add(BrowserMessages::NO, "否");
    bundle.add(BrowserMessages::SAVE, "保存");
    bundle.add(BrowserMessages::CLOSE, "关闭");

    bundle.add(BrowserMessages::NEW_TAB, "新标签页");
    bundle.add(BrowserMessages::CLOSE_TAB, "关闭标签页");
    bundle.add(BrowserMessages::REOPEN_TAB, "重新打开标签页");

    bundle.add(BrowserMessages::ADD_BOOKMARK, "添加书签");
    bundle.add(BrowserMessages::REMOVE_BOOKMARK, "删除书签");
    bundle.add(BrowserMessages::EDIT_BOOKMARK, "编辑书签");

    bundle.add(BrowserMessages::ERR_NOT_FOUND, "找不到页面");
    bundle.add(BrowserMessages::ERR_CONNECTION, "无法连接");
    bundle.add(BrowserMessages::ERR_CERTIFICATE, "安全证书错误");
    bundle.add(BrowserMessages::ERR_TIMEOUT, "连接超时");

    bundle
}

/// Create Spanish message bundle
pub fn spanish_bundle() -> MessageBundle {
    let mut bundle = MessageBundle::new("es-ES");

    bundle.add(BrowserMessages::BACK, "Atrás");
    bundle.add(BrowserMessages::FORWARD, "Adelante");
    bundle.add(BrowserMessages::RELOAD, "Recargar");
    bundle.add(BrowserMessages::STOP, "Detener");
    bundle.add(BrowserMessages::HOME, "Inicio");

    bundle.add(BrowserMessages::COPY, "Copiar");
    bundle.add(BrowserMessages::PASTE, "Pegar");
    bundle.add(BrowserMessages::CUT, "Cortar");
    bundle.add(BrowserMessages::UNDO, "Deshacer");
    bundle.add(BrowserMessages::REDO, "Rehacer");
    bundle.add(BrowserMessages::SELECT_ALL, "Seleccionar todo");

    bundle.add(BrowserMessages::OK, "Aceptar");
    bundle.add(BrowserMessages::CANCEL, "Cancelar");
    bundle.add(BrowserMessages::YES, "Sí");
    bundle.add(BrowserMessages::NO, "No");
    bundle.add(BrowserMessages::SAVE, "Guardar");
    bundle.add(BrowserMessages::CLOSE, "Cerrar");

    bundle.add(BrowserMessages::NEW_TAB, "Nueva pestaña");
    bundle.add(BrowserMessages::CLOSE_TAB, "Cerrar pestaña");
    bundle.add(BrowserMessages::REOPEN_TAB, "Reabrir pestaña");

    bundle.add(BrowserMessages::ADD_BOOKMARK, "Añadir marcador");
    bundle.add(BrowserMessages::REMOVE_BOOKMARK, "Eliminar marcador");
    bundle.add(BrowserMessages::EDIT_BOOKMARK, "Editar marcador");

    bundle.add(BrowserMessages::ERR_NOT_FOUND, "Página no encontrada");
    bundle.add(BrowserMessages::ERR_CONNECTION, "No se puede conectar");
    bundle.add(
        BrowserMessages::ERR_CERTIFICATE,
        "Error de certificado de seguridad",
    );
    bundle.add(BrowserMessages::ERR_TIMEOUT, "Tiempo de conexión agotado");

    bundle
}

/// Create German message bundle
pub fn german_bundle() -> MessageBundle {
    let mut bundle = MessageBundle::new("de-DE");

    bundle.add(BrowserMessages::BACK, "Zurück");
    bundle.add(BrowserMessages::FORWARD, "Vor");
    bundle.add(BrowserMessages::RELOAD, "Neu laden");
    bundle.add(BrowserMessages::STOP, "Stopp");
    bundle.add(BrowserMessages::HOME, "Startseite");

    bundle.add(BrowserMessages::COPY, "Kopieren");
    bundle.add(BrowserMessages::PASTE, "Einfügen");
    bundle.add(BrowserMessages::CUT, "Ausschneiden");
    bundle.add(BrowserMessages::UNDO, "Rückgängig");
    bundle.add(BrowserMessages::REDO, "Wiederherstellen");
    bundle.add(BrowserMessages::SELECT_ALL, "Alles auswählen");

    bundle.add(BrowserMessages::OK, "OK");
    bundle.add(BrowserMessages::CANCEL, "Abbrechen");
    bundle.add(BrowserMessages::YES, "Ja");
    bundle.add(BrowserMessages::NO, "Nein");
    bundle.add(BrowserMessages::SAVE, "Speichern");
    bundle.add(BrowserMessages::CLOSE, "Schließen");

    bundle.add(BrowserMessages::NEW_TAB, "Neuer Tab");
    bundle.add(BrowserMessages::CLOSE_TAB, "Tab schließen");
    bundle.add(BrowserMessages::REOPEN_TAB, "Tab wiederherstellen");

    bundle.add(BrowserMessages::ADD_BOOKMARK, "Lesezeichen hinzufügen");
    bundle.add(BrowserMessages::REMOVE_BOOKMARK, "Lesezeichen entfernen");
    bundle.add(BrowserMessages::EDIT_BOOKMARK, "Lesezeichen bearbeiten");

    bundle.add(BrowserMessages::ERR_NOT_FOUND, "Seite nicht gefunden");
    bundle.add(BrowserMessages::ERR_CONNECTION, "Verbindung nicht möglich");
    bundle.add(
        BrowserMessages::ERR_CERTIFICATE,
        "Sicherheitszertifikatfehler",
    );
    bundle.add(
        BrowserMessages::ERR_TIMEOUT,
        "Zeitüberschreitung der Verbindung",
    );

    bundle
}

/// Create French message bundle
pub fn french_bundle() -> MessageBundle {
    let mut bundle = MessageBundle::new("fr-FR");

    bundle.add(BrowserMessages::BACK, "Retour");
    bundle.add(BrowserMessages::FORWARD, "Avancer");
    bundle.add(BrowserMessages::RELOAD, "Actualiser");
    bundle.add(BrowserMessages::STOP, "Arrêter");
    bundle.add(BrowserMessages::HOME, "Accueil");

    bundle.add(BrowserMessages::COPY, "Copier");
    bundle.add(BrowserMessages::PASTE, "Coller");
    bundle.add(BrowserMessages::CUT, "Couper");
    bundle.add(BrowserMessages::UNDO, "Annuler");
    bundle.add(BrowserMessages::REDO, "Rétablir");
    bundle.add(BrowserMessages::SELECT_ALL, "Tout sélectionner");

    bundle.add(BrowserMessages::OK, "OK");
    bundle.add(BrowserMessages::CANCEL, "Annuler");
    bundle.add(BrowserMessages::YES, "Oui");
    bundle.add(BrowserMessages::NO, "Non");
    bundle.add(BrowserMessages::SAVE, "Enregistrer");
    bundle.add(BrowserMessages::CLOSE, "Fermer");

    bundle.add(BrowserMessages::NEW_TAB, "Nouvel onglet");
    bundle.add(BrowserMessages::CLOSE_TAB, "Fermer l'onglet");
    bundle.add(BrowserMessages::REOPEN_TAB, "Rouvrir l'onglet");

    bundle.add(BrowserMessages::ADD_BOOKMARK, "Ajouter un favori");
    bundle.add(BrowserMessages::REMOVE_BOOKMARK, "Supprimer le favori");
    bundle.add(BrowserMessages::EDIT_BOOKMARK, "Modifier le favori");

    bundle.add(BrowserMessages::ERR_NOT_FOUND, "Page introuvable");
    bundle.add(BrowserMessages::ERR_CONNECTION, "Connexion impossible");
    bundle.add(
        BrowserMessages::ERR_CERTIFICATE,
        "Erreur de certificat de sécurité",
    );
    bundle.add(BrowserMessages::ERR_TIMEOUT, "Délai de connexion dépassé");

    bundle
}
