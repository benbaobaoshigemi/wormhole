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
      <div className="pref-window">
        <header className="pref-header">
          <h2>Preferences</h2>
          <button className="icon-btn" onClick={onClose}><X size={20} /></button>
        </header>

        <div className="pref-body">
          <section className="pref-section">
            <h3>Connection</h3>
            <div className="pref-row">
              <label>Device Name</label>
              <input type="text" value={formData.device_name || ''} onChange={(e) => handleChange('device_name', e.target.value)} />
            </div>
            <div className="pref-row">
              <label>Peer Host IP</label>
              <input type="text" value={formData.peer_host || ''} onChange={(e) => handleChange('peer_host', e.target.value)} />
            </div>
            <div className="pref-row">
              <label>Peer Port</label>
              <input type="number" value={formData.peer_port || 53318} onChange={(e) => handleChange('peer_port', parseInt(e.target.value))} />
            </div>
          </section>

          <section className="pref-section">
            <h3>File Transfer</h3>
            <div className="pref-row">
              <label>Receive Directory</label>
              <input type="text" value={formData.receive_dir || ''} onChange={(e) => handleChange('receive_dir', e.target.value)} />
            </div>
          </section>

          <section className="pref-section">
            <h3>Clipboard Sync</h3>
            <div className="pref-row-checkbox">
              <input type="checkbox" id="cb-text" checked={formData.clipboard?.text_enabled || false} onChange={(e) => handleChange('clipboard.text_enabled', e.target.checked)} />
              <label htmlFor="cb-text">Enable Text Sync</label>
            </div>
            <div className="pref-row-checkbox">
              <input type="checkbox" id="cb-img" checked={formData.clipboard?.image_enabled || false} onChange={(e) => handleChange('clipboard.image_enabled', e.target.checked)} />
              <label htmlFor="cb-img">Enable Image Sync</label>
            </div>
          </section>
        </div>

        <footer className="pref-footer">
          <button className="btn-secondary" onClick={onClose}>Cancel</button>
          <button className="btn-primary" onClick={handleSave} disabled={saving}>
            <Save size={16} /> {saving ? 'Saving...' : 'Save Settings'}
          </button>
        </footer>
      </div>
    </div>
  );
}
