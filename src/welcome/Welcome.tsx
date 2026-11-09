import { useCallback, useEffect, useState, type ReactNode } from "react";
import Cat from "../components/Cat";
import type { Lang } from "../i18n";
import type { OnboardingStatus, WelcomeApi } from "./api";
import { welcomeTexts, type WelcomeTexts } from "./texts";

const REFRESH_MS = 4000;

interface WelcomeProps {
  api: WelcomeApi;
}

/** Tela de boas-vindas: 3 passos com status (ok/pendente) e o que falta em cada um. */
export default function Welcome({ api }: WelcomeProps) {
  const [lang, setLang] = useState<Lang>("pt-BR");
  const [status, setStatus] = useState<OnboardingStatus | null>(null);
  const [claudeManual, setClaudeManual] = useState(false);
  const [tested, setTested] = useState(false);
  const [busy, setBusy] = useState(false);
  const [device, setDevice] = useState<string | null>(null);
  const t = welcomeTexts(lang);

  const refresh = useCallback(() => {
    api.status().then(setStatus, (e) => console.warn(e));
  }, [api]);

  useEffect(() => {
    document.body.classList.add("welcome-page");
    api.settings().then((s) => setLang(s.lang), () => {});
    refresh();
    const timer = setInterval(refresh, REFRESH_MS);
    return () => clearInterval(timer);
  }, [api, refresh]);

  const act = (action: () => Promise<unknown>) => async () => {
    setBusy(true);
    try {
      await action();
    } catch (e) {
      console.warn(e);
    } finally {
      setBusy(false);
      refresh();
    }
  };

  const claudeOk = status?.claude_connected ?? false;
  const githubOk = status?.github === "on";
  const signIn = act(async () => {
    try {
      const prompt = await api.deviceStart();
      setDevice(`${t.deviceCode.replace("{code}", prompt.user_code)} ${t.deviceWaiting}`);
      const result = await api.deviceWait();
      setDevice(result === "ok" ? null : result === "denied" ? t.deviceDenied : t.deviceExpired);
    } catch (e) {
      setDevice(`${t.deviceError} ${String(e)}`);
    }
  });
  const github = githubStep(status, t, refresh, act(api.enableGithub), signIn, busy);

  return (
    <main className="welcome">
      <header className="welcome-head">
        <Cat mood={claudeOk && githubOk && tested ? "ok" : "info"} pixelSize={4} />
        <div>
          <h1>{t.title}</h1>
          <p>{t.intro}</p>
        </div>
      </header>

      <ol className="steps">
        <Step n={1} title={t.claudeTitle} ok={claudeOk} t={t}>
          <p className={claudeOk ? "" : "warn"}>
            {claudeOk ? t.claudeOk : claudeManual ? t.claudeManual : t.claudePending}
          </p>
          {status?.cursor_present && !status.cursor_connected && <p className="hint">{t.cursorHint}</p>}
          {!claudeOk && (
            <button
              onClick={act(async () => setClaudeManual((await api.connectClaude()) === "manual"))}
              disabled={busy}
            >
              {t.connect}
            </button>
          )}
        </Step>

        <Step n={2} title={t.githubTitle} ok={githubOk} t={t}>
          <p className={githubOk ? "" : "warn"}>{github.text}</p>
          {device && <p className="device">{device}</p>}
          {github.button}
        </Step>

        <Step n={3} title={t.testTitle} ok={tested} t={t}>
          <p>{tested ? t.testOk : t.testPending}</p>
          <button
            onClick={act(async () => {
              await api.test();
              setTested(true);
            })}
            disabled={busy}
          >
            {t.test}
          </button>
        </Step>
      </ol>

      {status && !status.nowplaying && <p className="hint media">{t.mediaHint}</p>}

      <footer className="welcome-foot">
        <span className="hint">{t.reopen}</span>
        <button className="primary" onClick={act(api.finish)}>
          {t.done}
        </button>
      </footer>
    </main>
  );
}

/** Texto e botão do passo do GitHub para cada situação. */
function githubStep(
  status: OnboardingStatus | null,
  t: WelcomeTexts,
  recheck: () => void,
  enable: () => void,
  signIn: () => void,
  busy: boolean,
): { text: string; button: ReactNode } {
  const again = (
    <button className="secondary" onClick={recheck}>
      {t.recheck}
    </button>
  );
  switch (status?.github) {
    case "on":
      return { text: t.githubOn, button: null };
    case "ready":
      return {
        text: t.githubReady,
        button: (
          <button onClick={enable} disabled={busy}>
            {t.enable}
          </button>
        ),
      };
    case "gh_logged_out":
      return { text: t.githubLoggedOut, button: again };
    default:
      if (status?.github_device_flow) {
        // Login por código do GitHub App (sem gh).
        return {
          text: t.githubDevice,
          button: (
            <button onClick={signIn} disabled={busy}>
              {t.signIn}
            </button>
          ),
        };
      }
      return { text: t.githubNoLogin, button: again };
  }
}

function Step(props: { n: number; title: string; ok: boolean; t: WelcomeTexts; children: ReactNode }) {
  return (
    <li className={props.ok ? "step is-ok" : "step"}>
      <div className="step-head">
        <span className="step-n" aria-hidden="true">
          {props.ok ? "✓" : props.n}
        </span>
        <h2>{props.title}</h2>
        <span className={props.ok ? "chip ok" : "chip pending"}>{props.ok ? props.t.ok : props.t.pending}</span>
      </div>
      <div className="step-body">{props.children}</div>
    </li>
  );
}
