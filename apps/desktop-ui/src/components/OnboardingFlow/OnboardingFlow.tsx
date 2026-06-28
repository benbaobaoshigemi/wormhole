import React, { useState } from 'react';
import { useAppState } from '../../store/appState';
import { updateSettings } from '../../api/localClient';
import { ArrowRight, Check } from 'lucide-react';
import './OnboardingFlow.css';

export default function OnboardingFlow({ onComplete }: { onComplete: () => void }) {
  const { settings, refreshSettings } = useAppState();
  const [step, setStep] = useState(1);
  const [formData, setFormData] = useState(settings || {});
  const [saving, setSaving] = useState(false);

  const handleNext = () => setStep(s => s + 1);

  const handleFinish = async () => {
    setSaving(true);
    try {
      await updateSettings(formData);
      await refreshSettings();
      localStorage.setItem('wormhole_onboarding_complete', 'true');
      onComplete();
    } catch (e) {
      console.error(e);
    }
    setSaving(false);
  };

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

  return (
    <div className="onboarding-overlay">
      <div className="onboarding-window">
        <div className="onboarding-sidebar">
          <div className="ob-logo">Wormhole</div>
          <ul className="ob-steps">
            <li className={step >= 1 ? 'active' : ''}>Welcome</li>
            <li className={step >= 2 ? 'active' : ''}>Identity</li>
            <li className={step >= 3 ? 'active' : ''}>Connect Peer</li>
            <li className={step >= 4 ? 'active' : ''}>Features</li>
          </ul>
        </div>
        
        <div className="onboarding-content">
          {step === 1 && (
            <div className="ob-step-pane">
              <h2>Welcome to Wormhole</h2>
              <p>A seamless, secure, and blazing-fast way to connect your devices across the local network.</p>
              <div className="ob-spacer" />
              <button className="btn-primary" onClick={handleNext}>Get Started <ArrowRight size={16}/></button>
            </div>
          )}

          {step === 2 && (
            <div className="ob-step-pane">
              <h2>Identify This Device</h2>
              <p>Give this computer a recognizable name.</p>
              <div className="pref-row" style={{marginTop: 24}}>
                <label>Device Name</label>
                <input 
                  type="text" 
                  value={formData.device_name || ''} 
                  onChange={(e) => handleChange('device_name', e.target.value)} 
                  placeholder="e.g. My Windows PC"
                />
              </div>
              <div className="ob-spacer" />
              <button className="btn-primary" onClick={handleNext}>Next <ArrowRight size={16}/></button>
            </div>
          )}

          {step === 3 && (
            <div className="ob-step-pane">
              <h2>Connect to Peer</h2>
              <p>Enter the local IP address of the device you want to connect to.</p>
              <div className="pref-row" style={{marginTop: 24}}>
                <label>Peer IP Address</label>
                <input 
                  type="text" 
                  value={formData.peer_host || ''} 
                  onChange={(e) => handleChange('peer_host', e.target.value)} 
                  placeholder="e.g. 192.168.1.180"
                />
              </div>
              <div className="ob-spacer" />
              <button className="btn-primary" onClick={handleNext}>Next <ArrowRight size={16}/></button>
            </div>
          )}

          {step === 4 && (
            <div className="ob-step-pane">
              <h2>Enable Features</h2>
              <p>Choose what you want to sync automatically.</p>
              
              <div className="ob-features-list">
                <div className="pref-row-checkbox">
                  <input type="checkbox" id="ob-cb-text" checked={formData.clipboard?.text_enabled || false} onChange={(e) => handleChange('clipboard.text_enabled', e.target.checked)} />
                  <label htmlFor="ob-cb-text">Text Clipboard Sync</label>
                </div>
                <div className="pref-row-checkbox">
                  <input type="checkbox" id="ob-cb-img" checked={formData.clipboard?.image_enabled || false} onChange={(e) => handleChange('clipboard.image_enabled', e.target.checked)} />
                  <label htmlFor="ob-cb-img">Image Clipboard Sync</label>
                </div>
              </div>

              <div className="ob-spacer" />
              <button className="btn-primary" onClick={handleFinish} disabled={saving}>
                <Check size={16}/> {saving ? 'Saving...' : 'Finish Setup'}
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
