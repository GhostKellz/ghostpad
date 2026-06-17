import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import QtQuick.Dialogs 1.3
import Qt5Compat.GraphicalEffects 1.15
import Qt.labs.platform 1.1 as PlatformDialogs
import org.kde.kirigami 2.20 as Kirigami
import GhostPad 1.0

Kirigami.ApplicationWindow {
    id: root
    width: 960
    height: 620
    minimumWidth: 640
    minimumHeight: 480
    visible: true
    color: "transparent"

    property real cornerRadius: Kirigami.Units.cornerRadius * 1.6
    property real chromeMargin: Kirigami.Units.mediumSpacing
    property real chromeContentPadding: Kirigami.Units.mediumSpacing
    property bool translucencyEnabled: false
    property bool shadowEnabled: true
    property string themeVariant: "system"

    Theme {
        id: gpTheme
    }

    property var appState: ({
        active_id: null,
        documents: [],
        recent: [],
        active_text: "",
        error: null,
        pending_close: null,
        autosave_epoch: null,
        find: null
    })
    property int activeTabIndex: -1
    property bool ignoreEditorChange: false
    property bool syncingTabSelection: false
    property bool pendingCloseSuppressed: false
    property var pendingSaveAfterClose: null
    property int saveAsTargetId: -1
    property bool wordWrapEnabled: false
    property bool formatSyncing: false
    property bool findPanelVisible: false
    property bool replaceMode: false
    property bool findCaseSensitive: false
    property bool findWholeWord: false
    property bool findUseRegex: false
    property bool findWrapAround: true
    property string lastFindQuery: ""
    property bool activeDocumentReadOnly: false
    property bool activeDocumentLocked: false
    property bool settingsLoaded: false
    property bool ignoreSettingsChange: false

    function surfaceColor() {
        const base = Kirigami.Theme.backgroundColor;
        const opacity = translucencyEnabled ? 0.72 : 1.0;
        return Qt.rgba(base.r, base.g, base.b, opacity);
    }

    function surfaceBorderColor() {
        const base = Kirigami.Theme.textColor;
        const alpha = translucencyEnabled ? 0.22 : 0.12;
        return Qt.rgba(base.r, base.g, base.b, alpha);
    }

    function headerFillColor() {
        const base = Kirigami.Theme.headerBackgroundColor;
        const alpha = translucencyEnabled ? 0.82 : 1.0;
        return Qt.rgba(base.r, base.g, base.b, alpha);
    }

    // Applies a theme by name. "system" follows the desktop color scheme; the
    // other variants assign explicit colors to the writable Kirigami.Theme
    // roles, which cascade to child items.
    function applyTheme(variant) {
        const colors = gpTheme.colorsFor(variant);
        if (!colors) {
            Kirigami.Theme.inherit = true;
            return;
        }
        Kirigami.Theme.inherit = false;
        Kirigami.Theme.backgroundColor = colors.background;
        Kirigami.Theme.textColor = colors.text;
        Kirigami.Theme.disabledTextColor = colors.disabledText;
        Kirigami.Theme.alternateBackgroundColor = colors.alternateBackground;
        Kirigami.Theme.highlightColor = colors.highlight;
        Kirigami.Theme.negativeTextColor = colors.negativeText;
        Kirigami.Theme.neutralTextColor = colors.neutralText;
        Kirigami.Theme.positiveTextColor = colors.positiveText;
    }

    background: Item {
        anchors.fill: parent

        Rectangle {
            id: chromeSurfaceBackdrop
            anchors.fill: parent
            anchors.margins: root.chromeMargin
            radius: root.cornerRadius
            color: root.surfaceColor()
            border.color: root.surfaceBorderColor()
            border.width: 1
            antialiasing: true
        }

        DropShadow {
            anchors.fill: chromeSurfaceBackdrop
            horizontalOffset: 0
            verticalOffset: translucencyEnabled ? Kirigami.Units.smallSpacing * 1.2 : Kirigami.Units.smallSpacing
            radius: root.shadowEnabled ? 48 : 0
            samples: 48
            color: Qt.rgba(0, 0, 0, translucencyEnabled ? 0.30 : 0.24)
            transparentBorder: true
            cached: true
            source: chromeSurfaceBackdrop
            visible: root.shadowEnabled
            z: -1
        }
    }

    onFindPanelVisibleChanged: {
        if (findPanelVisible) {
            Qt.callLater(function() {
                updateFindSelection();
                if (findInput)
                    findInput.selectAll();
            });
        } else {
            findDebounceTimer.stop();
        }
    }

    onWordWrapEnabledChanged: {
        applyWordWrap();
        saveUiSettings();
    }

    onTranslucencyEnabledChanged: saveUiSettings()
    onShadowEnabledChanged: saveUiSettings()
    onThemeVariantChanged: {
        applyTheme(themeVariant);
        saveUiSettings();
    }

    onClosing: function(close) {
        saveWindowState();
        close.accepted = true;
    }

    readonly property string versionLabel: backend.appName() + " " + backend.appVersion()

    title: {
        const doc = activeDocument();
        if (doc) {
            return doc.title + (doc.dirty ? "*" : "") + " — " + backend.appName();
        }
        return backend.appName();
    }

    GhostPad.Backend {
        id: backend
    }

    ListModel { id: documentModel }
    ListModel { id: recentModel }
    ListModel {
        id: encodingModel
        ListElement { label: "UTF-8"; value: "Utf8" }
        ListElement { label: "UTF-16 LE"; value: "Utf16Le" }
        ListElement { label: "UTF-16 BE"; value: "Utf16Be" }
        ListElement { label: "ISO-8859-1"; value: "Iso8859_1" }
    }
    ListModel {
        id: lineEndingModel
        ListElement { label: "LF (Unix)"; value: "Lf" }
        ListElement { label: "CRLF (Windows)"; value: "Crlf" }
    }

    Component.onCompleted: {
        applyWordWrap();
        handleResponse(backend.bootstrap());
    }

    function safeParse(response) {
        if (!response)
            return null;
        try {
            return JSON.parse(response);
        } catch (err) {
            console.warn("Failed to parse backend response", err, response);
            return null;
        }
    }

    function handleResponse(response) {
        const payload = safeParse(response);
        if (payload) {
            applyState(payload);
        }
    }

    function applyState(state) {
        if (!state)
            return;

        appState = state;
        textSyncTimer.stop();

        if (!state.active_id) {
            findPanelVisible = false;
            replaceMode = false;
        }

        syncModel(documentModel, state.documents || []);
        syncModel(recentModel, state.recent || []);

        const index = findDocumentIndex(state.active_id);
        syncingTabSelection = true;
        activeTabIndex = index;
        tabBar.currentIndex = index >= 0 ? index : -1;
        syncingTabSelection = false;

        if (state.active_text !== null && state.active_text !== undefined) {
            if (editor.text !== state.active_text) {
                ignoreEditorChange = true;
                editor.text = state.active_text;
                ignoreEditorChange = false;
            }
        }

        const doc = activeDocument();
        activeDocumentReadOnly = doc ? !!doc.read_only : false;
        activeDocumentLocked = doc ? !!doc.editing_locked : false;

        editor.readOnly = !state.active_id || activeDocumentLocked;
        editor.focus = state.active_id !== null;

        updateStatus(state);
        updateCursorInfo();
        syncFormatSelectors();
        if (highlightCanvas)
            highlightCanvas.requestPaint();
        updateFindSelection();

        if (state.pending_close && !pendingCloseSuppressed) {
            unsavedDialog.pendingDoc = state.pending_close;
            if (!unsavedDialog.visible) {
                unsavedDialog.open();
            }
        } else {
            unsavedDialog.pendingDoc = null;
            if (unsavedDialog.visible) {
                unsavedDialog.close();
            }
        }
        pendingCloseSuppressed = false;

        // Apply settings if present in response (e.g., on bootstrap)
        if (state.settings) {
            applySettings(state.settings);
        }
    }

    function applySettings(settings) {
        if (!settings)
            return;

        ignoreSettingsChange = true;

        // UI Settings
        if (settings.ui) {
            wordWrapEnabled = settings.ui.word_wrap_enabled || false;
            translucencyEnabled = settings.ui.translucency_enabled || false;
            shadowEnabled = settings.ui.shadow_enabled !== false; // default true
            themeVariant = settings.ui.theme || "system";
            applyTheme(themeVariant);
        }

        // Editor Settings (font stored for editor)
        if (settings.editor) {
            if (editor) {
                if (settings.editor.font_family)
                    editor.font.family = settings.editor.font_family;
                if (settings.editor.font_size > 0)
                    editor.font.pointSize = settings.editor.font_size;
                if (settings.editor.tab_stop_distance > 0)
                    editor.tabStopDistance = settings.editor.tab_stop_distance * editor.font.pixelSize;
            }
        }

        // Find defaults
        if (settings.find_defaults) {
            findCaseSensitive = settings.find_defaults.case_sensitive || false;
            findWholeWord = settings.find_defaults.whole_word || false;
            findUseRegex = settings.find_defaults.use_regex || false;
            findWrapAround = settings.find_defaults.wrap_around !== false; // default true
        }

        // Window state
        if (settings.window) {
            if (settings.window.width > 0 && settings.window.height > 0) {
                root.width = settings.window.width;
                root.height = settings.window.height;
            }
            if (settings.window.x !== null && settings.window.y !== null) {
                root.x = settings.window.x;
                root.y = settings.window.y;
            }
        }

        ignoreSettingsChange = false;
        settingsLoaded = true;
    }

    function saveUiSettings() {
        if (!settingsLoaded || ignoreSettingsChange)
            return;
        backend.update_ui_settings(
            wordWrapEnabled,
            translucencyEnabled,
            shadowEnabled,
            themeVariant
        );
    }

    function saveFindDefaults() {
        if (!settingsLoaded || ignoreSettingsChange)
            return;
        backend.update_find_defaults(
            findCaseSensitive,
            findWholeWord,
            findUseRegex,
            findWrapAround
        );
    }

    function saveWindowState() {
        if (!settingsLoaded)
            return;
        backend.update_window_state(root.width, root.height, root.x, root.y);
    }

    function updateStatus(state) {
        const doc = activeDocument();
        if (state.error) {
            statusBar.isError = true;
            statusBar.statusMessage = state.error;
        } else if (doc && doc.editing_locked) {
            statusBar.isError = false;
            statusBar.statusMessage = qsTr("Read-only — enable editing to make changes");
        } else if (state.find && state.find.message) {
            statusBar.isError = false;
            statusBar.statusMessage = state.find.message;
        } else {
            statusBar.isError = false;
            statusBar.statusMessage = formatAutosave(state.autosave_epoch);
        }
        statusBar.pathInfo = activeDocumentPath();
    }

    function syncModel(model, items) {
        model.clear();
        for (let i = 0; i < items.length; ++i) {
            model.append(items[i]);
        }
    }

    function findDocumentIndex(id) {
        if (id === null || id === undefined)
            return -1;
        for (let i = 0; i < documentModel.count; ++i) {
            if (documentModel.get(i).id === id)
                return i;
        }
        return -1;
    }

    function findDocumentById(id) {
        for (let i = 0; i < documentModel.count; ++i) {
            const item = documentModel.get(i);
            if (item.id === id) {
                return item;
            }
        }
        return null;
    }

    function activeDocument() {
        return findDocumentById(appState.active_id);
    }

    function activeDocumentIsReadOnly() {
        const doc = activeDocument();
        return doc ? !!doc.read_only : false;
    }

    function activeDocumentIsLocked() {
        const doc = activeDocument();
        return doc ? !!doc.editing_locked : false;
    }

    function toggleReadOnlyOverride(allowEdit) {
        if (!appState.active_id)
            return;
        handleResponse(backend.set_active_edit_override(allowEdit));
    }

    function activateDocument(id) {
        if (appState.active_id === id || id === null || id === undefined)
            return;
        handleResponse(backend.set_active_document(id));
    }

    function closeDocument(id) {
        handleResponse(backend.close_document(id));
    }

    function forceCloseDocument(id) {
        handleResponse(backend.force_close_document(id));
    }

    function ensureActiveDocument(id) {
        if (!id || appState.active_id === id)
            return;
        handleResponse(backend.set_active_document(id));
    }

    function activeDocumentPath() {
        const doc = activeDocument();
        if (!doc)
            return "";
        let label = doc.path && doc.path.length > 0 ? doc.path : qsTr("Unsaved document");
        if (doc.read_only) {
            label += doc.editing_locked ? qsTr(" (Read-only)") : qsTr(" (Override enabled)");
        }
        return label;
    }

    function performSaveActive() {
        const doc = activeDocument();
        if (!doc)
            return;
        if (doc.editing_locked)
            return;
        if (!doc.path || doc.path.length === 0) {
            openSaveAsDialog(doc.id);
            return;
        }
        handleResponse(backend.save_active());
    }

    function openSaveAsDialog(docId) {
        if (!docId)
            return;
        ensureActiveDocument(docId);
        saveAsTargetId = docId;
        try {
            nativeSaveDialog.open();
        } catch (err) {
            fallbackSaveDialog.open();
        }
    }

    function openRecentDocument(path) {
        handleResponse(backend.open_document(path));
    }

    function formatAutosave(epoch) {
        if (!epoch)
            return qsTr("Ready");
        const now = Date.now();
        const stamp = epoch * 1000;
        const delta = Math.max(0, Math.floor((now - stamp) / 1000));
        if (delta < 5)
            return qsTr("Autosaved just now");
        if (delta < 60)
            return qsTr("Autosaved %1s ago").arg(delta);
        if (delta < 3600)
            return qsTr("Autosaved %1m ago").arg(Math.floor(delta / 60));
        if (delta < 86400)
            return qsTr("Autosaved %1h ago").arg(Math.floor(delta / 3600));
        const date = new Date(stamp);
        return qsTr("Autosaved %1").arg(date.toLocaleString(Qt.locale()));
    }

    function applyWordWrap() {
        if (!editor)
            return;
        editor.wrapMode = wordWrapEnabled ? TextEdit.WordWrap : TextEdit.NoWrap;
    }

    function updateCursorInfo() {
        if (!editor || !appState.active_id) {
            statusBar.cursorInfo = "";
            return;
        }
        const pos = editor.cursorPosition;
        const buffer = editor.text;
        const upToCursor = buffer.slice(0, pos);
        const segments = upToCursor.split("\n");
        const line = segments.length;
        const column = segments[segments.length - 1].length + 1;
        statusBar.cursorInfo = qsTr("Ln %1, Col %2").arg(line).arg(column);
    }

    function duplicateCurrentLine() {
        if (!editor || !appState.active_id)
            return;

        const buffer = editor.text;
        if (buffer.length === 0)
            return;

        const pos = editor.cursorPosition;
        let lineStart = buffer.lastIndexOf("\n", Math.max(0, pos - 1));
        if (lineStart === -1)
            lineStart = 0;
        else
            lineStart += 1;

        let lineEnd = buffer.indexOf("\n", pos);
        if (lineEnd === -1)
            lineEnd = buffer.length;

        const lineContent = buffer.slice(lineStart, lineEnd);
        const insertPos = lineEnd;
        const insertion = "\n" + lineContent;
        editor.insert(insertPos, insertion);
        editor.cursorPosition = insertPos + insertion.length;
        updateCursorInfo();
    }

    function currentFindState() {
        return appState && appState.find ? appState.find : null;
    }

    function openFindPanel(asReplace) {
        if (!editor || !appState.active_id)
            return;
        replaceMode = asReplace;
        if (!findPanelVisible)
            findPanelVisible = true;
        if (editor.selectedText && editor.selectedText.length > 0) {
            findInput.text = editor.selectedText;
        } else if (findInput.text.length === 0 && lastFindQuery.length > 0) {
            findInput.text = lastFindQuery;
        }
        Qt.callLater(function() {
            if (replaceMode && replaceInput.visible) {
                replaceInput.forceActiveFocus();
                replaceInput.selectAll();
            } else {
                findInput.forceActiveFocus();
                findInput.selectAll();
            }
        });
        scheduleFindUpdate();
        updateFindSelection();
    }

    function closeFindPanel() {
        if (!findPanelVisible)
            return;
        findPanelVisible = false;
        replaceMode = false;
        if (appState.active_id)
            handleResponse(backend.clear_find());
        editor.forceActiveFocus();
    }

    function scheduleFindUpdate() {
        findDebounceTimer.restart();
    }

    function triggerFindUpdate() {
        if (!appState.active_id)
            return;
        const query = findInput.text || "";
        if (query.length > 0)
            lastFindQuery = query;
        handleResponse(backend.begin_find(
            query,
            findCaseSensitive,
            findWholeWord,
            findUseRegex
        ));
    }

    function sendFindStep(backwards) {
        if (!appState.active_id)
            return;
        const state = currentFindState();
        if (!findPanelVisible && (!state || !state.query || state.query.length === 0) && findInput.text.length === 0) {
            openFindPanel(false);
            return;
        }
        const response = backwards ? backend.find_previous(findWrapAround) : backend.find_next(findWrapAround);
        handleResponse(response);
    }

    function performReplace(backwards) {
        if (!appState.active_id)
            return;
        if (activeDocumentLocked)
            return;
        const response = backend.replace_current(replaceInput.text || "", findWrapAround, backwards);
        handleResponse(response);
    }

    function performReplaceAll() {
        if (!appState.active_id)
            return;
        if (activeDocumentLocked)
            return;
        handleResponse(backend.replace_all(replaceInput.text || ""));
    }

    function updateFindSelection() {
        if (!editor || !findPanelVisible)
            return;
        const state = currentFindState();
        if (!state || state.current_index === null)
            return;
        const matches = state.matches || [];
        if (state.current_index < 0 || state.current_index >= matches.length)
            return;
        const match = matches[state.current_index];
        ignoreEditorChange = true;
        editor.select(match.start, match.end);
        ignoreEditorChange = false;
        ensureMatchVisible(match.start, match.end);
    }

    function ensureMatchVisible(start, end) {
        if (!editorScroll || !editorScroll.flickableItem)
            return;
        const rect = editor.positionToRectangle(start);
        const flick = editorScroll.flickableItem;
        const top = rect.y;
        const bottom = rect.y + rect.height;
        if (top < flick.contentY) {
            flick.contentY = Math.max(0, top - Kirigami.Units.gridUnit);
        } else if (bottom > flick.contentY + flick.height) {
            flick.contentY = Math.min(
                flick.contentHeight - flick.height,
                bottom - flick.height + Kirigami.Units.gridUnit
            );
        }
    }

    function findCounterText() {
        const state = currentFindState();
        if (!state)
            return "";
        const total = state.matches ? state.matches.length : 0;
        if (total === 0)
            return "0 / 0";
        const current = state.current_index !== null ? state.current_index + 1 : 0;
        return `${current} / ${total}`;
    }

    function findModelIndex(model, value) {
        if (!model)
            return -1;
        for (let i = 0; i < model.count; ++i) {
            if (model.get(i).value === value)
                return i;
        }
        return -1;
    }

    function syncFormatSelectors() {
        if (!encodingSelector || !lineEndingSelector)
            return;

        formatSyncing = true;
        const doc = activeDocument();
        if (doc) {
            const encodingIndex = findModelIndex(encodingModel, doc.encoding || "Utf8");
            encodingSelector.currentIndex = encodingIndex >= 0 ? encodingIndex : 0;
            const lineIndex = findModelIndex(lineEndingModel, doc.line_ending || "Lf");
            lineEndingSelector.currentIndex = lineIndex >= 0 ? lineIndex : 0;
            const editable = !doc.editing_locked;
            encodingSelector.enabled = editable;
            lineEndingSelector.enabled = editable;
            reloadEncodingButton.enabled = editable && !!doc.path && doc.path.length > 0 && !doc.dirty;
        } else {
            encodingSelector.currentIndex = 0;
            lineEndingSelector.currentIndex = 0;
            encodingSelector.enabled = false;
            lineEndingSelector.enabled = false;
            reloadEncodingButton.enabled = false;
        }
        formatSyncing = false;
    }

    function toLocalPath(url) {
        if (!url)
            return "";
        if (typeof url === "string")
            return url;
        if (url.toString)
            return url.toString();
        return "";
    }

    function openFilePicker() {
        try {
            nativeOpenDialog.open();
        } catch (err) {
            fallbackOpenDialog.open();
        }
    }

    function handlePendingSave() {
        const doc = unsavedDialog.pendingDoc;
        if (!doc)
            return;
        pendingCloseSuppressed = true;
        ensureActiveDocument(doc.id);
        const summary = findDocumentById(doc.id);
        if (summary && summary.path && summary.path.length > 0) {
            handleResponse(backend.save_active());
            handleResponse(backend.close_document(doc.id));
            unsavedDialog.close();
        } else {
            pendingSaveAfterClose = doc.id;
            unsavedDialog.close();
            openSaveAsDialog(doc.id);
        }
    }

    function handlePendingDiscard() {
        const doc = unsavedDialog.pendingDoc;
        if (!doc)
            return;
        pendingCloseSuppressed = true;
        handleResponse(backend.force_close_document(doc.id));
        unsavedDialog.close();
    }

    function cancelPendingClose() {
        pendingCloseSuppressed = true;
        unsavedDialog.close();
        handleResponse(backend.state());
    }

    Kirigami.Action {
        id: newTabAction
        text: qsTr("New Tab")
        icon.name: "tab-new"
        shortcut: StandardKey.New
        onTriggered: handleResponse(backend.new_document())
    }

    Kirigami.Action {
        id: openAction
        text: qsTr("Open…")
        icon.name: "document-open"
        shortcut: StandardKey.Open
        onTriggered: openFilePicker()
    }

    Kirigami.Action {
        id: saveAction
        text: qsTr("Save")
        icon.name: "document-save"
        enabled: documentModel.count > 0 && !activeDocumentLocked
        shortcut: StandardKey.Save
        onTriggered: performSaveActive()
    }

    Kirigami.Action {
        id: saveAsAction
        text: qsTr("Save As…")
        icon.name: "document-save-as"
        enabled: documentModel.count > 0
        shortcut: StandardKey.SaveAs
        onTriggered: {
            const doc = activeDocument();
            if (doc)
                openSaveAsDialog(doc.id);
        }
    }

    Kirigami.Action {
        id: closeTabAction
        text: qsTr("Close Tab")
        icon.name: "tab-close"
        enabled: documentModel.count > 0
        shortcut: StandardKey.Close
        onTriggered: {
            const doc = activeDocument();
            if (doc)
                closeDocument(doc.id);
        }
    }

    Kirigami.Action {
        id: undoAction
        text: qsTr("Undo")
        icon.name: "edit-undo"
        shortcut: StandardKey.Undo
        enabled: editor.enabled && editor.canUndo && !activeDocumentLocked
        onTriggered: editor.undo()
    }

    Kirigami.Action {
        id: redoAction
        text: qsTr("Redo")
        icon.name: "edit-redo"
        shortcut: StandardKey.Redo
        enabled: editor.enabled && editor.canRedo && !activeDocumentLocked
        onTriggered: editor.redo()
    }

    Kirigami.Action {
        id: cutAction
        text: qsTr("Cut")
        icon.name: "edit-cut"
        shortcut: StandardKey.Cut
        enabled: editor.enabled && editor.selectedText.length > 0 && !activeDocumentLocked
        onTriggered: editor.cut()
    }

    Kirigami.Action {
        id: copyAction
        text: qsTr("Copy")
        icon.name: "edit-copy"
        shortcut: StandardKey.Copy
        enabled: editor.enabled && editor.selectedText.length > 0
        onTriggered: editor.copy()
    }

    Kirigami.Action {
        id: pasteAction
        text: qsTr("Paste")
        icon.name: "edit-paste"
        shortcut: StandardKey.Paste
        enabled: editor.enabled && !activeDocumentLocked
        onTriggered: editor.paste()
    }

    Kirigami.Action {
        id: selectAllAction
        text: qsTr("Select All")
        icon.name: "edit-select-all"
        shortcut: StandardKey.SelectAll
        enabled: editor.enabled
        onTriggered: editor.selectAll()
    }

    Kirigami.Action {
        id: duplicateLineAction
        text: qsTr("Duplicate Line")
        icon.name: "edit-copy"
        shortcut: "Ctrl+D"
        enabled: editor.enabled && !activeDocumentLocked
        onTriggered: duplicateCurrentLine()
    }

    Kirigami.Action {
        id: findAction
        text: qsTr("Find…")
        icon.name: "edit-find"
        shortcut: StandardKey.Find
        enabled: documentModel.count > 0
        onTriggered: openFindPanel(false)
    }

    Kirigami.Action {
        id: findNextAction
        text: qsTr("Find Next")
        icon.name: "go-down"
        shortcut: "F3"
        enabled: documentModel.count > 0
        onTriggered: sendFindStep(false)
    }

    Kirigami.Action {
        id: findPreviousAction
        text: qsTr("Find Previous")
        icon.name: "go-up"
        shortcut: "Shift+F3"
        enabled: documentModel.count > 0
        onTriggered: sendFindStep(true)
    }

    Kirigami.Action {
        id: replaceAction
        text: qsTr("Replace…")
        icon.name: "edit-find-replace"
        shortcut: StandardKey.Replace
        enabled: documentModel.count > 0 && !activeDocumentLocked
        onTriggered: openFindPanel(true)
    }

    Shortcut {
        sequence: "Escape"
        enabled: findPanelVisible
        onActivated: closeFindPanel()
    }

    Kirigami.Action {
        id: wordWrapAction
        text: qsTr("Word Wrap")
        icon.name: "format-justify-fill"
        checkable: true
        checked: wordWrapEnabled
        shortcut: "Ctrl+Shift+W"
        onToggled: wordWrapEnabled = checked
    }

    Kirigami.Action {
        id: translucencyAction
        text: qsTr("Translucent Chrome")
        icon.name: "preferences-desktop-theme"
        checkable: true
        checked: translucencyEnabled
        onToggled: translucencyEnabled = checked
    }

    Kirigami.Action {
        id: readOnlyToggleAction
        text: activeDocumentLocked ? qsTr("Enable Editing") : qsTr("Disable Editing")
    icon.name: activeDocumentLocked ? "object-unlocked" : "object-locked"
        enabled: activeDocumentReadOnly
        visible: documentModel.count > 0 && activeDocumentReadOnly
        onTriggered: toggleReadOnlyOverride(activeDocumentLocked)
    }

    menuBar: MenuBar {
        Menu {
            title: qsTr("File")
            MenuItem { action: newTabAction }
            MenuItem { action: openAction }
            MenuItem { action: saveAction }
            MenuItem { action: saveAsAction }
            MenuItem { action: closeTabAction }
            MenuSeparator {}
            MenuItem {
                text: qsTr("Quit")
                shortcut: StandardKey.Quit
                onTriggered: Qt.quit()
            }
        }
        Menu {
            title: qsTr("Edit")
            MenuItem { action: undoAction }
            MenuItem { action: redoAction }
            MenuSeparator {}
            MenuItem { action: cutAction }
            MenuItem { action: copyAction }
            MenuItem { action: pasteAction }
            MenuItem { action: selectAllAction }
            MenuSeparator {}
            MenuItem { action: duplicateLineAction }
            MenuSeparator {}
            MenuItem { action: findAction }
            MenuItem { action: findNextAction }
            MenuItem { action: findPreviousAction }
            MenuItem { action: replaceAction }
        }
        Menu {
            title: qsTr("View")
            MenuItem { action: wordWrapAction }
            MenuItem { action: translucencyAction }
            MenuSeparator {}
            MenuItem {
                text: qsTr("Settings…")
                icon.name: "configure"
                onTriggered: settingsSheet.open()
            }
            MenuSeparator {}
            MenuItem {
                action: readOnlyToggleAction
                visible: readOnlyToggleAction.visible
            }
        }
        Menu {
            title: qsTr("Help")
            MenuItem {
                text: qsTr("About")
                onTriggered: aboutSheet.open()
            }
        }
    }

    header: ToolBar {
        height: Kirigami.Units.gridUnit * 3
        contentHeight: Kirigami.Units.gridUnit * 3
        background: Rectangle {
            color: root.headerFillColor()
            radius: root.cornerRadius
            antialiasing: true
            border.width: translucencyEnabled ? 1 : 0
            border.color: root.surfaceBorderColor()
        }

        RowLayout {
            anchors.fill: parent
            spacing: Kirigami.Units.smallSpacing

            ToolButton { action: newTabAction }
            ToolButton { action: openAction }
            ToolButton { action: saveAction }
            ToolButton { action: saveAsAction }
            ToolButton { action: closeTabAction }
            ToolButton { action: undoAction }
            ToolButton { action: redoAction }
            ToolButton { action: findAction }
            ToolButton { action: replaceAction }
            ToolButton { action: wordWrapAction }
            ToolButton { action: translucencyAction }
            ToolButton {
                action: readOnlyToggleAction
                visible: readOnlyToggleAction.visible
            }

            Item { Layout.fillWidth: true }

            Label {
                text: activeDocumentPath()
                color: Kirigami.Theme.disabledTextColor
                elide: Text.ElideMiddle
            }

            Label {
                text: versionLabel
                color: Kirigami.Theme.disabledTextColor
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: root.chromeContentPadding
        spacing: Kirigami.Units.smallSpacing

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: appState.error !== null && appState.error !== undefined
            text: appState.error || ""
            type: Kirigami.MessageType.Error
            actions: [
                Kirigami.Action {
                    text: qsTr("Dismiss")
                    onTriggered: {
                        pendingCloseSuppressed = true;
                        handleResponse(backend.state());
                    }
                }
            ]
        }

        Kirigami.InlineMessage {
            id: readOnlyBanner
            Layout.fillWidth: true
            visible: activeDocumentReadOnly
            type: activeDocumentLocked ? Kirigami.MessageType.Warning : Kirigami.MessageType.Information
            text: activeDocumentLocked ? qsTr("This document is read-only on disk. Enable editing to make changes.") : qsTr("Read-only protection is temporarily disabled. Remember to restore it when you're done.")
            actions: [
                Kirigami.Action {
                    text: qsTr("Enable Editing")
                    icon.name: "object-unlocked"
                    visible: activeDocumentLocked
                    onTriggered: toggleReadOnlyOverride(true)
                },
                Kirigami.Action {
                    text: qsTr("Disable Editing")
                    icon.name: "object-locked"
                    visible: activeDocumentReadOnly && !activeDocumentLocked
                    onTriggered: toggleReadOnlyOverride(false)
                }
            ]
        }

        Kirigami.InlineMessage {
            id: externalChangeBanner
            Layout.fillWidth: true
            visible: {
                const doc = activeDocument();
                return doc && (doc.externally_modified || doc.externally_deleted);
            }
            type: {
                const doc = activeDocument();
                return (doc && doc.externally_deleted) ? Kirigami.MessageType.Error : Kirigami.MessageType.Warning;
            }
            text: {
                const doc = activeDocument();
                if (!doc) return "";
                if (doc.externally_deleted) {
                    return qsTr("This file has been deleted or moved. Save to recreate it, or close the tab.");
                }
                return qsTr("This file has been modified outside of GhostPad. Reload to see changes, or keep editing to overwrite.");
            }
            actions: [
                Kirigami.Action {
                    text: qsTr("Reload")
                    icon.name: "view-refresh"
                    visible: {
                        const doc = activeDocument();
                        return doc && doc.externally_modified && !doc.externally_deleted;
                    }
                    onTriggered: {
                        const doc = activeDocument();
                        if (doc) handleResponse(backend.reload_document(doc.id));
                    }
                },
                Kirigami.Action {
                    text: qsTr("Dismiss")
                    icon.name: "dialog-close"
                    onTriggered: {
                        const doc = activeDocument();
                        if (doc) handleResponse(backend.dismiss_external_change(doc.id));
                    }
                }
            ]
        }

        RowLayout {
            Layout.fillWidth: true
            visible: documentModel.count > 0

            TabBar {
                id: tabBar
                Layout.fillWidth: true
                currentIndex: root.activeTabIndex
                onCurrentIndexChanged: {
                    if (root.syncingTabSelection)
                        return;
                    if (currentIndex >= 0 && currentIndex < documentModel.count) {
                        const entry = documentModel.get(currentIndex);
                        activateDocument(entry.id);
                    }
                }

                Repeater {
                    model: documentModel
                    delegate: TabButton {
                        readonly property var entry: model
                        text: entry.title + (entry.dirty ? "*" : "")
                        checkable: true
                        checked: index === tabBar.currentIndex
                        contentItem: RowLayout {
                            spacing: Kirigami.Units.smallSpacing
                            Kirigami.Icon {
                                visible: entry.externally_deleted || entry.externally_modified
                                source: entry.externally_deleted ? "dialog-warning" : "emblem-important"
                                width: Kirigami.Units.iconSizes.small
                                height: width
                                color: entry.externally_deleted ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.neutralTextColor
                            }
                            Kirigami.Icon {
                                visible: entry.read_only && !entry.externally_deleted && !entry.externally_modified
                                source: entry.editing_locked ? "object-locked" : "object-unlocked"
                                width: Kirigami.Units.iconSizes.small
                                height: width
                            }
                            Label {
                                text: parent.TabButton.text
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                                color: entry.externally_deleted ? Kirigami.Theme.negativeTextColor :
                                       entry.externally_modified ? Kirigami.Theme.neutralTextColor :
                                       Kirigami.Theme.textColor
                            }
                            ToolButton {
                                icon.name: "tab-close"
                                visible: documentModel.count > 1
                                focusPolicy: Qt.NoFocus
                                onPressed: mouse.accepted = true
                                onClicked: closeDocument(entry.id)
                            }
                        }
                        onClicked: activateDocument(entry.id)
                    }
                }
            }

            ToolButton {
                icon.name: "tab-new"
                visible: documentModel.count > 0
                onClicked: newTabAction.trigger()
            }

            Item { Layout.fillWidth: true }

            ComboBox {
                id: encodingSelector
                model: encodingModel
                textRole: "label"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 8
                enabled: false
                displayText: currentIndex >= 0 ? qsTr(encodingModel.get(currentIndex).label) : qsTr("Encoding")
                onActivated: {
                    if (root.formatSyncing)
                        return;
                    const option = encodingModel.get(index);
                    if (option)
                        handleResponse(backend.set_active_encoding(option.value));
                }
            }

            Kirigami.Button {
                id: reloadEncodingButton
                text: qsTr("Reload")
                icon.name: "view-refresh"
                enabled: false
                onClicked: {
                    if (root.formatSyncing)
                        return;
                    const option = encodingModel.get(encodingSelector.currentIndex);
                    if (option)
                        handleResponse(backend.reload_active_with_encoding(option.value));
                }
            }

            ComboBox {
                id: lineEndingSelector
                model: lineEndingModel
                textRole: "label"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 6
                enabled: false
                displayText: currentIndex >= 0 ? qsTr(lineEndingModel.get(currentIndex).label) : qsTr("Line Endings")
                onActivated: {
                    if (root.formatSyncing)
                        return;
                    const option = lineEndingModel.get(index);
                    if (option)
                        handleResponse(backend.set_active_line_ending(option.value));
                }
            }
        }

        Kirigami.Separator {
            Layout.fillWidth: true
            visible: documentModel.count > 0
        }

        StackLayout {
            id: tabStack
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: documentModel.count === 0 ? 0 : 1

            Kirigami.ScrollablePage {
                id: welcomePage
                title: qsTr("Welcome")
                contentItem: ColumnLayout {
                    anchors.fill: parent
                    spacing: Kirigami.Units.largeSpacing

                    Kirigami.Heading {
                        level: 1
                        text: backend.welcome_headline()
                    }
                    Label {
                        text: backend.welcome_tagline()
                        wrapMode: Text.WordWrap
                        font.pointSize: 14
                        color: Kirigami.Theme.disabledTextColor
                    }

                    Kirigami.Card {
                        Layout.fillWidth: true
                        visible: recentModel.count > 0
                        header: Kirigami.Heading {
                            level: 4
                            text: qsTr("Recent documents")
                        }
                        contentItem: ListView {
                            id: recentList
                            implicitHeight: Math.min(contentHeight, Kirigami.Units.gridUnit * 12)
                            model: recentModel
                            clip: true
                            delegate: Kirigami.BasicListItem {
                                width: recentList.width
                                text: model.title
                                subtitle: model.path
                                onClicked: openRecentDocument(model.path)
                            }
                        }
                    }

                    Kirigami.PlaceholderMessage {
                        visible: recentModel.count === 0
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: Math.min(parent.width, Kirigami.Units.gridUnit * 28)
                        icon.name: "document-open"
                        text: qsTr("Open a file to get started")
                        explanation: qsTr("Create a new document or pick one from disk to begin editing.")
                        helpfulAction: Kirigami.Action {
                            text: qsTr("Open File…")
                            onTriggered: openFilePicker()
                        }
                        actions: [
                            Kirigami.Action {
                                text: qsTr("New Tab")
                                onTriggered: newTabAction.trigger()
                            }
                        ]
                    }
                }
            }

            Kirigami.ScrollablePage {
                id: editorPage
                title: qsTr("Editor")
                contentItem: ColumnLayout {
                    anchors.fill: parent
                    spacing: Kirigami.Units.smallSpacing

                    ScrollView {
                        id: editorScroll
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true

                        TextArea {
                            id: editor
                            anchors.fill: parent
                            wrapMode: root.wordWrapEnabled ? TextEdit.WordWrap : TextEdit.NoWrap
                            text: ""
                            font.family: "monospace"
                            selectByMouse: true
                            persistentSelection: true
                            placeholderText: documentModel.count === 0 ? qsTr("Open or create a document…") : ""
                            enabled: documentModel.count > 0
                            tabStopDistance: font.pixelSize * 4
                            onTextChanged: {
                                if (root.ignoreEditorChange)
                                    return;
                                if (!appState.active_id)
                                    return;
                                textSyncTimer.restart();
                                appState.active_text = text;
                                updateCursorInfo();
                            }
                            onCursorPositionChanged: updateCursorInfo()
                            onWidthChanged: {
                                if (highlightCanvas)
                                    highlightCanvas.requestPaint();
                            }
                            onHeightChanged: {
                                if (highlightCanvas)
                                    highlightCanvas.requestPaint();
                            }
                            onContentXChanged: {
                                if (highlightCanvas)
                                    highlightCanvas.requestPaint();
                            }
                            onContentYChanged: {
                                if (highlightCanvas)
                                    highlightCanvas.requestPaint();
                            }

                            Canvas {
                                id: highlightCanvas
                                anchors.fill: parent
                                z: 1
                                renderTarget: Canvas.FramebufferObject
                                property var matches: (findPanelVisible && appState.find && appState.find.matches) ? appState.find.matches : []
                                visible: findPanelVisible && matches.length > 0
                                onMatchesChanged: requestPaint()
                                onVisibleChanged: requestPaint()
                                onWidthChanged: requestPaint()
                                onHeightChanged: requestPaint()
                                onPaint: {
                                    var ctx = getContext("2d");
                                    ctx.save();
                                    ctx.clearRect(0, 0, width, height);
                                    if (!visible || matches.length === 0) {
                                        ctx.restore();
                                        return;
                                    }
                                    ctx.translate(-editor.contentX, -editor.contentY);
                                    ctx.fillStyle = Kirigami.Theme.highlightColor;
                                    ctx.globalAlpha = 0.25;
                                    for (var i = 0; i < matches.length; ++i) {
                                        paintMatch(matches[i].start, matches[i].end, ctx);
                                    }
                                    ctx.globalAlpha = 1.0;
                                    ctx.restore();
                                }

                                function paintMatch(start, end, ctx) {
                                    if (end <= start)
                                        return;
                                    var segmentStart = start;
                                    var previousRect = editor.positionToRectangle(segmentStart);
                                    var currentY = previousRect.y;
                                    for (var pos = start + 1; pos <= end; ++pos) {
                                        var rect = editor.positionToRectangle(pos);
                                        if (rect.y !== currentY) {
                                            drawSegment(segmentStart, pos, ctx);
                                            segmentStart = pos;
                                            currentY = rect.y;
                                        }
                                    }
                                    drawSegment(segmentStart, end, ctx);
                                }

                                function drawSegment(startPos, endExclusive, ctx) {
                                    if (endExclusive <= startPos)
                                        return;
                                    var startRect = editor.positionToRectangle(startPos);
                                    var lastRect = editor.positionToRectangle(endExclusive - 1);
                                    var startX = startRect.x;
                                    var endX = lastRect.x + lastRect.width;
                                    if (endX <= startX) {
                                        endX = startX + Math.max(startRect.width, lastRect.width, 2);
                                    }
                                    var height = startRect.height || editor.font.pixelSize * 1.3;
                                    ctx.fillRect(startX, startRect.y, endX - startX, height);
                                }
                            }
                        }

                        Connections {
                            target: editorScroll.flickableItem
                            ignoreUnknownSignals: true
                            function onContentXChanged() {
                                if (highlightCanvas)
                                    highlightCanvas.requestPaint();
                            }
                            function onContentYChanged() {
                                if (highlightCanvas)
                                    highlightCanvas.requestPaint();
                            }
                        }
                    }

                    Rectangle {
                        id: findPanel
                        Layout.fillWidth: true
                        visible: findPanelVisible
                        color: Kirigami.Theme.alternateBackgroundColor
                        border.color: Kirigami.Theme.disabledTextColor
                        radius: Kirigami.Units.smallSpacing
                        enabled: documentModel.count > 0

                        ColumnLayout {
                            id: findPanelLayout
                            anchors.fill: parent
                            anchors.margins: Kirigami.Units.smallSpacing
                            spacing: Kirigami.Units.smallSpacing

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: Kirigami.Units.smallSpacing
                                Label {
                                    text: qsTr("Find")
                                    font.bold: true
                                }
                                TextField {
                                    id: findInput
                                    Layout.fillWidth: true
                                    placeholderText: qsTr("Search text")
                                    onTextChanged: scheduleFindUpdate()
                                    Keys.onReturnPressed: sendFindStep(false)
                                    Keys.onEnterPressed: sendFindStep(false)
                                }
                                Label {
                                    id: findCounterLabel
                                    text: findCounterText()
                                    visible: text.length > 0
                                    color: Kirigami.Theme.disabledTextColor
                                }
                                Button {
                                    text: qsTr("Previous")
                                    icon.name: "go-up"
                                    enabled: documentModel.count > 0
                                    onClicked: sendFindStep(true)
                                }
                                Button {
                                    text: qsTr("Next")
                                    icon.name: "go-down"
                                    enabled: documentModel.count > 0
                                    onClicked: sendFindStep(false)
                                }
                                ToolButton {
                                    icon.name: "window-close"
                                    onClicked: closeFindPanel()
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: Kirigami.Units.smallSpacing
                                CheckBox {
                                    text: qsTr("Case sensitive")
                                    checked: findCaseSensitive
                                    onToggled: {
                                        findCaseSensitive = checked;
                                        scheduleFindUpdate();
                                        saveFindDefaults();
                                    }
                                }
                                CheckBox {
                                    text: qsTr("Whole word")
                                    checked: findWholeWord
                                    onToggled: {
                                        findWholeWord = checked;
                                        scheduleFindUpdate();
                                        saveFindDefaults();
                                    }
                                }
                                CheckBox {
                                    text: qsTr("Regular expression")
                                    checked: findUseRegex
                                    onToggled: {
                                        findUseRegex = checked;
                                        scheduleFindUpdate();
                                        saveFindDefaults();
                                    }
                                }
                                Item { Layout.fillWidth: true }
                                CheckBox {
                                    text: qsTr("Wrap around")
                                    checked: findWrapAround
                                    onToggled: {
                                        findWrapAround = checked;
                                        saveFindDefaults();
                                    }
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: Kirigami.Units.smallSpacing
                                visible: replaceMode
                                Label {
                                    text: qsTr("Replace")
                                    font.bold: true
                                }
                                TextField {
                                    id: replaceInput
                                    Layout.fillWidth: true
                                    placeholderText: qsTr("Replacement text")
                                    enabled: !activeDocumentLocked
                                    onAccepted: performReplace(false)
                                }
                                Button {
                                    text: qsTr("Replace")
                                    icon.name: "edit-find-replace"
                                    enabled: documentModel.count > 0 && !activeDocumentLocked
                                    onClicked: performReplace(false)
                                }
                                Button {
                                    text: qsTr("Replace All")
                                    icon.name: "edit-find-replace"
                                    enabled: documentModel.count > 0 && !activeDocumentLocked
                                    onClicked: performReplaceAll()
                                }
                            }

                            Label {
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                                text: currentFindState() && currentFindState().message ? currentFindState().message : ""
                                visible: text.length > 0
                                color: Kirigami.Theme.disabledTextColor
                            }
                        }
                    }
                }
            }
        }
    }

    footer: Rectangle {
        id: statusBar
        color: root.surfaceColor()
        implicitHeight: Kirigami.Units.gridUnit * 2
        border.color: root.surfaceBorderColor()
        border.width: 1
        property string statusMessage: qsTr("Ready")
        property string pathInfo: ""
        property bool isError: false
        property string cursorInfo: ""

        RowLayout {
            anchors.fill: parent
            anchors.margins: Kirigami.Units.smallSpacing
            spacing: Kirigami.Units.smallSpacing

            Kirigami.Heading {
                level: 5
                text: statusBar.statusMessage
                color: statusBar.isError ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
            }
            Kirigami.Separator {
                visible: true
                Layout.preferredHeight: parent.height * 0.6
            }
            Label {
                text: statusBar.pathInfo
                elide: Text.ElideRight
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
            }
            Kirigami.Separator {
                visible: statusBar.cursorInfo.length > 0
                Layout.preferredHeight: parent.height * 0.6
            }
            Label {
                text: statusBar.cursorInfo
                visible: statusBar.cursorInfo.length > 0
                color: Kirigami.Theme.disabledTextColor
            }
            Label {
                text: versionLabel
                color: Kirigami.Theme.disabledTextColor
            }
        }
    }

    PlatformDialogs.FileDialog {
        id: nativeOpenDialog
        title: qsTr("Open File")
        fileMode: PlatformDialogs.FileDialog.OpenFile
        onAccepted: {
            const path = toLocalPath(file);
            if (path && path.length > 0)
                handleResponse(backend.open_document(path));
        }
    }

    PlatformDialogs.FileDialog {
        id: nativeSaveDialog
        title: qsTr("Save File As")
        fileMode: PlatformDialogs.FileDialog.SaveFile
        onAccepted: {
            const path = toLocalPath(file);
            if (path && path.length > 0) {
                handleResponse(backend.save_active_as(path));
                if (pendingSaveAfterClose) {
                    handleResponse(backend.close_document(pendingSaveAfterClose));
                    pendingSaveAfterClose = null;
                }
            }
        }
        onRejected: {
            if (pendingSaveAfterClose) {
                pendingCloseSuppressed = true;
                handleResponse(backend.state());
                pendingSaveAfterClose = null;
            }
        }
    }

    FileDialog {
        id: fallbackOpenDialog
        title: qsTr("Open File")
        selectExisting: true
        onAccepted: {
            const path = toLocalPath(fileUrl);
            if (path && path.length > 0)
                handleResponse(backend.open_document(path));
        }
    }

    FileDialog {
        id: fallbackSaveDialog
        title: qsTr("Save File As")
        selectExisting: false
        onAccepted: {
            const path = toLocalPath(fileUrl);
            if (path && path.length > 0) {
                handleResponse(backend.save_active_as(path));
                if (pendingSaveAfterClose) {
                    handleResponse(backend.close_document(pendingSaveAfterClose));
                    pendingSaveAfterClose = null;
                }
            }
        }
        onRejected: {
            if (pendingSaveAfterClose) {
                pendingCloseSuppressed = true;
                handleResponse(backend.state());
                pendingSaveAfterClose = null;
            }
        }
    }

    Timer {
        id: findDebounceTimer
        interval: 150
        repeat: false
        onTriggered: triggerFindUpdate()
    }

    Timer {
        id: textSyncTimer
        interval: 250
        repeat: false
        onTriggered: {
            if (!appState.active_id)
                return;
            handleResponse(backend.update_active_text(editor.text));
        }
    }

    Timer {
        id: autosaveTimer
        interval: 60000
        repeat: true
        running: true
        onTriggered: {
            if (documentModel.count > 0) {
                handleResponse(backend.autosave());
            }
        }
    }

    Timer {
        id: fileWatchTimer
        interval: 2000
        repeat: true
        running: documentModel.count > 0
        onTriggered: {
            handleResponse(backend.poll_file_events());
        }
    }

    Kirigami.Dialog {
        id: unsavedDialog
        modal: true
        focus: true
        closePolicy: Popup.CloseOnEscape
        property var pendingDoc: null
        title: pendingDoc ? qsTr("Save changes to %1?").arg(pendingDoc.title) : ""

        contentItem: ColumnLayout {
            spacing: Kirigami.Units.mediumSpacing
            Label {
                visible: unsavedDialog.pendingDoc && unsavedDialog.pendingDoc.path
                text: unsavedDialog.pendingDoc ? unsavedDialog.pendingDoc.path : ""
                color: Kirigami.Theme.disabledTextColor
                wrapMode: Text.WordWrap
            }
            Label {
                text: qsTr("Your changes will be lost if you don’t save.")
                wrapMode: Text.WordWrap
            }
        }

        footer: RowLayout {
            spacing: Kirigami.Units.smallSpacing
            Layout.alignment: Qt.AlignRight
            Kirigami.Button {
                text: qsTr("Cancel")
                onClicked: cancelPendingClose()
            }
            Kirigami.Button {
                text: qsTr("Discard")
                icon.name: "edit-delete"
                onClicked: handlePendingDiscard()
            }
            Kirigami.Button {
                text: qsTr("Save")
                icon.name: "document-save"
                onClicked: handlePendingSave()
            }
        }
    }

    Kirigami.OverlaySheet {
        id: aboutSheet
        header: Kirigami.Heading {
            text: backend.appName()
        }
        contentItem: ColumnLayout {
            spacing: Kirigami.Units.mediumSpacing
            Label {
                text: qsTr("Version %1").arg(backend.appVersion())
            }
            Label {
                text: qsTr("App ID: %1").arg(backend.appId())
                wrapMode: Text.WrapAnywhere
            }
            Label {
                text: backend.welcome_tagline()
                wrapMode: Text.WordWrap
            }
        }
    }

    Kirigami.OverlaySheet {
        id: settingsSheet
        header: Kirigami.Heading {
            text: qsTr("Settings")
        }
        contentItem: ColumnLayout {
            spacing: Kirigami.Units.largeSpacing
            width: Math.min(parent.width, Kirigami.Units.gridUnit * 30)

            // Appearance Section
            Kirigami.Heading {
                level: 4
                text: qsTr("Appearance")
            }

            Kirigami.FormLayout {
                Layout.fillWidth: true

                ComboBox {
                    Kirigami.FormData.label: qsTr("Theme:")
                    model: gpTheme.options
                    textRole: "label"
                    valueRole: "value"
                    currentIndex: gpTheme.indexOf(themeVariant)
                    onActivated: themeVariant = gpTheme.options[currentIndex].value
                }

                Switch {
                    Kirigami.FormData.label: qsTr("Word wrap:")
                    checked: wordWrapEnabled
                    onToggled: wordWrapEnabled = checked
                }

                Switch {
                    Kirigami.FormData.label: qsTr("Translucent chrome:")
                    checked: translucencyEnabled
                    onToggled: translucencyEnabled = checked
                }

                Switch {
                    Kirigami.FormData.label: qsTr("Window shadow:")
                    checked: shadowEnabled
                    onToggled: shadowEnabled = checked
                }
            }

            Kirigami.Separator {
                Layout.fillWidth: true
            }

            // Search Section
            Kirigami.Heading {
                level: 4
                text: qsTr("Search Defaults")
            }

            Kirigami.FormLayout {
                Layout.fillWidth: true

                Switch {
                    Kirigami.FormData.label: qsTr("Case sensitive:")
                    checked: findCaseSensitive
                    onToggled: {
                        findCaseSensitive = checked;
                        saveFindDefaults();
                    }
                }

                Switch {
                    Kirigami.FormData.label: qsTr("Whole word:")
                    checked: findWholeWord
                    onToggled: {
                        findWholeWord = checked;
                        saveFindDefaults();
                    }
                }

                Switch {
                    Kirigami.FormData.label: qsTr("Regular expression:")
                    checked: findUseRegex
                    onToggled: {
                        findUseRegex = checked;
                        saveFindDefaults();
                    }
                }

                Switch {
                    Kirigami.FormData.label: qsTr("Wrap around:")
                    checked: findWrapAround
                    onToggled: {
                        findWrapAround = checked;
                        saveFindDefaults();
                    }
                }
            }

            Item {
                Layout.fillHeight: true
            }
        }
    }
}
