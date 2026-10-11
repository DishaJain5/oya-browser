/**
 * The account page's building blocks: a section (a small uppercase label over one
 * card) and its rows, each with what it is on the left and its control on the
 * right, separated by hairlines. One shape for every setting, so the page reads as
 * a list of decisions rather than a stack of differently drawn boxes.
 */
import type { ReactNode } from 'react';

/** What a section is given. */
interface SectionProps {
  /** The label's id, which names the section for assistive tech. */
  id: string;
  /** The small uppercase label. */
  label: string;
  /** Its rows. */
  children: ReactNode;
}

/** A labelled card of rows. */
export function SettingsSection({ id, label, children }: SectionProps) {
  return (
    <section className="settings-section" aria-labelledby={id}>
      <h4 className="profile-label" id={id}>
        {label}
      </h4>
      <div className="settings-card">{children}</div>
    </section>
  );
}

/** What a row is given. */
interface RowProps {
  /** What the setting is: its title, a line of detail, and any status under them. */
  children: ReactNode;
  /** The control on the right, if any. */
  control?: ReactNode;
  /** Hidden rows take no room. */
  hidden?: boolean;
  /** The control goes under the text, full width: for a row whose control is its main action. */
  wide?: boolean;
}

/** One setting: text on the left, its control on the right. */
export function SettingsRow({ children, control, hidden, wide }: RowProps) {
  return (
    <div className={wide ? 'settings-row wide' : 'settings-row'} hidden={hidden}>
      <div className="row-text">{children}</div>
      {control ? <div className="settings-control">{control}</div> : null}
    </div>
  );
}
