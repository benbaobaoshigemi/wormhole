import React, { useState, useEffect } from 'react';
import { useAppState } from '../../store/appState';
import { updateSettings } from '../../api/localClient';
import { X, Save } from 'lucide-react';
import './PreferencePanel.css';

export default function PreferencePanel({ onClose }: { onClose: () => void }) {
  const { settings, refreshSettings } = useAppState();
  const [formData, setFormData] = useState<any>({});
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (settings) {
      setFormData(settings);
    }
  }, [settings]);

  const handleChange = (field: string, value: any) => {
    const keys = field.split('.');
    if (keys.length === 1) {
      setFormData({ ...formData, [field]: value });
    } else {
      setFormData({
        ...formData,
        [keys[0]]: {
          ...formData[keys[0]],
          [keys[1]]: value
        }
      });
    }
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      await updateSettings(formData);
      await refreshSettings();
      onClose();
    } catch (e) {
      console.error(e);
    }
    setSaving(false);
  };

  return (
    <div className="pref-overlay">
      <div className="pref-window glass-panel">
        <header className="pref-header">
          <h2>偏好设置</h2>
          <button className="icon-btn" onClick={onClose}><X size={20} /></button>
        </header>

        <div className="pref-body">
          <section className="pref-section">
            <h3>连接设置</h3>
            <div className="pref-row">
              <label>本机设备名称</label>
              <input type="text" value={formData.device_name || ''} onChange={(e) => handleChange('device_name', e.target.value)} />
            </div>
            <div className="pref-row">
              <label>对端设备 IP 地址</label>
              <input type="text" value={formData.peer_host || ''} onChange={(e) => handleChange('peer_host', e.target.value)} />
            </div>
            <div className="pref-row">
              <label>对端设备端口</label>
              <input type="number" value={formData.peer_port || 53318} onChange={(e) => handleChange('peer_port', parseInt(e.target.value))} />
            </div>
          </section>

          <section className="pref-section">
            <h3>文件传输</h3>
            <div className="pref-row">
              <label>默认接收目录</label>
              <input type="text" value={formData.receive_dir || ''} onChange={(e) => handleChange('receive_dir', e.target.value)} placeholder="如 C:\Users\xxx\Downloads" />
            </div>
          </section>

          <section className="pref-section">
            <h3>剪贴板同步</h3>
            <div className="pref-row-checkbox">
              <input type="checkbox" id="cb-text" checked={formData.clipboard?.text_enabled || false} onChange={(e) => handleChange('clipboard.text_enabled', e.target.checked)} />
              <label htmlFor="cb-text">启用文本剪贴板同步</label>
            </div>
            <div className="pref-row-checkbox">
              <input type="checkbox" id="cb-img" checked={formData.clipboard?.image_enabled || false} onChange={(e) => handleChange('clipboard.image_enabled', e.target.checked)} />
              <label htmlFor="cb-img">启用图片剪贴板同步</label>
            </div>
          </section>
        </div>

        <footer className="pref-footer">
          <button className="btn-secondary" onClick={onClose}>取消</button>
          <button className="btn-primary" onClick={handleSave} disabled={saving}>
            <Save size={16} /> {saving ? '保存中...' : '保存设置'}
          </button>
        </footer>
      </div>
    </div>
  );
}
