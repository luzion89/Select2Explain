import { useEffect, useState, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

interface Settings {
  baseUrl: string;
  model: string;
  visionModel: string;
  apiKeyConfigured: boolean;
}

interface ExplainResult {
  explanation: string;
  sourceApp: string;
  latencyMs: number;
}

type AppView = 'settings' | 'monitor';

export default function App() {
  const [view, setView] = useState<AppView>('settings');
  const [settings, setSettings] = useState<Settings>({
    baseUrl: 'https://api.openai.com/v1',
    model: 'qwen/qwen2.5-vl-72b-instruct',
    visionModel: 'qwen/qwen2.5-vl-72b-instruct',
    apiKeyConfigured: false,
  });
  const [apiKey, setApiKey] = useState('');
  const [saving, setSaving] = useState(false);
  const [saveMsg, setSaveMsg] = useState('');
  const [testStatus, setTestStatus] = useState<'idle' | 'testing' | 'ok' | 'fail'>('idle');
  const [testMsg, setTestMsg] = useState('');

  const [popup, setPopup] = useState<{
    visible: boolean;
    text: string;
    result: ExplainResult | null;
    loading: boolean;
    error: string;
  }>({ visible: false, text: '', result: null, loading: false, error: '' });

  const popupRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    invoke<Settings>('get_settings').then(setSettings).catch(console.error);
  }, []);

  useEffect(() => {
    const unlistenPromise = listen<string>('selection-changed', async (event) => {
      const selectedText = event.payload;
      if (!selectedText.trim()) return;

      setPopup({ visible: true, text: selectedText, result: null, loading: true, error: '' });
      setView('monitor');

      try {
        const result = await invoke<ExplainResult>('explain_selection', { selectedText });
        setPopup(p => ({ ...p, loading: false, result, error: '' }));
      } catch (e: unknown) {
        setPopup(p => ({
          ...p,
          loading: false,
          error: typeof e === 'string' ? e : 'AI 请求失败，请检查 API Key 和网络',
        }));
      }
    });
    return () => { unlistenPromise.then(fn => fn()); };
  }, []);

  const handleSaveSettings = async () => {
    setSaving(true);
    setSaveMsg('');
    try {
      await invoke('save_settings', {
        settings: {
          baseUrl: settings.baseUrl,
          model: settings.model,
          visionModel: settings.visionModel,
        },
      });
      setSaveMsg('配置已保存');
    } catch {
      setSaveMsg('保存失败');
    } finally {
      setSaving(false);
      setTimeout(() => setSaveMsg(''), 2000);
    }
  };

  const handleSaveApiKey = async () => {
    if (!apiKey.trim()) return;
    setSaving(true);
    try {
      await invoke('save_api_key', { key: apiKey });
      setApiKey('');
      setSettings(s => ({ ...s, apiKeyConfigured: true }));
      setSaveMsg('API Key 已保存到系统钥匙串');
    } catch {
      setSaveMsg('保存 API Key 失败');
    } finally {
      setSaving(false);
      setTimeout(() => setSaveMsg(''), 2500);
    }
  };

  const handleDeleteApiKey = async () => {
    try {
      await invoke('delete_api_key');
      setSettings(s => ({ ...s, apiKeyConfigured: false }));
      setSaveMsg('API Key 已删除');
      setTimeout(() => setSaveMsg(''), 2000);
    } catch {
      setSaveMsg('删除失败');
    }
  };

  const handleTestConnection = async () => {
    setTestStatus('testing');
    setTestMsg('');
    try {
      const latency = await invoke<number>('test_connection');
      setTestStatus('ok');
      setTestMsg(`连接成功，延迟 ${latency}ms`);
    } catch (e: unknown) {
      setTestStatus('fail');
      setTestMsg(typeof e === 'string' ? e : '连接失败，请检查 API Key 和 Base URL');
    }
  };

  const closePopup = () => setPopup(p => ({ ...p, visible: false }));

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="logo">S2E</div>
        <nav>
          <button
            className={view === 'monitor' ? 'nav-btn active' : 'nav-btn'}
            onClick={() => setView('monitor')}
          >监控</button>
          <button
            className={view === 'settings' ? 'nav-btn active' : 'nav-btn'}
            onClick={() => setView('settings')}
          >设置</button>
        </nav>
        <div className="sidebar-status">
          <span className={settings.apiKeyConfigured ? 'dot green' : 'dot red'} />
          {settings.apiKeyConfigured ? 'AI 已就绪' : '未配置'}
        </div>
      </aside>

      <main className="content">
        {view === 'settings' && (
          <SettingsView
            settings={settings}
            apiKey={apiKey}
            saving={saving}
            saveMsg={saveMsg}
            testStatus={testStatus}
            testMsg={testMsg}
            onSettingsChange={setSettings}
            onApiKeyChange={setApiKey}
            onSaveSettings={handleSaveSettings}
            onSaveApiKey={handleSaveApiKey}
            onDeleteApiKey={handleDeleteApiKey}
            onTestConnection={handleTestConnection}
          />
        )}
        {view === 'monitor' && (
          <MonitorView popup={popup} onClose={closePopup} popupRef={popupRef} />
        )}
      </main>
    </div>
  );
}

