import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { TauriEvent } from '@tauri-apps/api/event';
import { getCurrentWindow, monitorFromPoint, PhysicalPosition } from '@tauri-apps/api/window';

interface Settings {
  baseUrl: string;
  model: string;
  translationEnabled: boolean;
  debugLoggingEnabled: boolean;
  apiKeyConfigured: boolean;
  maxAiWaitMs: number;
  debugLogPath?: string | null;
  interactionLogPath?: string | null;
  httpLogPath?: string | null;
}

interface CursorPoint {
  x: number;
  y: number;
}

interface PopupPayload {
  selectedText: string;
  explanation: string;
  sourceApp: string;
  windowTitle: string;
  latencyMs: number;
  error: string;
  cursorX: number;
  cursorY: number;
}

interface PopupLoadingPayload {
  selectedText: string;
  sourceApp: string;
  windowTitle: string;
  cursorX: number;
  cursorY: number;
  maxWaitMs: number;
}

interface NoticeState {
  tone: 'idle' | 'ok' | 'error';
  text: string;
}

interface HintBadgeProps {
  text: string;
}

const defaultSettings: Settings = {
  baseUrl: 'https://openrouter.ai/api/v1',
  model: 'x-ai/grok-4.1-fast',
  translationEnabled: false,
  debugLoggingEnabled: false,
  apiKeyConfigured: false,
  maxAiWaitMs: 15000,
  debugLogPath: null,
  interactionLogPath: null,
  httpLogPath: null,
};

const TRANSLATION_HOTKEY = 'Ctrl + Alt + Q';

function HintBadge({ text }: HintBadgeProps) {
  return (
    <span className="hint-badge" title={text} aria-label={text} role="note">
      ?
    </span>
  );
}

export default function App() {
  const currentWindow = getCurrentWindow();

  useEffect(() => {
    document.body.dataset.window = currentWindow.label;

    return () => {
      delete document.body.dataset.window;
    };
  }, [currentWindow.label]);

  return currentWindow.label === 'popup' ? <PopupWindow /> : <ControlPanelWindow />;
}

