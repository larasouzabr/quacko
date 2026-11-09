# Miau: notificador desktop com gatinho (Tauri v2)

> Como usar este documento: coloque na raiz do repositório e peça ao Claude Code:
> "Leia miau-spec.md e implemente um marco por vez, na ordem. Ao fim de cada marco, rode os critérios de aceite, me diga como testar e pare para eu validar. Confira a documentação atual do Tauri v2 antes de usar qualquer chave de configuração ou API marcada como VERIFICAR."

## 1. Objetivo

App pequeno que fica rodando em segundo plano no macOS e mostra um gatinho com um balão ("Seu pedido finalizou!") quando algo termina:

- Um agente de IA parou de trabalhar ou precisa de atenção: Claude Code (extensão do VS Code) ou Cursor. Outros agentes com hooks (Codex CLI, Aider…) entram pelo mesmo caminho, com a origem genérica `agent`.
- A CI de um PR terminou (passou ou falhou).

O app **não observa nada sozinho**. Ele só recebe avisos por HTTP local e os exibe. Quem detecta os eventos são os scripts que já existem (`notify.sh`, `prwatch`, hooks do Claude Code).

## 2. Contexto já existente (não quebrar)

- `~/.claude/hooks/notify.sh`: pausa Spotify (AppleScript) e mídia do sistema (`nowplaying-cli pause`), depois toca um som com `afplay`. Recebe o caminho do som como primeiro argumento.
- Hooks `Stop` e `Notification` em `~/.claude/settings.json` chamam esse script.
- Função `prwatch` no `~/.zshrc` roda `gh pr checks --watch` e chama o script ao final (Glass se passou, Basso se falhou).

O app é uma **camada adicional**: se ele não estiver rodando, tudo continua funcionando como hoje (som e pausa de mídia).

## 3. Escopo

**Dentro (v1)**
- Servidor HTTP em `127.0.0.1` que recebe notificações.
- Janela overlay transparente, sempre no topo, com gatinho animado e balão.
- Fila de até 3 cartões empilhados, com auto-dispensa.
- Ícone na barra de menus: Não perturbe, Testar, Sair.
- Estados do gato: feliz (ok), preocupado (falha), atento (precisa de você), neutro (info).
- Idioma configurável: pt-BR e en.
- Clique no cartão: dispensa; se houver `url`, abre no navegador.
- Iniciar com o login (opcional, ligado por menu).
- Scripts atualizados (`notify.sh`, `prwatch`) que avisam o app.

**Fora (v1)**
- Windows e Linux (mantenha o código portável, mas só valide no macOS).
- Pausar mídia dentro do app (continua no `notify.sh`).
- Som tocado pelo app (continua no `notify.sh`).
- Histórico de notificações, contas, rede externa.

## 4. Arquitetura

```
 Hook Stop/Notification ─┐
 prwatch (gh pr checks) ─┼─> notify.sh ─┬─> osascript / nowplaying-cli (pausa mídia)
 outros scripts ─────────┘              ├─> afplay (som)
                                        └─> curl POST 127.0.0.1:4747/notify
                                                      │
                                          ┌───────────▼────────────┐
                                          │  Miau (Tauri v2)       │
                                          │  Rust: axum + estado   │
                                          │     │ emit("notify")   │
                                          │  Webview: React + TS   │
                                          │  overlay + gato        │
                                          └────────────────────────┘
```

Responsabilidades:

- **Rust (backend)**: servidor HTTP, validação, fila/estado do "Não perturbe", criação e posicionamento da janela overlay, tray, config, autostart. Emite o evento `notify` para o frontend.
- **Frontend (React + TypeScript + Vite)**: renderiza cartões, animações, timers de dispensa, clique. Avisa o Rust quando a fila esvazia (para esconder a janela) via comando Tauri.
- **Scripts de shell**: detecção dos eventos e disparo do `POST`. Falham em silêncio se o app não estiver rodando.

## 5. Contrato da API local

Bind apenas em `127.0.0.1`. Porta padrão `4747` (sobrescrevível por `MIAU_PORT` ou pelo arquivo de config).

### `POST /notify`

Cabeçalho obrigatório: `Content-Type: application/json`.

```json
{
  "source": "claude",
  "status": "ok",
  "message": "Seu pedido finalizou!",
  "title": "Claude Code",
  "project": "lagoa-api",
  "url": "https://github.com/org/repo/pull/1234",
  "ttl_ms": 8000
}
```

