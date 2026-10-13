/** A calm, actionable account limit message within the conversation. */
import type { PlanNotice } from '../model/plan-notice.ts';
import { Icon } from '../../../ui/index.ts';
import './plan-notice.css';
/** Presentation and the browser-owned destination, with no arbitrary links. */
interface Props {
  /** Validated billing state. */
  notice: PlanNotice;
  /** Opens the connected workspace's billing page. */
  open: () => void;
}
/** Plan limits preserve the task context and provide one clear next action. */
export function PlanNoticeCard({ notice, open }: Props) {
  return (
    <section className="plan-notice" role="status" aria-label={notice.title}>
      <div className="plan-notice-top">
        <span className="plan-notice-mark" aria-hidden="true">
          <Icon name="spark" />
        </span>
        <span>{notice.label}</span>
      </div>
      <h2>{notice.title}</h2>
      {notice.usage && <p className="plan-notice-usage">{notice.usage}</p>}
      <p>{notice.body}</p>
      <button type="button" className="plan-notice-action" onClick={open}>
        {notice.action}
        <span aria-hidden="true">↗</span>
      </button>
      <p className="plan-notice-footnote">
        Already upgraded? Check that this browser and your subscription use the same Oya account.
      </p>
    </section>
  );
}
