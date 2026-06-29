import { Save } from "lucide-react";
import { useEffect, useState } from "react";
import EdgeEntry from "../../components/EdgeEntry/EdgeEntry";
import { settingsFormToUpdate, settingsToForm, type SettingsFormState } from "../../localApi/settingsMapper";
import { updateSettings } from "../../localApi/localClient";
import { useAppState } from "../../store/appState";

export default function SettingsPage() {
  const { settings, refreshSettings } = useAppState();
  const [form, setForm] = useState<SettingsFormState | null>(settings ? settingsToForm(settings) : null);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    if (settings) setForm(settingsToForm(settings));
  }, [settings]);

  const save = async () => {
    if (!form) return;
    setMessage(null);
    await updateSettings(settingsFormToUpdate(form));
    await refreshSettings();
    setMessage("已保存到 daemon 配置。");
  };

  if (!form) return <p className="empty panel">设置不可用，daemon 未返回真实配置。</p>;

  return (
    <div className="single-column">
      <section className="section-head split">
        <div><h1>设置</h1><p>桌面应用偏好设置。</p></div>
        <button className="primary" onClick={save}><Save size={16} />保存</button>
      </section>
      {message && <p className="ok-text">{message}</p>}
      <section className="preference-grid">
        <div className="panel">
          <h2>连接</h2>
          <label>本机名称<input value={form.device_name} onChange={(e) => setForm({ ...form, device_name: e.target.value })} /></label>
          <label>对端名称<input value={form.peer_name} onChange={(e) => setForm({ ...form, peer_name: e.target.value })} /></label>
          <label>对端 host<input value={form.peer_host} onChange={(e) => setForm({ ...form, peer_host: e.target.value })} /></label>
          <label>对端端口<input type="number" value={form.peer_port} onChange={(e) => setForm({ ...form, peer_port: Number(e.target.value) })} /></label>
        </div>
        <div className="panel">
          <h2>文件</h2>
          <label>接收目录<input value={form.receive_dir} onChange={(e) => setForm({ ...form, receive_dir: e.target.value })} /></label>
          <p className="muted">原生拖拽投递和文件选择会使用系统层真实路径。</p>
        </div>
        <div className="panel">
          <h2>高速传输</h2>
          <label>并行任务<input type="number" min={1} max={8} value={form.max_concurrent_tasks} onChange={(e) => setForm({ ...form, max_concurrent_tasks: Number(e.target.value) })} /></label>
          <label>单文件连接<input type="number" min={1} max={16} value={form.parallel_chunk_uploads} onChange={(e) => setForm({ ...form, parallel_chunk_uploads: Number(e.target.value) })} /></label>
          <label>分片大小 MB<input type="number" min={1} max={2} value={Math.max(1, Math.round(form.chunk_size_bytes / 1048576))} onChange={(e) => setForm({ ...form, chunk_size_bytes: Number(e.target.value) * 1048576 })} /></label>
        </div>
        <div className="panel">
          <h2>剪贴板</h2>
          <label className="check-row"><input type="checkbox" checked={form.clipboard_enabled} onChange={(e) => setForm({ ...form, clipboard_enabled: e.target.checked })} />启用同步</label>
          <label className="check-row"><input type="checkbox" checked={form.clipboard_text_enabled} onChange={(e) => setForm({ ...form, clipboard_text_enabled: e.target.checked })} />文本同步</label>
          <label className="check-row"><input type="checkbox" checked={form.clipboard_image_enabled} onChange={(e) => setForm({ ...form, clipboard_image_enabled: e.target.checked })} />图片同步</label>
        </div>
        <div className="panel">
          <h2>外观</h2>
          <p className="muted">控制中心跟随系统浏览器渲染。原生主题细节留给后续桌面入口迭代。</p>
        </div>
      </section>
      <EdgeEntry />
    </div>
  );
}