| Campo | Tipo | Obrigatório | Regras |
|---|---|---|---|
| `source` | `"agent"` \| `"claude"` \| `"ci"` \| `"custom"` | sim | define o rótulo do cartão quando não há `title`. `agent` = qualquer agente de IA (Cursor, Codex…), com o nome da ferramenta no `title`; `claude` é mantido por compatibilidade (equivale a `agent` com título "Claude Code") |
| `status` | `"ok"` \| `"fail"` \| `"attention"` \| `"info"` | sim | define o estado do gato |
| `message` | string | não | até 140 caracteres; se ausente, usa texto padrão do idioma |
| `title` | string | não | até 60 caracteres |
| `project` | string | não | até 80 caracteres; mostrado em fonte menor no balão |
| `url` | string | não | apenas `http://` ou `https://`; qualquer outro esquema é descartado |
| `ttl_ms` | inteiro | não | entre 2000 e 30000; padrão 8000 (`attention` padrão 15000) |

Respostas: `202` aceito, `400` payload inválido (com mensagem), `413` corpo maior que 4 KB, `415` content-type errado, `403` se houver cabeçalho `Origin`, `429` acima de 10 requisições por segundo.

Se "Não perturbe" estiver ligado: responder `202` com `{"delivered": false}` e não exibir nada.

### `GET /health`

Responde `200` com `{"app":"miau","version":"x.y.z"}`. Usado pelos scripts para testar se o app está de pé.

### Segurança (local, mas importante)

Qualquer página web aberta no navegador pode tentar enviar requisições para `localhost`. Mitigações obrigatórias:

- Rejeitar qualquer requisição com cabeçalho `Origin` (navegadores o enviam em requisições cross-origin; `curl` não).
- Exigir `Content-Type: application/json` (força preflight CORS em navegadores). **Não** enviar cabeçalhos CORS permissivos.
- Limite de tamanho de corpo e rate limit.
- Tratar todo texto como não confiável no frontend: renderizar como texto, nunca como HTML.
- `url` só abre com esquema `http(s)`, e só por clique explícito do usuário.

## 6. Janela overlay

- Uma única janela, criada no start e **escondida**. Mostra quando há cartão ativo, esconde quando a fila esvazia. Assim não é preciso lidar com click-through.
- Propriedades: sem decoração, transparente, sempre no topo, fora da barra de tarefas/Dock, sem sombra, não redimensionável, tamanho fixo aproximado de 380x300.
- Posição: canto superior direito do monitor onde está o cursor, logo abaixo da barra de menus, com margem lateral de 24 px (usar a área útil do monitor, que já desconta barra de menus e Dock).
- Não deve roubar foco do app em uso (o usuário pode estar digitando). Mostrar a janela sem chamar `set_focus`.
- Fundo do webview totalmente transparente (`html, body { background: transparent }`).
- macOS: janela transparente exige `app.macOSPrivateApi: true` no `tauri.conf.json` e a feature `macos-private-api` no `Cargo.toml` do crate `tauri`. **VERIFICAR** na documentação atual do Tauri v2 os nomes exatos.
- Política de ativação do app no macOS: `Accessory` (sem ícone no Dock, só a barra de menus). **VERIFICAR** a API (`set_activation_policy`).
- Aparecer em todos os Spaces e, se possível, sobre apps em tela cheia. **VERIFICAR** o que é viável com a API do Tauri; se exigir código nativo, deixar como melhoria futura e documentar a limitação.

## 7. Interface (frontend)

Cartão:

```
 ┌─────────────────────────────┐
 │ 🐱  Claude Code             │   título + origem
 │  Seu pedido finalizou!      │   mensagem
 │  lagoa-api            │   projeto (pequeno)
 └─────────────────────────────┘
```

- Gato à esquerda do balão. Entrada com bounce curto, balanço leve enquanto visível, saída com fade.
- Estados do gato (todos desenhados, ver seção 8):
  - `ok`: feliz, olhos fechados sorrindo, cauda balançando.
  - `fail`: preocupado, orelhas baixas, gota de suor.
  - `attention`: alerta, olhos grandes, pulinho repetido.
  - `info`: neutro, piscando.