function ControlPanelWindow() {
  const appWindow = getCurrentWindow();
  const [settings, setSettings] = useState<Settings>(defaultSettings);
  const [apiKey, setApiKey] = useState('');
  const [settingsBusy, setSettingsBusy] = useState(false);
  const [credentialBusy, setCredentialBusy] = useState(false);
  const [runtimeBusyKey, setRuntimeBusyKey] = useState<'translationEnabled' | 'debugLoggingEnabled' | null>(null);
  const [notice, setNotice] = useState<NoticeState>({ tone: 'idle', text: '' });
  const [testStatus, setTestStatus] = useState<'idle' | 'testing' | 'ok' | 'fail'>('idle');
  const [testMessage, setTestMessage] = useState('');
  const maxAiWaitSeconds = Math.max(5, Math.round(settings.maxAiWaitMs / 1000));

  useEffect(() => {
    void loadSettings();
  }, []);

  useEffect(() => {
    const promise = appWindow.listen<Settings>('settings:changed', (event) => {
      setSettings(event.payload);
      setNotice({
        tone: 'ok',
        text: `自动监听已由系统热键 ${TRANSLATION_HOTKEY} 切换为${event.payload.translationEnabled ? '开启' : '关闭'}。`,
      });
    });

    return () => {
      void promise.then((unlisten) => unlisten());
    };
  }, [appWindow]);

  async function loadSettings() {
    try {
      const loaded = await invoke<Settings>('get_settings');
      setSettings(loaded);
    } catch (error) {
      setNotice({
        tone: 'error',
        text: typeof error === 'string' ? error : '读取设置失败',
      });
    }
  }

  async function handleSaveSettings() {
    setSettingsBusy(true);
    try {
      await invoke('save_settings', {
        settings: {
          baseUrl: settings.baseUrl,
          model: settings.model,
          translationEnabled: settings.translationEnabled,
          debugLoggingEnabled: settings.debugLoggingEnabled,
          maxAiWaitMs: settings.maxAiWaitMs,
        },
      });
      setNotice({ tone: 'ok', text: '设置已保存，后台监听配置已更新。' });
      await loadSettings();
    } catch (error) {
      setNotice({
        tone: 'error',
        text: typeof error === 'string' ? error : '保存设置失败',
      });
    } finally {
      setSettingsBusy(false);
    }
  }

  async function handleSaveApiKey() {
    if (!apiKey.trim()) {
      return;
    }

    setCredentialBusy(true);
    try {
      await invoke('save_api_key', { key: apiKey.trim() });
      setApiKey('');
      setSettings((current) => ({ ...current, apiKeyConfigured: true }));
      setNotice({ tone: 'ok', text: 'API Key 已写入系统凭据存储。' });
    } catch (error) {
      setNotice({
        tone: 'error',
        text: typeof error === 'string' ? error : '保存 API Key 失败',
      });
    } finally {
      setCredentialBusy(false);
    }
  }

  async function handleDeleteApiKey() {
    setCredentialBusy(true);
    try {
      await invoke('delete_api_key');
      setSettings((current) => ({ ...current, apiKeyConfigured: false }));
      setNotice({ tone: 'ok', text: 'API Key 已删除。' });
    } catch (error) {
      setNotice({
        tone: 'error',
        text: typeof error === 'string' ? error : '删除 API Key 失败',
      });
    } finally {
      setCredentialBusy(false);
    }
  }

  async function handleTestConnection() {
    setTestStatus('testing');
    setTestMessage('');
    try {
      const latency = await invoke<number>('test_connection');
      setTestStatus('ok');
      setTestMessage(`连接成功，往返 ${latency} ms`);
    } catch (error) {
      setTestStatus('fail');
      setTestMessage(typeof error === 'string' ? error : '连接失败，请检查 Base URL、模型名和 API Key');
    }
  }

  async function handleRuntimeToggle<K extends 'translationEnabled' | 'debugLoggingEnabled'>(key: K) {
    const nextSettings: Settings = {
      ...settings,
      [key]: !settings[key],
    };

    setSettings(nextSettings);
    setRuntimeBusyKey(key);
    try {
      await invoke('save_settings', {
        settings: {
          baseUrl: nextSettings.baseUrl,
          model: nextSettings.model,
          translationEnabled: nextSettings.translationEnabled,
          debugLoggingEnabled: nextSettings.debugLoggingEnabled,
          maxAiWaitMs: nextSettings.maxAiWaitMs,
        },
      });
      setNotice({
        tone: 'ok',
        text: key === 'translationEnabled'
          ? (nextSettings.translationEnabled ? '监听已开启并立即生效。' : '监听已关闭。')
          : (nextSettings.debugLoggingEnabled
            ? `调试日志已开启，runtime.log 和 interaction.log 会持续追加。`
            : '调试日志已关闭。'),
      });
      await loadSettings();
    } catch (error) {
      setSettings(settings);
      setNotice({
        tone: 'error',
        text: typeof error === 'string' ? error : '保存运行时开关失败',
      });
    } finally {
      setRuntimeBusyKey(null);
    }
  }

  async function handleHideToTray() {
    try {
      await appWindow.hide();
      setNotice({ tone: 'ok', text: '主窗口已隐藏到托盘，后台监听现在才会开始工作。' });
    } catch (error) {
      setNotice({
        tone: 'error',
        text: typeof error === 'string' ? error : '隐藏到托盘失败',
      });
    }
  }

  async function handleQuitApp() {
    await appWindow.close();
  }

  function handleMaxAiWaitSecondsChange(nextSeconds: number) {
    const normalizedSeconds = Math.min(120, Math.max(5, nextSeconds));
    setSettings((current) => ({
      ...current,
      maxAiWaitMs: normalizedSeconds * 1000,
    }));
  }

  return (
    <div className="control-shell compact-control-shell">
      <section className="settings-sheet">
        <div className="settings-toolbar">
          <div>
            <div className="eyebrow">后台划词取义</div>
            <h1>Select2Explain</h1>
          </div>

          <div className="settings-toolbar-actions">
            <button className="action-button badge-action-button toolbar-action-button" onClick={handleSaveSettings} disabled={settingsBusy} type="button">
              {settingsBusy ? '保存中...' : '保存设置'}
            </button>
            <button className="action-button badge-action-button toolbar-action-button" onClick={() => void handleHideToTray()} type="button">
              隐藏到托盘
            </button>
            <button className="action-button badge-action-button toolbar-action-button" onClick={() => void handleQuitApp()} type="button">
              退出应用
            </button>
          </div>
        </div>

        <div className="settings-group">
          <div className="settings-group-title">运行</div>

          <div className="settings-row">
            <div className="settings-row-title">
              <span>自动监听</span>
              <HintBadge text="鼠标松开后检查新的选区签名；系统热键也可以在任意应用里快速开关监听。" />
            </div>
            <div className="settings-row-control">
              <button
                className={`toggle-button ${settings.translationEnabled ? 'active' : ''}`}
                onClick={() => void handleRuntimeToggle('translationEnabled')}
                disabled={runtimeBusyKey !== null}
                type="button"
              >
                <span className="toggle-thumb" />
              </button>
            </div>
          </div>

          <div className="settings-row">
            <div className="settings-row-title">
              <span>本地调试日志</span>
              <HintBadge text="记录监听状态、全文采集、AI 请求分支和 popup 显示/隐藏轨迹。" />
            </div>
            <div className="settings-row-control settings-row-stack">
              <button
                className={`toggle-button ${settings.debugLoggingEnabled ? 'active' : ''}`}
                onClick={() => void handleRuntimeToggle('debugLoggingEnabled')}
                disabled={runtimeBusyKey !== null}
                type="button"
              >
                <span className="toggle-thumb" />
              </button>

              {settings.debugLoggingEnabled && (
                <div className="settings-note settings-log-note">
                  <span>runtime: {settings.debugLogPath || 'runtime.log'}</span>
                  <span>interaction: {settings.interactionLogPath || 'interaction.log'}</span>
                  <span>http: {settings.httpLogPath || 'http.log'}</span>
                </div>
              )}
            </div>
          </div>

          <div className="settings-row">
            <div className="settings-row-title">
              <span>AI 最长等待</span>
              <HintBadge text="超过这个时间仍未收到返回时，当前请求会自动取消。范围 5 到 120 秒。" />
            </div>
            <div className="settings-row-control compact-inline-control">
              <input
                className="field-input field-input-small"
                type="number"
                min={5}
                max={120}
                step={1}
                value={maxAiWaitSeconds}
                onChange={(event) => {
                  const nextSeconds = Number(event.target.value);
                  if (Number.isFinite(nextSeconds)) {
                    handleMaxAiWaitSecondsChange(nextSeconds);
                  }
                }}
              />
              <span className="unit-label">秒</span>
            </div>
          </div>

          <div className="settings-row">
            <div className="settings-row-title">
              <span>当前状态</span>
              <HintBadge text="主窗口聚焦时后台监听会暂停，切到其他应用后自动恢复。" />
            </div>
            <div className="settings-row-control multi-chip-control">
              <span className={`chip status-chip ${settings.translationEnabled ? 'online' : 'offline'}`}>
                <span className="status-chip-dot" />
                {settings.translationEnabled ? '监听已开启' : '监听已关闭'}
              </span>
              <span className={`chip status-chip ${settings.apiKeyConfigured ? 'online' : 'offline'}`}>
                <span className="status-chip-dot" />
                {settings.apiKeyConfigured ? 'API Key 已就绪' : 'API Key 未配置'}
              </span>
              <span className="chip">热键：{TRANSLATION_HOTKEY}</span>
            </div>
          </div>

        </div>

        <div className="settings-group">
          <div className="settings-group-title">Provider</div>

          <div className="settings-row">
            <div className="settings-row-title">
              <span>Base URL</span>
              <HintBadge text="OpenRouter 兼容接口地址。修改后点击“保存设置”才会生效。" />
            </div>
            <div className="settings-row-control wide-control">
              <input
                className="field-input"
                value={settings.baseUrl}
                onChange={(event) => setSettings((current) => ({ ...current, baseUrl: event.target.value }))}
                placeholder="https://openrouter.ai/api/v1"
              />
            </div>
          </div>

          <div className="settings-row">
            <div className="settings-row-title">
              <span>文本模型</span>
              <HintBadge text="默认建议使用更快的文本模型；修改后点击“保存设置”才会生效。" />
            </div>
            <div className="settings-row-control wide-control">
              <input
                className="field-input"
                value={settings.model}
                onChange={(event) => setSettings((current) => ({ ...current, model: event.target.value }))}
                placeholder="x-ai/grok-4.1-fast"
              />
            </div>
          </div>

          <div className="settings-row">
            <div className="settings-row-title">
              <span>API Key</span>
              <HintBadge text="只在本机系统凭据存储里保存，不会写入仓库。按 Enter 或点“保存 Key”即可写入。" />
            </div>
            <div className="settings-row-control wide-control">
              <input
                className="field-input"
                type="password"
                value={apiKey}
                onChange={(event) => setApiKey(event.target.value)}
                placeholder="sk-..."
                onKeyDown={(event) => {
                  if (event.key === 'Enter') {
                    void handleSaveApiKey();
                  }
                }}
              />
            </div>
          </div>

          <div className="settings-row">
            <div className="settings-row-title">
              <span>凭据与测试</span>
              <HintBadge text="先保存 Key，再执行连接测试。删除 Key 会立即清除本机凭据。" />
            </div>
            <div className="settings-row-control provider-actions-row">
              <span className={`badge ${settings.apiKeyConfigured ? 'ok' : 'warn'}`}>
                {settings.apiKeyConfigured ? 'API Key 已配置' : 'API Key 未配置'}
              </span>
              <button className="action-button badge-action-button provider-action-button" onClick={handleSaveApiKey} disabled={credentialBusy || !apiKey.trim()} type="button">
                保存 Key
              </button>
              <button className="action-button badge-action-button provider-action-button" onClick={handleTestConnection} disabled={!settings.apiKeyConfigured || testStatus === 'testing'} type="button">
                {testStatus === 'testing' ? '测试中...' : '测试连接'}
              </button>
              <button className="action-button badge-action-button provider-action-button" onClick={handleDeleteApiKey} disabled={!settings.apiKeyConfigured || credentialBusy} type="button">
                删除 Key
              </button>
            </div>
          </div>

          {testMessage && (
            <div className={`inline-message ${testStatus === 'ok' ? 'ok' : 'error'}`}>
              {testMessage}
            </div>
          )}
        </div>
      </section>

      {notice.text && (
        <div className={`notice-bar ${notice.tone === 'ok' ? 'ok' : notice.tone === 'error' ? 'error' : ''}`}>
          {notice.text}
        </div>
      )}
    </div>
  );
}

