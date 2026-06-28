import { PanelRight } from "lucide-react";

export default function EdgeEntry() {
  return (
    <section className="panel edge-entry">
      <div className="panel-title">
        <PanelRight size={18} />
        <h2>EdgeDropZone</h2>
      </div>
      <p className="muted">未接入。第一阶段仅保留入口和设置位，后续由系统层边缘投递能力接入。</p>
      <span className="badge neutral">后续系统层功能</span>
    </section>
  );
}
