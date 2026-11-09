# Miau

Um gatinho em pixel art que aparece no canto da tela quando algo termina: o Claude Code ou o Cursor acabou de responder (ou precisa de você), ou a CI de um PR passou ou falhou. Clicar no cartão abre o PR. Fica só na barra de menus, não rouba o foco de onde você está digitando e funciona offline: tudo acontece no seu computador.

| Estado | Quando |
|---|---|
| ✓ Concluído | o agente terminou, a CI passou |
| ✕ Falhou | a CI falhou, o agente parou com erro |
| ! Atenção | o agente precisa de permissão ou de uma resposta |
| i Aviso | qualquer outro aviso |

## Instalar (macOS 11+, Apple Silicon e Intel)

Baixe o `Miau_<versão>_universal.dmg` da página de Releases do repositório (quando houver) ou gere a partir do código (veja [Desenvolver](#desenvolver)). O `.dmg` universal funciona em Macs com Apple Silicon e com Intel.

1. Abra o `.dmg` e arraste o **Miau.app** para **Aplicativos**.
2. O build não é assinado pela Apple, então o macOS bloqueia a primeira abertura quando o `.dmg` veio de um download. Libere de um destes jeitos:
   - abra o Miau, vá em **Ajustes do Sistema → Privacidade e Segurança**, encontre o aviso sobre o Miau no fim da página e clique em **Abrir Mesmo Assim**; ou
   - no Terminal: `xattr -dr com.apple.quarantine /Applications/Miau.app`
3. Depois disso ele abre como qualquer app. O gatinho aparece na barra de menus (não no Dock).

No menu do gatinho: **Testar notificação** mostra os 4 estados. Marque **Iniciar com o login** para ele abrir sozinho.

## Conectar ao Claude Code, ao Cursor e aos PRs

Pelo menu do gatinho, em **Conectar**:

- **Claude Code**: instala o script de aviso e liga os hooks em `~/.claude/settings.json`. A marca (✓) mostra que está conectado; clique de novo para desconectar. Vale a partir da próxima sessão do Claude Code.
- **Cursor**: o mesmo em `~/.cursor/hooks.json` (só aparece ativo se o Cursor estiver instalado).
- **GitHub (PRs)**: o app acompanha **todos os seus PRs abertos** sozinho e avisa quando a CI de cada um passa ou falha (check runs e status de commit, incluindo Jenkins). Consulta a cada 30 s enquanto algum PR está rodando e a cada 3 min quando tudo terminou; um PR que já tinha terminado quando o app abriu não avisa, e um push novo reinicia o acompanhamento. Login, nesta ordem: **login com código** do GitHub App do Miau (botão Entrar na tela de boas-vindas; o token fica no Keychain e é renovado sozinho a cada 8 h por até 6 meses) ou o [GitHub CLI](https://cli.github.com) já logado (`gh auth login`), que o app encontra sozinho. O login com código só aparece quando o app conhece o **Client ID** do GitHub App: `"github_client_id": "Iv23li…"` em `~/.config/miau/config.json` ou a variável `MIAU_GITHUB_CLIENT_ID`. Com GitHub App, o Miau só enxerga os repositórios onde o app foi instalado (em organizações, um dono aprova uma vez).
- **Instalar comando prwatch (PRs)**: cria o comando `prwatch` em `~/.local/bin`.

O app faz backup do arquivo antes de mexer (`settings.json.miau-backup-<data>`), mantém os outros hooks e configurações como estavam e não duplica nada se você conectar duas vezes. Se o arquivo for gerenciado pelo Nix, somente leitura ou tiver comentários, ele não mexe: copia o trecho para a área de transferência e avisa onde colar. O resultado aparece no próprio cartão do gato.

Quem tem o código também pode instalar pelo terminal, com `scripts/install.sh` (copia os scripts e mostra os trechos de configuração, sem editar nada).

O que cada integração faz:

- **Claude Code** (`~/.claude/settings.json`): hooks `UserPromptSubmit`, `Stop` e `Notification` chamando `notify.sh --from claude`. O cartão mostra o pedido que o Claude terminou de responder e o projeto. Hooks antigos no formato `notify.sh <som>` continuam funcionando e já mostram o gato.
- **Cursor** (`~/.cursor/hooks.json`): `beforeSubmitPrompt` e `stop` chamando `notify.sh --from cursor`.
- **PRs**: `prwatch 1234` (ou a URL do PR, ou nada para o PR da branch atual) espera a CI terminar e avisa com o número, o título e o repositório do PR. Precisa do [GitHub CLI](https://cli.github.com) logado.

Além do aviso, o `notify.sh` pausa a mídia e toca um som:

- **Spotify**: pausado por AppleScript, sem instalar nada.
- **YouTube e outros players** (a "mídia em reprodução" do sistema): pelo [`nowplaying-cli`](https://github.com/kirtan-shah/nowplaying-cli), opcional (`brew install nowplaying-cli`). Se ele faltar e algum agente estiver conectado, o app avisa uma vez.
- Por que não é o próprio app que pausa: desde o macOS 15.4 a Apple bloqueou o framework de "mídia em reprodução" (MediaRemote) para apps que não são dela. Testado no macOS 26.5.2: o mesmo código lê a mídia quando roda num programa da Apple e recebe `nil` num app como o Miau. O `nowplaying-cli` contorna isso carregando uma biblioteca dentro de um programa da Apple; o Miau não copia esse contorno. Ele sempre sai com código 0 e em poucos milissegundos: com o app fechado, só não aparece o gato.

Qualquer outro script pode avisar o Miau:

```bash
curl -s -X POST localhost:4747/notify -H 'Content-Type: application/json' \
  -d '{"source":"custom","status":"ok","title":"Backup","message":"Backup concluído","project":"servidor"}'
```

Campos: `source` (`agent`, `claude`, `ci`, `custom`), `status` (`ok`, `fail`, `attention`, `info`) e, opcionais, `message` (até 140 caracteres), `title` (60), `project` (80), `url` (só `http`/`https`) e `ttl_ms` (2000 a 30000). Contrato completo na seção 5 do [miau-spec.md](miau-spec.md).

## Configuração

Arquivo `~/.config/miau/config.json`, criado no primeiro uso. O menu altera a maioria dos campos:

| Campo | Valores | Padrão |
|---|---|---|
| `lang` | `auto` (idioma do sistema), `pt-BR`, `en` | `auto` |
| `dnd` | Não perturbar | `false` |
| `autostart` | Iniciar com o login | `false` |
| `port` | porta local (a variável `MIAU_PORT` vence) | `4747` |
| `max_cards` | cartões na tela ao mesmo tempo, 1 a 3 | `3` |
| `pixel_size` | tamanho do pixel do gato: 3, 4, 5, 6 ou 8 | `3` |
| `github` | monitor de PRs ligado (menu Conectar → GitHub) | `false` |
| `github_client_id` | Client ID do GitHub App para o login com código | vazio |
| `onboarded` | a tela de boas-vindas já foi vista | `false` |

Valores inválidos voltam ao padrão sem travar o app. Mudanças feitas direto no arquivo valem ao reabrir o app.

## Desenvolver

Requisitos: macOS 11+, [Rust](https://rustup.rs) (estável), Node.js 20+ e as ferramentas de linha de comando do Xcode (`xcode-select --install`).

```bash
npm install
npm run tauri dev        # app em modo de desenvolvimento
scripts/demo.sh          # dispara 4 avisos de exemplo

npm test                 # testes do frontend (Vitest)
npm run typecheck
(cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings)
bash scripts/tests/notify.test.sh
bash scripts/tests/prwatch.test.sh

npm run build:mac        # .app e .dmg para este Mac, em src-tauri/target.noindex/release/bundle/
npm run build:mac:universal  # .dmg universal (Apple Silicon + Intel), em src-tauri/target.noindex/universal-apple-darwin/release/bundle/dmg/
```

O build universal usa o toolchain da Apple num ambiente limpo (`scripts/build-universal.sh`): dentro de um shell do Nix o compilador C padrão só gera arm64 e a parte Intel falharia. No GitHub Actions isso não acontece.

O `build:mac` roda o `tauri build` com dois ajustes para o macOS: põe as ferramentas do Xcode no **fim** do `PATH` (sem isso o `.dmg` pode falhar com `tool 'SetFile' not found` quando o terminal usa um SDK do Nix; no começo do `PATH` elas trocam o compilador e o link falha) e, no fim, tira a cópia de build do registro de apps do sistema (`scripts/after-build.sh`), para não aparecer um segundo "Miau" na lista de apps. A pasta de build se chama `target.noindex` pelo mesmo motivo: o Spotlight não indexa pastas com esse sufixo.

Estrutura: `src/` é o overlay (React + TypeScript), `src-tauri/` é o app (Rust: servidor local, janela, menu, config), `scripts/` são as integrações e `tools/` tem os geradores do gato e do ícone (como regenerar: seção 8 do [miau-spec.md](miau-spec.md)). As especificações ficam em [miau-spec.md](miau-spec.md) (v1), [miau-spec-v2-produto.md](miau-spec-v2-produto.md) e [miau-spec-personalizacao.md](miau-spec-personalizacao.md).

## Releases

O workflow [`.github/workflows/release-macos.yml`](.github/workflows/release-macos.yml) roda só quando uma tag `v*` é enviada:

```bash
git tag v0.1.0 && git push origin v0.1.0
```

Ele roda os testes, gera o `.dmg` universal e cria um release em **rascunho**; o release só fica público quando alguém clicar em **Publish release** no GitHub.

**Assinatura e notarização** (opcional, desligada por padrão). Sem ela, quem baixar o app vê o aviso do macOS na primeira abertura e libera em **Ajustes do Sistema → Privacidade e Segurança → Abrir Mesmo Assim** (ou com `xattr -dr com.apple.quarantine /Applications/Miau.app`), como descrito em [Instalar](#instalar-macos-11-apple-silicon-e-intel). Para assinar e notarizar, é preciso uma conta Apple Developer (cerca de US$ 99/ano) e:

1. Variável do repositório (Settings → Secrets and variables → Actions → Variables): `MACOS_SIGNING` = `true`.
2. Segredos do repositório:
   - `APPLE_CERTIFICATE`: o certificado **Developer ID Application** exportado como `.p12`, em base64 (`base64 -i certificado.p12 | pbcopy`).
   - `APPLE_CERTIFICATE_PASSWORD`: a senha do `.p12`.
   - `APPLE_SIGNING_IDENTITY`: o nome do certificado, ex.: `Developer ID Application: Nome (TEAMID)`.
   - `APPLE_ID`, `APPLE_PASSWORD` (senha de app gerada em appleid.apple.com) e `APPLE_TEAM_ID`: para a notarização.

Nunca coloque certificados, senhas ou tokens no repositório.

## Decisões e limitações

- **Porta fixa `4747`** em `127.0.0.1`: simples para os scripts. Troque pela config ou por `MIAU_PORT` se houver conflito; com a porta ocupada, o menu mostra o erro e o app continua aberto.
- **Segurança local**: o servidor recusa pedidos de páginas web (cabeçalho `Origin`), `Host` que não seja do próprio computador, corpo acima de 4 KB e mais de 10 pedidos por segundo. Todo texto é mostrado como texto, nunca como HTML.
- **Foco**: o cartão aparece sem tirar o foco do app em uso. O mouse é acompanhado pelo próprio app (o WebKit não entrega "hover" a uma janela sem foco), e os cliques atravessam a área transparente da janela.
- **Spaces e tela cheia**: a janela pede para aparecer em todos os Spaces e sobre apps em tela cheia (comportamento nativo do macOS configurado pelo app). Se em alguma versão do macOS ela não aparecer sobre tela cheia, é uma limitação do sistema.
- **`nowplaying-cli`** pode não funcionar em versões recentes do macOS para mídia de navegador; é independente do app e falha em silêncio. O Spotify é pausado por AppleScript.
- **Build não assinado pela Apple** (assinatura ad-hoc, sem custo): por isso o aviso na primeira abertura. Assinar e notarizar exige conta Apple Developer.
- **`macOSPrivateApi`** (necessário para a janela transparente) impede publicar na Mac App Store; a distribuição é direta.
- **"Iniciar com o login" no modo de desenvolvimento** registra o binário de desenvolvimento, que não abre sozinho sem o `npm run tauri dev`. Ative pelo app instalado.
- **Windows e Linux**: ainda não. O código é portável, mas a janela sem foco, o ícone da bandeja e os scripts precisam de versões próprias (planejado na spec v2).

## Privacidade

Nada sai do seu computador. O app só escuta em `127.0.0.1`, não tem telemetria e não registra o texto das mensagens nos logs. O pedido que você enviou ao Claude ou ao Cursor fica guardado em `~/.cache/miau/` (pasta só sua) entre o envio e a resposta, e é apagado em seguida.

## Licença

A definir. O personagem Gato Pixel é original e está sob CC0-1.0.