- Pilha de no máximo 3 cartões ancorada no topo, o mais novo em cima (mais perto da borda da tela); cada cartão entra deslizando de cima. Um novo cartão com mesmo `source` + `project` **substitui** o anterior em vez de empilhar.
- Auto-dispensa pelo `ttl_ms`. Passar o mouse pausa o timer.
- Clique: dispensa; se tiver `url`, abre no navegador padrão (plugin opener do Tauri). Botão "x" visível ao passar o mouse.
- Respeitar `prefers-reduced-motion`: sem bounce, só fade.
- Textos padrão (quando `message` ausente), por idioma:
  - pt-BR: ok "Seu pedido finalizou!", fail "Algo deu errado por aqui", attention "Preciso de você!", info "Aviso do Miau"
  - en: ok "Your request finished!", fail "Something went wrong", attention "I need you!", info "Miau notice"
- Quando `message` vier no payload, ela tem prioridade sobre os textos padrão.

## 8. Arte do gato (v1)

Não usar personagens ou imagens de terceiros. O gato padrão é o **"Gato Pixel"**, um personagem original em pixel art (licença CC0-1.0), embutido no app em `src/assets/characters/gato-pixel/` no formato de pacote `.miaupack` da spec v2 (seção 11.1): `manifest.json`, `preview.png` e `assets/{ok,fail,attention,info}.png`.

- Manifest `schema: 2` com `pixel_art: true`, `grid` 28x28, `bubble: "pixel"` e `palette` (papéis recoloríveis), conforme `miau-spec-personalizacao.md`.
- Cada estado é um **sprite sheet em 1x**: 2 quadros lado a lado (56x28; cada quadro 28x28, um pixel da imagem por pixel do desenho), a 3 quadros por segundo. Grade, quadros, colunas e fps vêm do `manifest.json`, não do código.
- O componente React `Cat` (prop `mood`: `ok | fail | attention | info`) desenha o sprite num `<div>` com `background-image`, `image-rendering: pixelated` (sem borrar o pixel art) e animação CSS de `background-position` em `steps(N)`. Com `prefers-reduced-motion`, mostra só o primeiro quadro, parado.
- **Tamanho do pixel**: inteiro (3, 4, 5, 6 ou 8 px por pixel de desenho; padrão 3, ou seja, gato de 84 px). O valor é normalizado pelo `devicePixelRatio` para cada pixel do desenho ocupar um número inteiro de pixels físicos, e o gato é alinhado à grade de pixels físicos. Até existir a janela de Configurações, escolhe-se pelo campo `pixel_size` da config (seção 10), com o app reiniciado. A janela do overlay cresce junto com o gato.
- O `Cat` é isolado do cartão: recebe só o humor (e, opcionalmente, outro personagem). Assim, a troca de personagem por pacotes (spec v2, Marco 13) não mexe no resto. Importação de pacotes, editor e temas **não** fazem parte da v1.

**Regenerar o desenho.** O gerador fica em `tools/make_pixel_cat.py` (precisa do Pillow). Para editar, mexa nas listas de pixels e nas funções `build_*` do script e rode:

```bash
python3 -m venv /tmp/miau-venv && /tmp/miau-venv/bin/pip install pillow   # uma vez
/tmp/miau-venv/bin/python tools/make_pixel_cat.py /tmp/miau-characters
cp -R /tmp/miau-characters/gato-pixel/. src/assets/characters/gato-pixel/
```

O venv evita instalar o Pillow no Python do sistema (que, quando vem do Nix ou do Homebrew, recusa `pip install` global). Com o mesmo desenho, o script gera os mesmos pixels; só a compressão dos PNGs pode mudar entre versões do Pillow. O script também gera `preview-sheet.png`, `preview.gif` e o `gato-pixel.miaupack` na pasta de saída, úteis para conferir o desenho. Esses arquivos não precisam entrar no repositório.

## 9. Tray (barra de menus)

Itens:
1. "Não perturbar" (checkbox; persiste na config)
2. "Testar notificação" (dispara uma de cada estado em sequência)
3. "Iniciar com o login" (checkbox)
4. "Idioma" (submenu: Automático (do sistema), Português, English). "Automático" é o padrão: usa o primeiro idioma suportado (pt ou en) da lista de idiomas preferidos do macOS; se nenhum for suportado, inglês. Ex.: com o Mac em `en-BR` e `pt-BR`, nessa ordem, o Automático dá inglês.
5. "Sair"

O menu e o tooltip seguem o idioma escolhido. "Testar notificação" mostra os 4 estados (2,5 s entre eles) mesmo com "Não perturbar" ligado, porque é um pedido explícito.

Tooltip do ícone mostra a porta em uso. Se a porta estiver ocupada ao iniciar, o tooltip e um item desabilitado no menu indicam o erro; o app continua aberto para o usuário ver o problema.

## 10. Config

Arquivo `~/.config/miau/config.json`, criado com padrões se não existir:

