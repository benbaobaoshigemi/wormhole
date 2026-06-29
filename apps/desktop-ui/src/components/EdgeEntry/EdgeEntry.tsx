import { PanelRight } from "lucide-react";

export default function EdgeEntry() {
  return (
    <section className="panel edge-entry">
      <div className="panel-title">
        <PanelRight size={18} />
        <h2>原生拖拽投递</h2>
      </div>
      <p className="muted">已接入桌面壳。拖拽路径由 Windows 托盘或 macOS 菜单栏打开的原生窗口获取，浏览器控制中心不读取本机文件路径。</p>
      <span className="badge connected">可测试</span>
    </section>
  );
}