function SettingsView({
  settings, apiKey, saving, saveMsg, testStatus, testMsg,
  onSettingsChange, onApiKeyChange,
  onSaveSettings, onSaveApiKey, onDeleteApiKey, onTestConnection,
}: {
  settings: Settings;
  apiKey: string;
  saving: boolean;
  saveMsg: string;
  testStatus: 'idle' | 'testing' | 'ok' | 'fail';
  testMsg: string;
  onSettingsChange: (s: Settings) => void;
  onApiKeyChange: (k: string) => void;
  onSaveSettings: () => void;
  onSaveApiKey: () => void;
  onDeleteApiKey: () => void;
  onTestConnection: () => void;
}) {
  return (
    <div className="settings-page">
      <h2>Provider 配置</h2>

      <label className="field-label">Base URL</label>
      <input
        className="field-input"
        value={settings.baseUrl}
        onChange={e => onSettingsChange({ ...settings, baseUrl: e.target.value })}
        placeholder="https://api.openai.com/v1"
      />

      <label className="field-label">文本模型（用于解释）</label>
      <input
        className="field-input"
        value={settings.model}
        onChange={e => onSettingsChange({ ...settings, model: e.target.value })}
        placeholder="qwen/qwen2.5-vl-72b-instruct"
      />

      <label className="field-label">视觉模型（用于截图判断）</label>
      <input
        className="field-input"
        value={settings.visionModel}
        onChange={e => onSettingsChange({ ...settings, visionModel: e.target.value })}
        placeholder="qwen/qwen2.5-vl-72b-instruct"
      />

      <button className="btn primary" onClick={onSaveSettings} disabled={saving}>
        保存配置
      </button>

      <div className="divider" />

      <h2>API Key</h2>
      <p className="hint">
        {settings.apiKeyConfigured
          ? '✓ API Key 已配置（存储于系统钥匙串）'
          : '尚未配置 API Key'}
      </p>

      <label className="field-label">输入新 Key</label>
      <input
        className="field-input"
        type="password"
        value={apiKey}
        onChange={e => onApiKeyChange(e.target.value)}
        placeholder="sk-..."
        onKeyDown={e => { if (e.key === 'Enter') onSaveApiKey(); }}
      />

      <div className="row-gap">
        <button className="btn primary" onClick={onSaveApiKey} disabled={saving || !apiKey.trim()}>
          保存 Key
        </button>
        {settings.apiKeyConfigured && (
          <button className="btn danger" onClick={onDeleteApiKey}>
            删除 Key
          </button>
        )}
        <button
          className="btn secondary"
          onClick={onTestConnection}
          disabled={!settings.apiKeyConfigured || testStatus === 'testing'}
        >
          {testStatus === 'testing' ? '测试中…' : '测试连接'}
        </button>
      </div>

      {saveMsg && <p className="save-msg">{saveMsg}</p>}
      {testMsg && (
        <p className={`save-msg ${testStatus === 'ok' ? 'ok' : testStatus === 'fail' ? 'err' : ''}`}>
          {testMsg}
        </p>
      )}

      <div className="divider" />
      <p className="hint small">
        配置完成后，在任意应用中长按拖动选中文字，Select2Explain 会自动检测并弹出解释。
        需要在「系统设置 → 隐私与安全性 → 辅助功能」中授权本应用。
      </p>
    </div>
  );
}

function MonitorView({
  popup, onClose, popupRef,
}: {
  popup: { visible: boolean; text: string; result: ExplainResult | null; loading: boolean; error: string };
  onClose: () => void;
  popupRef: React.RefObject<HTMLDivElement>;
}) {
  if (!popup.visible && !popup.text) {
    return (
      <div className="monitor-idle">
        <div className="idle-icon">👁</div>
        <p>正在监听选中文字…</p>
        <p className="hint">在任意应用中选中文字，解释将自动显示在这里。</p>
      </div>
    );
  }

  return (
    <div className="monitor-active" ref={popupRef}>
      <div className="selected-text-label">选中的文字</div>
      <blockquote className="selected-text">{popup.text}</blockquote>

      {popup.loading && (
        <div className="loading-row">
          <span className="spinner" />
          <span>AI 分析中…</span>
        </div>
      )}

      {popup.error && (
        <div className="error-box">{popup.error}</div>
      )}

      {popup.result && !popup.loading && (
        <div className="result-box">
          <div className="result-explanation">{popup.result.explanation}</div>
          <div className="result-meta">
            来源：{popup.result.sourceApp}&nbsp;·&nbsp;{popup.result.latencyMs}ms
          </div>
        </div>
      )}

      <button className="btn secondary close-btn" onClick={onClose}>关闭</button>
    </div>
  );
}