function PopupWindow() {
  const appWindow = getCurrentWindow();
  const [payload, setPayload] = useState<PopupPayload | null>(null);
  const [loadingPayload, setLoadingPayload] = useState<PopupLoadingPayload | null>(null);
  const lastShownAtRef = useRef(0);
  const autoHideTimerRef = useRef<number | null>(null);
  const popupRootRef = useRef<HTMLDivElement | null>(null);
  const popupCardRef = useRef<HTMLDivElement | null>(null);
  const popupBodyRef = useRef<HTMLDivElement | null>(null);
  const popupMeasureCardRef = useRef<HTMLDivElement | null>(null);
  const lastCursorRef = useRef<CursorPoint>({ x: 0, y: 0 });
  const lastAppliedHeightRef = useRef(0);

  function toPhysicalPixels(value: number, scaleFactor: number) {
    return Math.max(1, Math.round(value * scaleFactor));
  }

  function measurePopupContentHeight(measureCard: HTMLDivElement, width: number) {
    const clone = measureCard.cloneNode(true) as HTMLDivElement;
    clone.style.position = 'fixed';
    clone.style.left = '-10000px';
    clone.style.top = '0';
    clone.style.width = `${width}px`;
    clone.style.height = 'auto';
    clone.style.maxHeight = 'none';
    clone.style.overflow = 'visible';
    clone.style.visibility = 'hidden';
    clone.style.pointerEvents = 'none';
    clone.style.zIndex = '-1';
    document.body.appendChild(clone);

    const measuredHeight = Math.ceil(
      Math.max(clone.getBoundingClientRect().height, clone.scrollHeight),
    );

    clone.remove();
    return measuredHeight;
  }

  useEffect(() => {
    function clearAutoHide() {
      if (autoHideTimerRef.current !== null) {
        window.clearTimeout(autoHideTimerRef.current);
        autoHideTimerRef.current = null;
      }
    }

    async function clearPopup(options?: { cancel?: boolean; reason?: string }) {
      clearAutoHide();
      if (options?.cancel) {
        void invoke('cancel_popup_request', {
          reason: options.reason ?? 'popup hidden from frontend',
        }).catch(() => undefined);
      }
      setPayload(null);
      setLoadingPayload(null);
      lastAppliedHeightRef.current = 0;
      await appWindow.hide();
    }

    function scheduleHide(delayMs: number) {
      clearAutoHide();
      autoHideTimerRef.current = window.setTimeout(() => {
        void clearPopup({ cancel: true, reason: 'popup auto-hide timeout reached' });
        autoHideTimerRef.current = null;
      }, delayMs);
    }

    async function revealPopup(cursorX: number, cursorY: number, shouldFocus: boolean) {
      lastCursorRef.current = { x: cursorX, y: cursorY };
      lastShownAtRef.current = Date.now();
      await appWindow.show();
      await syncPopupFrame(cursorX, cursorY);
      if (shouldFocus) {
        await appWindow.setFocus();
      }
    }

    const loadingPromise = appWindow.listen<PopupLoadingPayload>('popup:loading', async (event) => {
      setLoadingPayload(event.payload);
      setPayload(null);
      await revealPopup(event.payload.cursorX, event.payload.cursorY, false);
      scheduleHide(event.payload.maxWaitMs + 1500);
    });

    const showPromise = appWindow.listen<PopupPayload>('popup:show', async (event) => {
      setLoadingPayload(null);
      setPayload(event.payload);
      await revealPopup(event.payload.cursorX, event.payload.cursorY, true);
      scheduleHide(12000);
    });

    const clearPromise = appWindow.listen('popup:clear', async () => {
      await clearPopup({ cancel: false });
    });

    const blurPromise = appWindow.listen(TauriEvent.WINDOW_BLUR, async () => {
      if (Date.now() - lastShownAtRef.current < 350) {
        return;
      }
      await clearPopup({ cancel: true, reason: 'popup window lost focus' });
    });

    return () => {
      clearAutoHide();
      void loadingPromise.then((unlisten) => unlisten());
      void showPromise.then((unlisten) => unlisten());
      void clearPromise.then((unlisten) => unlisten());
      void blurPromise.then((unlisten) => unlisten());
    };
  }, [appWindow]);

  useEffect(() => {
    if (!loadingPayload && !payload) {
      return;
    }

    const root = popupRootRef.current;
    const card = popupCardRef.current;
    const body = popupBodyRef.current;
    const measureCard = popupMeasureCardRef.current;
    if (!root || !card || !body || !measureCard) {
      return;
    }

    let settleTimer = 0;
    let finalTimer = 0;
    let frame = 0;
    let disposed = false;

    const scheduleSync = () => {
      window.cancelAnimationFrame(frame);
      frame = window.requestAnimationFrame(() => {
        if (disposed) {
          return;
        }

        const { x, y } = lastCursorRef.current;
        void syncPopupFrame(x, y);
      });
    };

    const observer = new ResizeObserver(() => {
      scheduleSync();
    });

    observer.observe(root);
    observer.observe(card);
    observer.observe(body);
    observer.observe(measureCard);
    scheduleSync();
    settleTimer = window.setTimeout(scheduleSync, 40);
    finalTimer = window.setTimeout(scheduleSync, 120);

    return () => {
      disposed = true;
      observer.disconnect();
      window.cancelAnimationFrame(frame);
      window.clearTimeout(settleTimer);
      window.clearTimeout(finalTimer);
    };
  }, [loadingPayload, payload]);

  async function syncPopupFrame(cursorX: number, cursorY: number) {
    const root = popupRootRef.current;
    const card = popupCardRef.current;
    const body = popupBodyRef.current;
    const measureCard = popupMeasureCardRef.current;
    if (!root || !card || !body || !measureCard) {
      await positionPopup(cursorX, cursorY);
      return;
    }

    await new Promise<void>((resolve) => {
      window.requestAnimationFrame(() => resolve());
    });

    const monitor = await monitorFromPoint(cursorX, cursorY);
    const scaleFactor = await appWindow.scaleFactor();
    const width = 424;
    const naturalCardHeight = measurePopupContentHeight(measureCard, width);
    const fallbackBodyHeight = Math.ceil(body.scrollHeight + 112);
    const contentHeight = Math.max(naturalCardHeight + 2, fallbackBodyHeight, 168);
    const nextHeight = contentHeight;
    const physicalWidth = toPhysicalPixels(width, scaleFactor);
    const physicalHeight = toPhysicalPixels(nextHeight, scaleFactor);

    if (Math.abs(nextHeight - lastAppliedHeightRef.current) > 2) {
      await invoke('resize_popup_window', {
        width: physicalWidth,
        height: physicalHeight,
      });
      lastAppliedHeightRef.current = nextHeight;
    }

    await positionPopup(cursorX, cursorY, physicalWidth, physicalHeight, monitor ?? undefined, scaleFactor);
  }

  async function positionPopup(
    cursorX: number,
    cursorY: number,
    widthOverride?: number,
    heightOverride?: number,
    knownMonitor?: Awaited<ReturnType<typeof monitorFromPoint>>,
    knownScaleFactor?: number,
  ) {
    const monitor = knownMonitor ?? await monitorFromPoint(cursorX, cursorY);
    const scaleFactor = knownScaleFactor ?? await appWindow.scaleFactor();
    const size = widthOverride && heightOverride ? null : await appWindow.outerSize();
    const width = widthOverride ?? size?.width ?? 408;
    const height = heightOverride ?? size?.height ?? 248;
    const horizontalOffset = toPhysicalPixels(16, scaleFactor);
    const rightOffset = toPhysicalPixels(18, scaleFactor);
    const topGap = toPhysicalPixels(18, scaleFactor);
    const edgeInset = toPhysicalPixels(12, scaleFactor);
    const minTopInset = toPhysicalPixels(8, scaleFactor);

    if (!monitor) {
      await appWindow.setPosition(
        new PhysicalPosition(cursorX + horizontalOffset, Math.max(minTopInset, cursorY - height - topGap)),
      );
      return;
    }

    const area = monitor.workArea;
    const maxX = area.position.x + area.size.width - width - edgeInset;
    const minX = area.position.x + edgeInset;
    const idealX = cursorX + rightOffset;
    const x = Math.max(minX, Math.min(maxX, idealX));

    const topInset = area.position.y + minTopInset;
    const preferredY = cursorY - height - topGap;
    const fallbackY = cursorY + topGap;
    const overflowBottomY = area.position.y + area.size.height - height - edgeInset;
    const y = height >= area.size.height - toPhysicalPixels(20, scaleFactor)
      ? topInset
      : preferredY >= topInset
        ? preferredY
        : Math.max(topInset, Math.min(overflowBottomY, fallbackY));

    await appWindow.setPosition(new PhysicalPosition(x, y));
  }

  const isLoading = !!loadingPayload && !payload;
  const selectedText = payload?.selectedText || loadingPayload?.selectedText;
  const sourceApp = payload?.sourceApp || loadingPayload?.sourceApp || 'Select2Explain';
  const windowTitle = payload?.windowTitle || loadingPayload?.windowTitle || '点击别处会自动隐藏';
  const footerText = !isLoading && payload?.latencyMs ? `${payload.latencyMs} ms` : !isLoading ? `热键 ${TRANSLATION_HOTKEY}` : '';

  return (
    <div className="popup-root" ref={popupRootRef}>
      <div className="popup-card" ref={popupCardRef}>
        <div className="popup-topline">
          <span className={`popup-pill ${isLoading ? 'loading' : ''}`}>{isLoading ? 'AI Loading' : 'AI Explain'}</span>
          <span className="popup-app">{sourceApp}</span>
        </div>

        {selectedText && (
          <blockquote className="popup-selection">{selectedText}</blockquote>
        )}

        {isLoading ? (
          <div className="popup-loading" ref={popupBodyRef}>
            <span className="popup-spinner" aria-hidden="true" />
            <div className="popup-loading-copy">
              <strong>正在生成解释</strong>
            </div>
          </div>
        ) : payload?.error ? (
          <div className="popup-error" ref={popupBodyRef}>{payload.error}</div>
        ) : (
          <div className="popup-explanation" ref={popupBodyRef}>{payload?.explanation || '等待新的选区...'}</div>
        )}

        <div className="popup-footer">
          <span>{windowTitle}</span>
          {footerText && <span>{footerText}</span>}
        </div>
      </div>

      <div className="popup-measure-layer" aria-hidden="true">
        <div className="popup-card popup-card-measure" ref={popupMeasureCardRef}>
          <div className="popup-topline">
            <span className={`popup-pill ${isLoading ? 'loading' : ''}`}>{isLoading ? 'AI Loading' : 'AI Explain'}</span>
            <span className="popup-app">{sourceApp}</span>
          </div>

          {selectedText && (
            <blockquote className="popup-selection">{selectedText}</blockquote>
          )}

          {isLoading ? (
            <div className="popup-loading">
              <span className="popup-spinner" aria-hidden="true" />
              <div className="popup-loading-copy">
                <strong>正在生成解释</strong>
              </div>
            </div>
          ) : payload?.error ? (
            <div className="popup-error">{payload.error}</div>
          ) : (
            <div className="popup-explanation">{payload?.explanation || '等待新的选区...'}</div>
          )}

          <div className="popup-footer">
            <span>{windowTitle}</span>
            {footerText && <span>{footerText}</span>}
          </div>
        </div>
      </div>
    </div>
  );
}
