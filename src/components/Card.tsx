import type { Lang } from "../i18n";
import { strings } from "../i18n";
import type { Notification } from "../types";
import type { Character, Mood } from "../character/types";
import Cat from "./Cat";

const STATUS_ICON = { ok: "✓", fail: "✕", attention: "!", info: "i" } as const;

interface CardProps {
  notification: Notification;
  lang: Lang;
  leaving: boolean;
  hovered: boolean;
  pixelSize: number;
  character?: Character;
  overlay?: Partial<Record<Mood, string>>;
  onActivate: () => void;
  onDismiss: () => void;
}

export default function Card({
  notification: n,
  lang,
  leaving,
  hovered,
  pixelSize,
  character,
  overlay,
  onActivate,
  onDismiss,
}: CardProps) {
  const t = strings(lang);
  const classes = ["card", `status-${n.status}`];
  if (leaving) classes.push("is-leaving");
  if (hovered) classes.push("is-hovered");
  if (n.url) classes.push("has-link");
  if (n.sticky) classes.push("is-sticky");

  return (
    <div className={classes.join(" ")} data-card-id={n.id} onClick={leaving ? undefined : onActivate}>
      <Cat mood={n.status} pixelSize={pixelSize} character={character} overlay={overlay} />
      <div className="bubble">
        <div className="bubble-head">
          <span className="badge">
            <span className="badge-icon" aria-hidden="true">
              {STATUS_ICON[n.status]}
            </span>
            {t.statusLabel[n.status]}
          </span>
          <span className="title" title={n.title ?? undefined}>
            {n.title ?? t.sourceLabel[n.source]}
          </span>
        </div>
        <p className="message">{n.message ?? t.defaultMessage[n.status]}</p>
        {n.progress && n.progress.total > 0 && <ProgressBar {...n.progress} />}
        {n.project && <p className="project">{n.project}</p>}
        <button
          type="button"
          className="close"
          aria-label={t.dismiss}
          title={t.dismiss}
          onClick={(e) => {
            e.stopPropagation();
            onDismiss();
          }}
        >
          ×
        </button>
      </div>
    </div>
  );
}

function ProgressBar({ done, total, failed }: { done: number; total: number; failed: number }) {
  const pct = (n: number) => `${Math.max(0, Math.min(100, (n / total) * 100))}%`;
  return (
    <div
      className="progress"
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={total}
      aria-valuenow={done}
      aria-valuetext={`${done}/${total}`}
    >
      <span className="progress-ok" style={{ width: pct(Math.max(0, done - failed)) }} />
      <span className="progress-failed" style={{ width: pct(failed) }} />
    </div>
  );
}
