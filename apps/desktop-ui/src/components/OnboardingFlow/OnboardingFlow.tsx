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
      <div className="onboarding-window glass-panel">
        <div className="onboarding-sidebar">
          <div className="ob-logo">Wormhole</div>
          <ul className="ob-steps">
            <li className={step >= 1 ? 'active' : ''}>欢迎</li>
            <li className={step >= 2 ? 'active' : ''}>设备标识</li>
            <li className={step >= 3 ? 'active' : ''}>连接对端</li>
            <li className={step >= 4 ? 'active' : ''}>核心功能</li>
          </ul>
        </div>
        
        <div className="onboarding-content">
          {step === 1 && (
            <div className="ob-step-pane">
              <h2>欢迎使用 Wormhole</h2>
              <p>一种无缝、安全、极速的局域网跨设备互通方案。</p>
              <div className="ob-spacer" />
              <button className="btn-primary" onClick={handleNext}>开始使用 <ArrowRight size={16}/></button>
            </div>
          )}

          {step === 2 && (
            <div className="ob-step-pane">
              <h2>标识此设备</h2>
              <p>为这台电脑设置一个容易识别的名称。</p>
              <div className="pref-row" style={{marginTop: 24}}>
                <label>本机设备名称</label>
                <input 
                  type="text" 
                  value={formData.device_name || ''} 
                  onChange={(e) => handleChange('device_name', e.target.value)} 
                  placeholder="如: 我的 Windows 电脑"
                />
              </div>
              <div className="ob-spacer" />
              <button className="btn-primary" onClick={handleNext}>下一步 <ArrowRight size={16}/></button>
            </div>
          )}

          {step === 3 && (
            <div className="ob-step-pane">
              <h2>连接到对端设备</h2>
              <p>输入你要连接的局域网设备的 IP 地址。</p>
              <div className="pref-row" style={{marginTop: 24}}>
                <label>对端 IP 地址</label>
                <input 
                  type="text" 
                  value={formData.peer_host || ''} 
                  onChange={(e) => handleChange('peer_host', e.target.value)} 
                  placeholder="如: 192.168.1.180"
                />
              </div>
              <div className="ob-spacer" />
              <button className="btn-primary" onClick={handleNext}>下一步 <ArrowRight size={16}/></button>
            </div>
          )}

          {step === 4 && (
            <div className="ob-step-pane">
              <h2>启用功能</h2>
              <p>选择你想在后台自动同步的服务。</p>
              
              <div className="ob-features-list">
                <div className="pref-row-checkbox">
                  <input type="checkbox" id="ob-cb-text" checked={formData.clipboard?.text_enabled || false} onChange={(e) => handleChange('clipboard.text_enabled', e.target.checked)} />
                  <label htmlFor="ob-cb-text">启用文本剪贴板同步</label>
                </div>
                <div className="pref-row-checkbox">
                  <input type="checkbox" id="ob-cb-img" checked={formData.clipboard?.image_enabled || false} onChange={(e) => handleChange('clipboard.image_enabled', e.target.checked)} />
                  <label htmlFor="ob-cb-img">启用图片剪贴板同步</label>
                </div>
              </div>

              <div className="ob-spacer" />
              <button className="btn-primary" onClick={handleFinish} disabled={saving}>
                <Check size={16}/> {saving ? '正在保存...' : '完成配置'}
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
