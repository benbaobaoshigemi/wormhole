import { PanelRight } from "lucide-react";

export default function EdgeEntry() {
  return (
    <section className="panel edge-entry">
      <div className="panel-title">
        <PanelRight size={18} />
        <h2>边缘拖拽投递</h2>
      </div>
      <p className="muted">已接入桌面壳右侧边缘投递层。平时不显示，拖文件进入屏幕边缘时展开；路径由系统原生 drop 事件提供。</p>
      <span className="badge connected">可测试</span>
    </section>
  );
}
