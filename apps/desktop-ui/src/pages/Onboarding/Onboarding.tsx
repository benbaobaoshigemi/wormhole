import { Check, ChevronRight } from "lucide-react";
import { useEffect, useState } from "react";
import { settingsFormToUpdate, settingsToForm, type SettingsFormState } from "../../localApi/settingsMapper";
import { updateSettings } from "../../localApi/localClient";
import { useAppState } from "../../store/appState";

export default function Onboarding({ onComplete }: { onComplete: () => void }) {
  const { device, settings, connectionStatus, refreshSettings } = useAppState();
  const [step, setStep] = useState(0);
  const [form, setForm] = useState<SettingsFormState | null>(settings ? settingsToForm(settings) : null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (settings) setForm(settingsToForm(settings));
  }, [settings]);

  const save = async () => {
    if (!form) return;
    try {
      setError(null);
      await updateSettings(settingsFormToUpdate(form));
      await refreshSettings();
      localStorage.setItem("wormhole_onboarding_complete", "true");
      onComplete();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div className="onboarding">
      <section className="onboarding-card">
        <h1>Wormhole</h1>
        <p>两台电脑互通文件和剪贴板的桌面工具。</p>
        {step === 0 && <p className="muted">本机：{device?.device_name ?? "等待 daemon"} · local API {device ? "可用" : "不可用"}</p>}
        {step === 1 && form && (
          <div className="form-grid">
            <label>本机名称<input value={form.device_name} onChange={(e) => setForm({ ...form, device_name: e.target.value })} /></label>
            <label>对端名称<input value={form.peer_name} onChange={(e) => setForm({ ...form, peer_name: e.target.value })} /></label>
            <label>对端 host<input value={form.peer_host} onChange={(e) => setForm({ ...form, peer_host: e.target.value })} /></label>
            <label>对端端口<input type="number" value={form.peer_port} onChange={(e) => setForm({ ...form, peer_port: Number(e.target.value) })} /></label>
          </div>
        )}
        {step === 2 && form && (
          <div className="form-grid">
            <label>接收目录<input value={form.receive_dir} onChange={(e) => setForm({ ...form, receive_dir: e.target.value })} /></label>
            <label className="check-row"><input type="checkbox" checked={form.clipboard_enabled} onChange={(e) => setForm({ ...form, clipboard_enabled: e.target.checked })} />剪贴板同步总开关</label>
            <label className="check-row"><input type="checkbox" checked={form.clipboard_text_enabled} onChange={(e) => setForm({ ...form, clipboard_text_enabled: e.target.checked })} />文本同步</label>
            <label className="check-row"><input type="checkbox" checked={form.clipboard_image_enabled} onChange={(e) => setForm({ ...form, clipboard_image_enabled: e.target.checked })} />图片同步</label>
          </div>
        )}
        {step === 3 && <p className="muted">当前连接状态：{connectionStatus.replace("_", " ")}。完成后进入控制中心。</p>}
        {error && <p className="error-text">{error}</p>}
        <div className="wizard-actions">
          {step < 3 ? (
            <button className="primary" onClick={() => setStep(step + 1)} disabled={!form && step > 0}>
              下一步 <ChevronRight size={16} />
            </button>
          ) : (
            <button className="primary" onClick={save} disabled={!form}>
              <Check size={16} /> 保存并进入
            </button>
          )}
        </div>
      </section>
    </div>
  );
}