```json
{ "port": 4747, "lang": "auto", "dnd": false, "autostart": false, "max_cards": 3 }
```

`lang`: `"auto"` (segue o sistema), `"pt-BR"` ou `"en"`.

Regras da leitura (Marco 4):
- Cada campo é validado sozinho: um valor inválido volta ao padrão sem descartar os outros, e o log diz quais campos foram ignorados. JSON quebrado não trava o app; o arquivo é regravado na próxima mudança pelo menu.
- `port`: 1 a 65535. A variável `MIAU_PORT` vence o arquivo. Mudar a porta exige reiniciar o app.
- `max_cards`: 1 a 3 (a janela comporta 3 cartões).
- `pixel_size`: 3, 4, 5, 6 ou 8 (padrão 3). Tamanho do pixel do personagem pixel art; ver seção 8.
- `autostart`: o sistema é a fonte da verdade. Se a pessoa tirar o Miau dos itens de login nos Ajustes do Sistema, a config passa a refletir isso no próximo start. Implementado com LaunchAgent (`~/Library/LaunchAgents`), sem pedir permissão de Automação.
- Gravação atômica (arquivo temporário + rename).

Ler no start, gravar quando o tray alterar algo. Valores inválidos caem no padrão (não travar o app).

## 11. Estrutura do repositório

```
miau/
├─ miau-spec.md
├─ package.json
├─ vite.config.ts
├─ index.html
├─ src/                       # frontend
│  ├─ main.tsx
│  ├─ App.tsx                 # escuta "notify", gerencia fila
│  ├─ state/queue.ts          # reducer puro (testável)
│  ├─ state/queue.test.ts
│  ├─ components/Card.tsx
│  ├─ components/Cat.tsx      # sprite sheet + mood
│  ├─ character/              # tipos e leitura do manifest do personagem
│  ├─ assets/characters/gato-pixel/   # personagem padrão (.miaupack descompactado)
│  ├─ i18n.ts
│  └─ styles.css
├─ src-tauri/
│  ├─ Cargo.toml
│  ├─ tauri.conf.json
│  ├─ capabilities/default.json
│  └─ src/
│     ├─ main.rs
│     ├─ lib.rs               # setup do app
│     ├─ server.rs            # axum, rotas, validação
│     ├─ model.rs             # Notification, enums, validação
│     ├─ window.rs            # criar/posicionar/mostrar/esconder overlay
│     ├─ tray.rs
│     └─ config.rs
├─ tools/
│  └─ make_pixel_cat.py       # gerador dos sprites do gato (seção 8)
└─ scripts/
   ├─ notify.sh               # versão nova (seção 13)
   ├─ prwatch.zsh             # função nova (seção 13)
   ├─ demo.sh                 # dispara os 4 estados (teste visual)
   └─ install.sh              # copia scripts para ~/.claude/hooks e informa o que colar no ~/.zshrc
```

## 12. Dependências sugeridas

- Rust: `tauri` (v2), `tauri-plugin-opener`, `tauri-plugin-autostart`, `axum`, `tokio`, `serde`, `serde_json`, `dirs`, `tower` (limite de taxa/corpo), `tracing`.
- Frontend: `react`, `react-dom`, `@tauri-apps/api`, `vite`, `typescript`, `vitest`.
- Conferir versões atuais e permissões (capabilities) exigidas pelos plugins na documentação do Tauri v2.

## 13. Scripts (integração)

