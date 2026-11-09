import type { Lang } from "../i18n";

const PT = {
  title: "Bem-vindo ao Miau",
  intro: "O gatinho avisa quando o Claude, o Cursor ou a CI dos seus PRs terminam. Três passos e pronto:",
  ok: "OK",
  pending: "Pendente",
  claudeTitle: "Conectar o Claude Code",
  claudeOk: "Conectado. Vale a partir da próxima sessão do Claude Code.",
  claudePending: "Hook não conectado: o gato não aparece quando o Claude termina.",
  claudeManual: "Não deu para editar o ~/.claude/settings.json. Copiei o trecho: cole no arquivo.",
  cursorHint: "Cursor encontrado: conecte também pelo menu do gato → Conectar → Cursor.",
  connect: "Conectar",
  githubTitle: "Conectar o GitHub",
  githubOn: "Acompanhando seus PRs abertos: o gato avisa quando a CI termina.",
  githubReady: "Login do GitHub encontrado. Ative para acompanhar seus PRs.",
  githubLoggedOut: "GitHub CLI deslogado: rode “gh auth login” no terminal e clique em Verificar de novo.",
  githubNoLogin: "Instale o GitHub CLI (gh) e rode “gh auth login”. O login com código do GitHub chega em breve.",
  githubDevice: "Entre com um código do GitHub, sem precisar do terminal.",
  enable: "Ativar",
  recheck: "Verificar de novo",
  signIn: "Entrar",
  testTitle: "Testar notificação",
  testPending: "Veja os 4 estados do gato no canto da tela.",
  testOk: "Apareceu? Se não, confira se o “Não perturbar” está desligado no menu.",
  test: "Testar",
  mediaHint:
    "Opcional: para pausar YouTube e outros players, instale o nowplaying-cli (brew install nowplaying-cli). O Spotify já pausa sozinho.",
  done: "Concluir",
  reopen: "Esta tela fica no menu do gato → Boas-vindas…",
};

export type WelcomeTexts = typeof PT;

const EN: WelcomeTexts = {
  title: "Welcome to Miau",
  intro: "The cat lets you know when Claude, Cursor or your PRs' CI finish. Three steps and you're set:",
  ok: "OK",
  pending: "Pending",
  claudeTitle: "Connect Claude Code",
  claudeOk: "Connected. Takes effect in new Claude Code sessions.",
  claudePending: "Hook not connected: the cat won't show up when Claude finishes.",
  claudeManual: "Couldn't edit ~/.claude/settings.json. The snippet was copied: paste it into the file.",
  cursorHint: "Cursor found: connect it too from the cat menu → Connect → Cursor.",
  connect: "Connect",
  githubTitle: "Connect GitHub",
  githubOn: "Watching your open PRs: the cat lets you know when CI finishes.",
  githubReady: "GitHub login found. Turn it on to watch your PRs.",
  githubLoggedOut: "GitHub CLI is logged out: run “gh auth login” in a terminal, then click Check again.",
  githubNoLogin: "Install the GitHub CLI (gh) and run “gh auth login”. Signing in with a GitHub code is coming soon.",
  githubDevice: "Sign in with a GitHub code, no terminal needed.",
  enable: "Turn on",
  recheck: "Check again",
  signIn: "Sign in",
  testTitle: "Test notification",
  testPending: "See the cat's 4 states in the corner of the screen.",
  testOk: "Did it show up? If not, make sure “Do Not Disturb” is off in the menu.",
  test: "Test",
  mediaHint:
    "Optional: to pause YouTube and other players, install nowplaying-cli (brew install nowplaying-cli). Spotify already pauses.",
  done: "Done",
  reopen: "This screen stays in the cat menu → Welcome…",
};

export function welcomeTexts(lang: Lang): WelcomeTexts {
  return lang === "en" ? EN : PT;
}
