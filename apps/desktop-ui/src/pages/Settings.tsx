import React, { useState, useEffect } from 'react';
import { useAppState } from '../store/appState';
import { updateSettings } from '../api/localClient';
import { Settings as SettingsIcon, Save } from 'lucide-react';

export default function Settings() {
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
    } catch (e) {
      console.error(e);
    }
    setSaving(false);
  };

  if (!settings) return null;

  return (
    <>
      <div className="page-header" style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center'}}>
        <div>
          <h1 className="page-title">Settings</h1>
          <p className="page-subtitle">Configure your Wormhole experience</p>
        </div>
        <button className="btn-primary" onClick={handleSave} disabled={saving}>
          <Save size={16} /> {saving ? 'Saving...' : 'Save Settings'}
        </button>
      </div>
      
      <div className="page-body">
        <div className="card">
          <div className="card-title" style={{marginBottom: 24}}>Device Configuration</div>
          
          <div style={{display: 'flex', flexDirection: 'column', gap: 16}}>
            <div>
              <label style={{display: 'block', marginBottom: 8, fontSize: '0.9rem', color: 'var(--text-secondary)'}}>Device Name</label>
              <input 
                type="text" 
                value={formData.device_name || ''} 
                onChange={(e) => handleChange('device_name', e.target.value)}
                style={{width: '100%', padding: '10px 12px', borderRadius: 8, border: '1px solid var(--border-color)', background: 'var(--bg-base)', color: 'var(--text-primary)'}}
              />
            </div>
            
            <div>
              <label style={{display: 'block', marginBottom: 8, fontSize: '0.9rem', color: 'var(--text-secondary)'}}>Peer Host</label>
              <input 
                type="text" 
                value={formData.peer_host || ''} 
                onChange={(e) => handleChange('peer_host', e.target.value)}
                style={{width: '100%', padding: '10px 12px', borderRadius: 8, border: '1px solid var(--border-color)', background: 'var(--bg-base)', color: 'var(--text-primary)'}}
              />
            </div>

            <div>
              <label style={{display: 'block', marginBottom: 8, fontSize: '0.9rem', color: 'var(--text-secondary)'}}>Peer Port</label>
              <input 
                type="number" 
                value={formData.peer_port || 53318} 
                onChange={(e) => handleChange('peer_port', parseInt(e.target.value))}
                style={{width: '100%', padding: '10px 12px', borderRadius: 8, border: '1px solid var(--border-color)', background: 'var(--bg-base)', color: 'var(--text-primary)'}}
              />
            </div>

            <div>
              <label style={{display: 'block', marginBottom: 8, fontSize: '0.9rem', color: 'var(--text-secondary)'}}>Receive Directory</label>
              <input 
                type="text" 
                value={formData.receive_dir || ''} 
                onChange={(e) => handleChange('receive_dir', e.target.value)}
                style={{width: '100%', padding: '10px 12px', borderRadius: 8, border: '1px solid var(--border-color)', background: 'var(--bg-base)', color: 'var(--text-primary)'}}
              />
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