Implementados no Marco 5, com os formatos de hooks conferidos na documentação oficial em 2026-11-09 ([Claude Code](https://code.claude.com/docs/en/hooks), [Cursor](https://cursor.com/docs/agent/hooks)). Testes: `bash scripts/tests/notify.test.sh` e `bash scripts/tests/prwatch.test.sh`.

### `scripts/notify.sh` (substitui o atual, mantém o comportamento)

Uso: `notify.sh [--from claude|cursor] [--sound CAMINHO] [--source agent|claude|ci|custom] [--status ok|fail|attention|info] [--message TEXTO] [--title TEXTO] [--project TEXTO] [--url URL]`

`--from` escolhe o **adaptador** que lê o JSON que a ferramenta manda no stdin e o traduz para os mesmos campos do aviso (`source` = `agent`, `title` = nome da ferramenta, `project`, `message`). Sem `--from`, o formato é detectado pelo `hook_event_name` (`Stop`, `Notification`, `UserPromptSubmit` → Claude; `stop`, `beforeSubmitPrompt` → Cursor). Argumentos explícitos sempre vencem o stdin. Adicionar outra ferramenta = mais um adaptador, sem mexer no app.

Comportamento:
1. Pausa Spotify (só se estiver aberto) e mídia do sistema (`nowplaying-cli pause`), como antes, **em segundo plano** (o `osascript` leva quase 1 s).
2. Toca o som com `afplay` em segundo plano. Sem `--sound`: Glass (ok), Basso (fail), Ping (attention/info).
3. Lê o stdin só se ele for um pipe ou arquivo (hooks), com `jq`.
4. Envia o aviso com `curl -s --max-time 1` em segundo plano, ignorando erros. JSON montado com `jq -n --arg` (sem `jq`, só os campos de lista fechada).
5. **Sempre sai com 0** e não escreve nada no stdout (o stdout do `UserPromptSubmit` entraria no contexto do Claude), exceto as respostas que o Cursor espera. Medido: 0,24 s num hook real; 0,014 s com o app fechado.
6. Retrocompatível: chamado só com um caminho de som (uso antigo), trata como `--sound`; se vier o JSON de um hook, já mostra o gato.
7. Log em `/tmp/miau-notify.log`, sem o texto das mensagens.

Texto do cartão, neutro de idioma (o selo "Concluído/Falhou" já vem traduzido pelo app):
- **Claude `Stop`**: título "Claude Code", mensagem = o pedido que o Claude respondeu, entre aspas (“adicione testes para o parser…”, até 100 caracteres), projeto = pasta do `cwd`. O `Stop` não traz o pedido (só `last_assistant_message`), então o hook `UserPromptSubmit` guarda o `prompt` em `~/.cache/miau/prompt-claude-<session_id>` (pasta 700), e o `Stop` lê e apaga. Sem o pedido, cai no texto padrão. `StopFailure` → fail.
- **Claude `Notification`**: mensagem = `message` do Claude Code. `notification_type` `agent_completed` → ok; `auth_success`, `elicitation_complete`, `elicitation_response`, `quota_*`, `plugin_notification` → info; o resto (permissão, ociosidade, perguntas) → attention.
- **Cursor `stop`**: título "Cursor", projeto = pasta do primeiro `workspace_roots`, mensagem = pedido guardado no `beforeSubmitPrompt`. `status` `completed` → ok, `error` → fail, `aborted` → não avisa. Responde `{}` (sem mensagem de continuação).
- **Cursor `beforeSubmitPrompt`**: só guarda o pedido e responde `{"continue":true}` (a documentação não diz o que acontece sem resposta; responder explicitamente garante que o envio nunca é bloqueado).

### Hooks do Claude Code (`~/.claude/settings.json`)

```json
"hooks": {
  "UserPromptSubmit": [
    { "hooks": [{ "type": "command", "command": "/Users/<você>/.claude/hooks/notify.sh --from claude" }] }
  ],
  "Stop": [
    { "hooks": [{ "type": "command", "command": "/Users/<você>/.claude/hooks/notify.sh --from claude --sound /System/Library/Sounds/Glass.aiff" }] }
  ],
  "Notification": [
    { "hooks": [{ "type": "command", "command": "/Users/<você>/.claude/hooks/notify.sh --from claude --sound /System/Library/Sounds/Ping.aiff" }] }
  ]
}
```

Usar caminho absoluto (a documentação recomenda; o `install.sh` imprime o bloco com o caminho certo).

### Hooks do Cursor (`~/.cursor/hooks.json`)

```json
{
  "version": 1,
  "hooks": {
    "beforeSubmitPrompt": [{ "command": "/Users/<você>/.claude/hooks/notify.sh --from cursor" }],
    "stop": [{ "command": "/Users/<você>/.claude/hooks/notify.sh --from cursor --sound /System/Library/Sounds/Glass.aiff" }]
  }
}
```

O `notify.sh` continua em `~/.claude/hooks/` para não quebrar os hooks atuais do Claude; quem só usa Cursor também usa esse caminho na v1 (a v2 troca tudo pelo CLI `miau`).

### `scripts/prwatch` (script executável)

Era uma função para o `~/.zshrc`; virou script porque o `~/.zshrc` pode ser gerenciado (Nix/home-manager) e não aceita edição manual.

- Aceita os mesmos argumentos de `gh pr checks` (número, URL, branch, `--repo`).
- Roda `gh pr checks "$@" --watch --interval 15` e devolve o mesmo código de saída. Interrompido (Ctrl+C), não avisa.
- Obtém `number`, `title` e `url` com `gh pr view` (só o PR e o `--repo` são repassados; falha é ignorada).
- Chama `notify.sh` com `--source ci`, `--status ok|fail`, `--title "PR #N"`, `--message` (título do PR: **qual PR** terminou), `--project` (`dono/repo`, tirado da URL) e `--url` (clicar abre o PR). Sem dados do PR, avisa sem número.
- Som: Glass se passou, Basso se falhou.

### `scripts/install.sh`

Copia `notify.sh` e `prwatch` para `~/.claude/hooks/` com permissão de execução e **faz backup** do que existia (`.bak`; os seguintes ganham data, sem sobrescrever o original). Imprime os blocos de hooks do Claude e do Cursor e como pôr o `prwatch` no PATH (alias no home-manager quando o `~/.zshrc` vem do Nix, ou link em `~/.local/bin`). Não edita `~/.zshrc`, `settings.json` nem `hooks.json`.

## 14. Marcos e critérios de aceite

**Marco 1: esqueleto**
- Projeto Tauri v2 + React + TS cria e abre com `npm run tauri dev`.
- Janela overlay existe escondida; ícone aparece na barra de menus; sem ícone no Dock.
- Aceite: app abre, tray com "Sair" funcional.

**Marco 2: servidor local**
- `server.rs` com `/health` e `/notify`, validação completa da seção 5 e testes unitários do `model.rs` (payload válido, campos longos, `url` com esquema inválido, enums inválidos).
- Aceite:
  ```bash
  curl -s localhost:4747/health
  curl -s -X POST localhost:4747/notify -H 'Content-Type: application/json' -d '{"source":"claude","status":"ok"}'   # 202
  curl -s -X POST localhost:4747/notify -d '{"source":"claude","status":"ok"}'                                       # 415
  curl -s -X POST localhost:4747/notify -H 'Content-Type: application/json' -H 'Origin: http://evil.test' -d '{"source":"claude","status":"ok"}'  # 403
  curl -s -X POST localhost:4747/notify -H 'Content-Type: application/json' -d '{"source":"x","status":"ok"}'        # 400
  ```

**Marco 3: overlay e cartão**
- Evento `notify` chega ao frontend; janela aparece no canto, mostra cartão com gato (mood correto), some após `ttl_ms` e a janela é escondida.
- Reducer da fila (`queue.ts`) com testes Vitest: limite de 3, substituição por `source`+`project`, remoção por expiração.
- Aceite: os 4 estados visualmente distintos; janela não rouba foco do editor; clicar dispensa; passar o mouse pausa o timer.

**Marco 4: tray e config**
- Não perturbe, Testar, Idioma, Iniciar com o login; config persistida em `~/.config/miau/config.json`.
- Aceite: com "Não perturbe" ligado, o `curl` retorna `{"delivered":false}` e nada aparece; idioma troca os textos padrão.

**Marco 5: integração**
- `notify.sh` novo, `prwatch` novo, `install.sh`.
- Aceite:
  ```bash
  ~/.claude/hooks/notify.sh --source ci --status fail --message "PR #1: checks falharam" --project teste
  echo '{"cwd":"/tmp/meu-projeto"}' | ~/.claude/hooks/notify.sh --source claude --status ok
  ```
  Com o app fechado, o script termina rápido, sem erro, e o som/pausa de mídia continuam funcionando.
  ```bash
  echo '{"hook_event_name":"stop","status":"completed","workspace_roots":["/tmp/meu-projeto"]}' | ~/.claude/hooks/notify.sh --from cursor
  ```
  (formato do Cursor conferido na documentação; ver seção 13.)
- Teste real: pedir algo curto ao Claude no VS Code e ao agente do Cursor e ver o gato ao final de cada um; rodar `prwatch` num PR.

**Marco 6: acabamento**
- Autostart funcionando, README curto (como rodar, empacotar com `npm run tauri build`, como instalar os scripts), ícone do app.

## 15. Decisões e riscos a registrar no README

- Porta fixa `4747`: simples para os scripts; configurável caso haja conflito.
- Sobre apps em tela cheia e Spaces: pode haver limitação no Tauri; documentar o comportamento observado.
- `nowplaying-cli` pode não funcionar em versões recentes do macOS; é independente do app e falha em silêncio.
- Build para distribuição fora da App Store: `macOSPrivateApi` impede publicação na Mac App Store (irrelevante para uso pessoal).
- Futuro: mover pausa de mídia e som para dentro do app, mais gatos/skins, Lottie, suporte a Windows/Linux.
