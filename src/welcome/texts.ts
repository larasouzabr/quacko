import type { Lang } from "../i18n";

const PT = {
  title: "Bem-vindo ao Quacko",
  intro: "O patinho avisa quando o Claude, o Cursor ou a CI dos seus PRs terminam. Três passos e pronto:",
  ok: "OK",
  pending: "Pendente",
  claudeTitle: "Conectar o Claude Code",
  claudeOk: "Conectado. Vale a partir da próxima sessão do Claude Code.",
  claudePending: "Hook não conectado: o pato não aparece quando o Claude termina.",
  claudeManual: "Não deu para editar o ~/.claude/settings.json. Copiei o trecho: cole no arquivo.",
  cursorHint: "Cursor encontrado: conecte também pelo menu do pato → Conectar → Cursor.",
  connect: "Conectar",
  githubTitle: "Conectar o GitHub",
  githubOn: "Acompanhando seus PRs abertos: o pato avisa quando a CI termina.",
  githubReady: "Login do GitHub encontrado. Ative para acompanhar seus PRs.",
  githubLoggedOut: "GitHub CLI deslogado: rode “gh auth login” no terminal e clique em Verificar de novo.",
  githubNoLogin: "Instale o GitHub CLI (cli.github.com) e rode “gh auth login” no terminal.",
  githubDevice: "Entre com um código do GitHub, sem precisar do terminal.",
  enable: "Ativar",
  recheck: "Verificar de novo",
  signIn: "Entrar",
  deviceCode: "Seu código é {code} (já copiado). Abri o GitHub no navegador: cole o código e autorize o Quacko.",
  deviceWaiting: "Esperando você autorizar no GitHub…",
  deviceDenied: "Login cancelado no GitHub. Clique em Entrar para tentar de novo.",
  deviceExpired: "O código expirou. Clique em Entrar para pedir outro.",
  deviceError: "Não deu para entrar no GitHub:",
  testTitle: "Testar notificação",
  testPending: "Veja os 4 estados do pato no canto da tela.",
  testOk: "Apareceu? Se não, confira se o “Não perturbar” está desligado no menu.",
  test: "Testar",
  mediaHint:
    "Opcional: para pausar YouTube e outros players, instale o nowplaying-cli (brew install nowplaying-cli). O Spotify já pausa sozinho.",
  done: "Concluir",
  reopen: "Esta tela fica no menu do pato → Preferências → Boas-vindas…",
};

export type WelcomeTexts = typeof PT;

const EN: WelcomeTexts = {
  title: "Welcome to Quacko",
  intro: "The duck lets you know when Claude, Cursor or your PRs' CI finish. Three steps and you're set:",
  ok: "OK",
  pending: "Pending",
  claudeTitle: "Connect Claude Code",
  claudeOk: "Connected. Takes effect in new Claude Code sessions.",
  claudePending: "Hook not connected: the duck won't show up when Claude finishes.",
  claudeManual: "Couldn't edit ~/.claude/settings.json. The snippet was copied: paste it into the file.",
  cursorHint: "Cursor found: connect it too from the duck menu → Connect → Cursor.",
  connect: "Connect",
  githubTitle: "Connect GitHub",
  githubOn: "Watching your open PRs: the duck lets you know when CI finishes.",
  githubReady: "GitHub login found. Turn it on to watch your PRs.",
  githubLoggedOut: "GitHub CLI is logged out: run “gh auth login” in a terminal, then click Check again.",
  githubNoLogin: "Install the GitHub CLI (cli.github.com) and run “gh auth login” in a terminal.",
  githubDevice: "Sign in with a GitHub code, no terminal needed.",
  enable: "Turn on",
  recheck: "Check again",
  signIn: "Sign in",
  deviceCode: "Your code is {code} (already copied). GitHub is open in your browser: paste the code and authorize Quacko.",
  deviceWaiting: "Waiting for you to authorize on GitHub…",
  deviceDenied: "Sign-in was canceled on GitHub. Click Sign in to try again.",
  deviceExpired: "The code expired. Click Sign in to get a new one.",
  deviceError: "Couldn't sign in to GitHub:",
  testTitle: "Test notification",
  testPending: "See the duck's 4 states in the corner of the screen.",
  testOk: "Did it show up? If not, make sure “Do Not Disturb” is off in the menu.",
  test: "Test",
  mediaHint:
    "Optional: to pause YouTube and other players, install nowplaying-cli (brew install nowplaying-cli). Spotify already pauses.",
  done: "Done",
  reopen: "This screen stays in the duck menu → Preferences → Welcome…",
};

export function welcomeTexts(lang: Lang): WelcomeTexts {
  return lang === "en" ? EN : PT;
}
