# Miau v2: do script pessoal ao app que qualquer pessoa instala

> Continuação do `miau-spec.md` (v1, MVP pessoal). Termine a v1 primeiro. Este documento descreve o que muda para o Miau virar um produto distribuível: sem scripts manuais, sem `jq`/`curl`/`gh` obrigatórios, com instalação por clique.
>
> Como usar: coloque na raiz do repositório e peça ao Claude Code: "Leia miau-spec.md e miau-spec-v2-produto.md. Implemente os marcos 7 em diante, um por vez, rodando os critérios de aceite e parando para eu validar. Itens marcados VERIFICAR devem ser conferidos na documentação atual antes de usar."

## 1. O que muda da v1 para a v2

| Tema | v1 (pessoal) | v2 (produto) |
|---|---|---|
| Pausar mídia e tocar som | `notify.sh` (osascript, nowplaying-cli, afplay) | dentro do app, com implementação por sistema operacional |
| Detectar fim da CI | `prwatch` (gh CLI no terminal) | o app consulta o GitHub sozinho |
| Detectar fim do Claude Code | hooks editados à mão no `settings.json` | botão "Conectar Claude Code" que instala os hooks com segurança |
| Qualquer outro comando longo | não existe | `miau run -- <comando>` e integração com o shell |
| Comunicação | HTTP em `127.0.0.1:4747` | socket local (padrão) + HTTP opcional com token |
| Configuração | editar JSON | janela de Configurações e onboarding |
| Aparência | um gato fixo | personagem e tema trocáveis, com pacotes compartilháveis e editor embutido |
| Instalação | cópia manual de scripts | `.dmg` assinado, atualização automática, Homebrew |

Princípio: **o usuário só instala o app e clica em "Conectar".** Nada de colar bloco no `.zshrc`.

## 2. Princípios de design

1. Local primeiro: nenhum dado sai da máquina, exceto chamadas explícitas ao GitHub que o usuário ligou.
2. Falha silenciosa nos integradores: o CLI e os hooks **nunca** podem quebrar ou atrasar a ferramenta do usuário.
3. Reversível: tudo que o app instala (hooks, link do CLI, snippet de shell) tem desinstalação limpa e backup.
4. Extensível por fontes e destinos, não por `if`s espalhados.
5. Sem telemetria por padrão.

## 3. Arquitetura geral

```
 FONTES (quem detecta)                 NÚCLEO (Rust)                      DESTINOS (como avisar)
 ┌───────────────────────┐     ┌───────────────────────────┐     ┌───────────────────────────┐
 │ Claude Code (hooks)   │──┐  │  Event bus (tokio mpsc)   │  ┌─>│ Overlay (gato + balão)    │
 │ GitHub PRs (polling)  │──┼─>│  Regras por fonte:        │──┼─>│ Som (rodio)               │
 │ miau run -- <cmd>     │──┤  │   habilitada? dnd? ttl?   │  ├─>│ Pausar mídia (+ retomar)  │
 │ Shell (precmd/preexec)│──┤  │  Deduplicação + fila      │  └─>│ Notificação nativa do SO  │
 │ HTTP/webhook (opcional)│─┘  └─────────────┬─────────────┘     └───────────────────────────┘
 └───────────────────────┘                   │
        ▲ IPC local (socket)         Config / Credenciais / Pacotes / Atualizador
        │
   miau-cli (sidecar)  <── chamado pelos hooks, pelo shell e pelo usuário
```

Fluxo típico (Claude termina): hook do Claude Code executa `miau hook claude stop` e entrega o JSON no stdin. O CLI lê o stdin, envia um evento ao app pelo socket em poucos milissegundos e sai com código 0. O núcleo aplica as regras e aciona os destinos.

## 4. Organização do código

Cargo workspace:

```
miau/
├─ Cargo.toml                    # [workspace]
├─ crates/
│  ├─ miau-core/                 # modelo de evento, regras, fila, config, traits (sem Tauri)
│  ├─ miau-sources/              # claude_hooks, github, shell, http
│  ├─ miau-sinks/                # overlay_bridge, sound, media (por SO), os_notify
│  ├─ miau-ipc/                  # cliente e servidor do socket local
│  └─ miau-cli/                  # binário pequeno, inicia rápido, sem dependências de GUI
├─ src-tauri/                    # app Tauri v2 (orquestra tudo, janelas, tray, updater)
├─ src/                          # frontend React + TS
│  ├─ overlay/                   # janela do gato
│  ├─ settings/                  # janela de configurações
│  └─ onboarding/                # assistente do primeiro uso
├─ packs/default/                # personagem e tema padrão (manifest + assets)
└─ .github/workflows/            # CI e release
```

Decisão: o CLI é um **crate separado e pequeno**, empacotado como sidecar (`externalBin`) do Tauri. Motivo: partida rápida (hooks não podem esperar o Tauri inicializar). **VERIFICAR** a configuração atual de sidecars no Tauri v2.

## 5. Modelo de evento (contrato interno)

Mantém compatibilidade com o payload da v1 (`status`: `ok|fail|attention|info`).

```json
{
  "id": "uuid-v4",
  "ts": "2026-11-09T14:20:00-03:00",
  "source": "claude | github | cli | shell | http | custom",
  "status": "ok | fail | attention | info",
  "title": "string <= 60",
  "message": "string <= 140",
  "project": "string <= 80",
  "url": "http(s) apenas",
  "ttl_ms": 8000,
  "meta": { "duration_ms": 123000, "exit_code": 0, "pr": 1234 }
}
```

Regras por fonte (config):

```json
{
  "sources": {
    "claude": { "enabled": true,  "overlay": true, "sound": "glass", "pause_media": true,  "min_duration_s": 0 },
    "github": { "enabled": true,  "overlay": true, "sound": "ding",  "pause_media": true },
    "cli":    { "enabled": true,  "overlay": true, "sound": "glass", "pause_media": false },
    "shell":  { "enabled": false, "min_duration_s": 30, "ignore": ["vim","nvim","less","ssh","man","top"] }
  },
  "dnd": { "manual": false, "schedule": { "enabled": false, "from": "22:00", "to": "08:00" } }
}
```

Deduplicação: mesmo `source` + `project` dentro de 3 s substitui em vez de repetir (ex.: hooks `Stop` seguidos).

## 6. Fontes

### 6.1 Claude Code (hooks)

Comandos que o app instala nos hooks:

- `Stop`: `miau hook claude stop`
- `Notification`: `miau hook claude notification`

O CLI lê o JSON do stdin (campos como `cwd`, `session_id`, e, na notificação, `message`; **VERIFICAR** na documentação atual de hooks os campos exatos), deriva `project` do nome da pasta de `cwd` e envia o evento.

**Regra crítica:** o CLI deve **sempre sair com código 0** e em menos de ~300 ms, mesmo se o app estiver fechado. Em hooks do Claude Code, certos códigos de saída têm efeito de bloqueio (**VERIFICAR**), então um erro nosso não pode alterar o comportamento do Claude.

**Instalador de hooks** (botão "Conectar Claude Code" nas Configurações e no onboarding):

1. Localizar `~/.claude/settings.json` (Windows: `%USERPROFILE%\.claude\settings.json`). Se não existir, criar `{}`.
2. Resolver links simbólicos. Se o destino estiver em `/nix/store` ou não for gravável: **modo manual**. Mostrar o trecho JSON pronto para copiar e explicar o motivo. Não tentar contornar.
3. Fazer parse com `serde_json`. Se o arquivo tiver comentários ou for inválido: modo manual, sem alterar nada.
4. Criar backup `settings.json.miau-backup-<timestamp>`.
5. Mesclar de forma **idempotente**: adicionar entradas em `hooks.Stop` e `hooks.Notification` sem remover as existentes. Identificar as entradas do Miau pelo comando que começa com o caminho do `miau` (ou contém `miau hook`).
6. Gravar de forma atômica (arquivo temporário + rename).
7. Mostrar resultado: "Instalado. Abra uma nova sessão do Claude Code".
8. Botão "Desconectar" remove somente as entradas do Miau e preserva o resto.

Testes obrigatórios do instalador: arquivo vazio, arquivo com outros hooks, arquivo já com hooks do Miau (idempotência), arquivo inválido, link simbólico somente leitura, desinstalação preservando hooks de terceiros.

**Outros agentes (Cursor e afins).** A fonte é genérica (`agent`, com o nome da ferramenta no título), e cada ferramenta tem um adaptador no CLI: `miau hook cursor stop`, `miau hook claude stop`… O botão "Conectar Cursor" segue os mesmos 8 passos acima, aplicados ao `~/.cursor/hooks.json`, com os mesmos testes. A v1 já usa esse desenho no `notify.sh --from <ferramenta>`.

### 6.2 GitHub (CI de PRs) sem `gh` obrigatório

Autenticação, em ordem de preferência:

1. Se `gh` estiver instalado e autenticado, usar `gh auth token` (zero configuração para desenvolvedores).
2. Caso contrário, **OAuth device flow** (usuário vê um código, abre a página do GitHub e autoriza). O `client_id` de OAuth App é público. Escopos mínimos que permitam ler status de PRs privados (**VERIFICAR**; avaliar GitHub App ou fine-grained token como alternativa mais restrita).
3. Armazenar o token no chaveiro do sistema (crate `keyring`): Keychain no macOS, Credential Manager no Windows, Secret Service no Linux. Nunca em arquivo de config.
4. Suportar GitHub Enterprise: campo "host" nas configurações.

Descoberta de PRs: "monitorar meus PRs abertos" (padrão) ou lista manual de repositórios/PRs.

Consulta: GraphQL, uma requisição para todos os PRs abertos do usuário, lendo `statusCheckRollup` do último commit de cada um. Esse rollup cobre check runs e status contexts, o que inclui o Jenkins quando ele reporta como status do commit. Estados possíveis do rollup: `EXPECTED`, `PENDING`, `SUCCESS`, `FAILURE`, `ERROR`. **VERIFICAR** nomes de campos e limites de taxa na documentação atual.

Algoritmo de transição:

- Estado por PR: `{ headSha, rollupState }`.
- Disparar evento quando, para o **mesmo `headSha`**, o estado passa de `PENDING`/`EXPECTED` para `SUCCESS` (status `ok`) ou `FAILURE`/`ERROR` (status `fail`).
- Novo push (`headSha` mudou) reinicia o acompanhamento sem disparar.
- Primeira leitura de um PR já finalizado **não** dispara (evita enxurrada ao abrir o app).
- Mensagem: "PR #N: checks passaram" / "PR #N: checks falharam", `url` do PR, `project` = repositório.

Frequência: a cada 20 a 30 s enquanto houver algum PR pendente; a cada 3 a 5 min quando tudo estiver finalizado. Usar cabeçalho condicional (ETag) quando possível e respeitar limites de taxa com recuo exponencial. Pausar polling quando o usuário desligar a fonte ou estiver sem rede.

Webhooks ficam fora (exigem servidor público). Polling resolve para uso individual.

### 6.3 `miau run`: qualquer comando longo

```bash
miau run -- npm test
miau run --title "Build" -- ./gradlew build
```

Executa o comando com stdio herdado, preserva o código de saída e, ao terminar, envia evento (`ok` se 0, senão `fail`) com duração em `meta`. Útil para qualquer pessoa, não só quem usa Claude ou GitHub.

### 6.4 Integração com o shell (opcional)

`miau shell-init zsh|bash|fish|powershell` imprime um snippet para o usuário colocar no arquivo de init do shell (o app **não edita** esses arquivos por conta própria; em máquinas com `.zshrc` gerenciado por Nix ou similar isso falharia). O snippet usa `preexec`/`precmd` (zsh), mede a duração e chama `miau notify` quando passar de `min_duration_s` (padrão 30 s), ignorando a lista `ignore`. O onboarding exibe o snippet com botão "Copiar".

### 6.5 HTTP/webhook (opcional, desligado por padrão)

Mesmo contrato da v1, mas só com **token obrigatório** (`Authorization: Bearer ...`), gerado na instalação e visível nas Configurações. Mantém as proteções da v1 (sem `Origin`, `Content-Type: application/json`, limite de corpo e de taxa).

## 7. Destinos (sinks)

### 7.1 Overlay
Como na v1, mais: "retomar música" como botão do cartão quando o Miau tiver pausado algo.

### 7.2 Som
Em Rust (crate `rodio`), com sons embutidos (Glass-like, ding, miau), volume e opção de arquivo próprio. Substitui o `afplay`.

### 7.3 Pausar mídia (por sistema operacional)

```rust
pub struct PauseToken(Vec<PausedPlayer>);   // lembra o que FOI pausado por nós

pub trait MediaController: Send + Sync {
    fn pause_if_playing(&self) -> PauseToken;   // só pausa o que estava tocando
    fn resume(&self, token: PauseToken);        // só retoma o que nós pausamos
}
```

Regra de ouro: **nunca usar play/pause alternado às cegas**. Consultar o estado, pausar apenas o que estiver tocando e retomar apenas o que o Miau pausou.

- **macOS**:
  - Spotify e Music: AppleScript (consultar `player state`, pausar, retomar). Exige permissão de Automação; o app deve explicar antes de pedir.
  - Mídia de navegador e outros: a API de "Now Playing" foi restringida em versões recentes; ferramentas como `nowplaying-cli` funcionaram na sua máquina, mas a viabilidade para distribuição ampla é incerta. **VERIFICAR** projetos que contornam a restrição (ex.: adaptadores para MediaRemote) e seus riscos de manutenção. Plano B: pausar via JavaScript no Chrome/Safari (exige que o usuário habilite a opção de JavaScript por Apple Events) ou simplesmente oferecer só Spotify/Music e documentar a limitação.
- **Windows**: System Media Transport Controls (SMTC) via crate `windows`, pausando sessões em reprodução e retomando as mesmas.
- **Linux**: MPRIS por D-Bus (crate `zbus`), pausando players com `PlaybackStatus = Playing`.

### 7.4 Notificação nativa do SO
Fallback quando o overlay estiver desligado ou o sistema não suportar janela transparente (ex.: alguns ambientes Wayland). Usar plugin de notificação do Tauri (**VERIFICAR**).

## 8. IPC e segurança

- **Canal principal: socket local**, não porta TCP. Unix domain socket em `~/Library/Application Support/miau/miau.sock` (macOS/Linux, permissão `0600`) e named pipe no Windows (crate `interprocess` ou equivalente). Vantagens: nenhuma página web consegue alcançar (sem CSRF), sem conflito de portas.
- Protocolo: JSON por linha (uma mensagem = um evento), com campo `v` de versão.
- O CLI descobre o endereço pelo diretório de dados do app; se não conseguir conectar em 200 ms, desiste em silêncio.
- Validar e limitar todo campo (mesmos limites da seção 5), renderizar como texto, abrir `url` só com `http(s)` e por clique explícito.
- O instalador de hooks e o link do CLI só atuam com ação explícita do usuário.
- Nenhum segredo em logs. Logs rotativos em `~/Library/Logs/miau/` sem conteúdo de mensagens por padrão.

## 9. Janelas e experiência de primeiro uso

Janelas: **overlay** (v1), **Configurações** (nova, janela normal) e **Onboarding** (primeira execução).

Onboarding (5 passos, todos puláveis):

1. Boas-vindas e teste: botão "Mostrar um gatinho" dispara um evento de exemplo.
2. "Conectar Claude Code": roda o instalador da seção 6.1.
3. "Conectar GitHub": usa `gh` se existir; senão device flow (6.2).
4. "Instalar o comando `miau`": cria link do CLI em `~/.local/bin` (sem exigir administrador) e avisa se esse diretório não estiver no `PATH`. Mostra o snippet de shell opcional (6.4).
5. "Iniciar com o login" e escolha de personagem, tema e som. Fim com um evento de teste de ponta a ponta.

Configurações: abas Fontes, Sons e Mídia, Aparência (personagem, tema e layout), Conexões (Claude, GitHub, CLI, token HTTP), Não perturbe (manual e agendado), Avançado (host do GitHub, logs, exportar/importar configuração).

## 10. Dados e credenciais

- Config: JSON versionado (`"version": 2`) com migração a partir da v1. Valores inválidos caem no padrão.
- Diretórios por SO via crate `dirs` / API do Tauri.
- Credenciais exclusivamente no chaveiro do sistema.
- Exportar/importar config exclui credenciais.

## 11. Personalização: personagens e temas

> **Atualizado em `miau-spec-personalizacao.md`** (schema 2: pixel art em 1x com paleta recolorível, balão por presets, 3 cores). Em caso de diferença, vale aquele documento; o texto abaixo fica como histórico.

Duas camadas independentes que o usuário combina livremente:

- **Personagem**: quem aparece avisando (gato, cachorro, robô, uma imagem própria do usuário).
- **Tema**: como o cartão e o balão se parecem (cores, bordas, fonte, forma do balão).

Mais um terceiro bloco, **layout e comportamento** (posição, tamanho, animações), que é preferência do usuário e não faz parte dos pacotes.

Princípio de segurança: **pacotes de personagem e de tema são dados, nunca código.** Isso permite compartilhar pacotes sem risco de executar algo malicioso.

### 11.1 Pacote de personagem (`.miaupack`)

Pasta ou arquivo zip renomeado para `.miaupack`:

```
meu-personagem/
├─ manifest.json
├─ preview.png
└─ assets/
   ├─ ok.json  (ou .svg/.png/.gif/.webp/sprite)
   ├─ fail.json
   ├─ attention.json
   └─ info.json
```

`manifest.json`:

```json
{
  "schema": 1,
  "id": "gato-laranja",
  "name": "Gato Laranja",
  "author": "Fulana",
  "license": "CC-BY-4.0",
  "preview": "preview.png",
  "size": { "width": 160, "height": 160 },
  "anchor": "bottom-left",
  "fallback": "info",
  "states": {
    "ok":        { "type": "lottie", "file": "assets/ok.json",        "loop": true },
    "fail":      { "type": "gif",    "file": "assets/fail.gif",       "loop": true },
    "attention": { "type": "sprite", "file": "assets/attn.png", "frames": 8, "columns": 4, "fps": 12, "loop": true },
    "info":      { "type": "image",  "file": "assets/info.png", "animation": "wiggle" }
  },
  "sounds": { "ok": "assets/ok.ogg" },
  "phrases": {
    "pt-BR": {
      "ok":        ["Seu pedido finalizou!", "Prontinho!"],
      "fail":      ["Algo deu errado por aqui"],
      "attention": ["Preciso de você!"],
      "info":      ["Aviso do Miau"]
    },
    "en": {
      "ok": ["Your request finished!", "All done!"]
    }
  }
}
```

Regras:

- Estados: `ok`, `fail`, `attention`, `info`. O que faltar usa o estado indicado em `fallback`; se nenhum existir, usa o personagem padrão do app.
- Tipos suportados: `svg`, `image` (PNG/WebP), `gif`/`apng`/`webp` animados, `sprite` (grade de quadros com `frames`, `columns`, `fps`) e `lottie` (JSON).
- Para `image` estática, o campo `animation` aplica uma animação pronta do app: `bounce`, `wiggle`, `shake`, `pulse` ou `none`. Assim qualquer PNG vira um personagem, sem saber animar.
- `phrases`: o app sorteia uma frase da lista do idioma atual. Uma `message` vinda no evento tem prioridade. Idioma ausente cai no inglês e depois nos textos padrão do app.
- `sounds` é opcional; o usuário escolhe se usa o som do personagem ou o som das configurações.
- `anchor` define de que lado o personagem fica em relação ao balão. O usuário pode espelhar nas configurações.

Limites (validados na importação): pacote até 5 MB, cada arquivo até 2 MB, no máximo 24 arquivos, dimensões até 1024 px, animações até 10 s.

### 11.2 Criar personagem sem código (editor embutido)

Nas Configurações, aba Aparência, botão **"Usar minha imagem"**:

1. Escolher uma imagem para todos os estados ou uma por estado (arrastar e soltar PNG, GIF, WebP, SVG).
2. Para imagem estática, escolher a animação por estado (bounce, wiggle, shake, pulse, nenhuma).
3. Ajustar tamanho, espelhar e posição em relação ao balão.
4. Editar as frases por estado e por idioma (campos de texto simples).
5. Pré-visualizar cada estado ao vivo; salvar com um nome.
6. **Exportar** como `.miaupack` para compartilhar.

Direitos autorais: o app **não inclui** personagens de terceiros e não baixa pacotes sozinho. Imagens que o usuário importa são responsabilidade dele. O formato exige os campos `author` e `license`, que aparecem na tela de confirmação de importação.

### 11.3 Tema (`.miautheme`)

Arquivo JSON de tokens de design. Sem CSS arbitrário na v1 (evita injeção e quebras de layout).

```json
{
  "schema": 1,
  "id": "rosa-pastel",
  "name": "Rosa Pastel",
  "author": "Fulana",
  "license": "CC0-1.0",
  "mode": "light",
  "tokens": {
    "card.bg": "#FFF0F5",
    "card.fg": "#4A2C3A",
    "card.border": "#F5B5CF",
    "card.radius": 18,
    "card.shadow": "soft",
    "card.opacity": 0.96,
    "status.ok": "#4CAF50",
    "status.fail": "#E53935",
    "status.attention": "#FFB300",
    "status.info": "#5C6BC0",
    "font.family": "rounded",
    "font.size": 14,
    "bubble.shape": "speech-tail"
  }
}
```

- `mode`: `light`, `dark` ou `auto` (segue o sistema).
- `card.shadow`: `none`, `soft` ou `strong`. `bubble.shape`: `rounded`, `pill`, `cloud` ou `speech-tail`.
- `status.*`: cores de destaque (faixa/ícone) de cada estado. **Nunca usar só a cor para indicar o estado**: o ícone e o texto também mudam, por acessibilidade.
- `font.family`: `system`, `rounded`, `mono` ou um arquivo de fonte (`woff2`, até 300 KB) dentro do pacote, com licença declarada.
- **Verificação de contraste:** ao salvar ou importar, o app calcula a razão de contraste entre `card.fg` e `card.bg` (e dos destaques) e **avisa** quando for menor que 4.5:1 (WCAG AA), sugerindo correção. O usuário pode ignorar o aviso, mas ele aparece.
- Temas embutidos: Claro, Escuro, Automático, Alto contraste, Pastel.
- Respeitar `prefers-reduced-motion` e uma opção própria "Reduzir movimento" que troca bounce/shake por fade.

### 11.4 Layout e comportamento (preferências do usuário)

- Posição: quatro cantos; monitor (onde está o cursor, principal ou específico); margem.
- Tamanho: escala de 50% a 200%.
- Lado do personagem e espelhamento.
- Duração padrão e comportamento ao passar o mouse (pausar o timer).
- Animação de entrada e saída: bounce, slide, fade ou nenhuma.
- Elementos visíveis: título, projeto, personagem (modo "só balão" quando desligado).
- Opacidade geral.
- Botão **"Pré-visualizar"** que dispara os 4 estados com as escolhas atuais.

### 11.5 Gerenciamento e combinações

- Diretórios no app data: `packs/characters/<id>/` e `packs/themes/<id>.miautheme`.
- Importar: arrastar o arquivo para a janela de Configurações ou botão "Importar". Mostrar resumo (nome, autor, licença, tamanho, estados incluídos) e pedir confirmação.
- Remover e exportar pelos mesmos controles. Embutidos não podem ser removidos, só ocultados.
- **Perfis de aparência:** combinação salva de personagem + tema + layout (ex.: "Trabalho", "Divertido").
- **Sobrescrita por fonte:** cada fonte pode usar um perfil diferente. Exemplo: Claude Code com o gato laranja, CI com o robô. Campo opcional `"profile"` em `sources.<fonte>` na config.

### 11.6 Segurança dos pacotes

- Pacote é dado, não código. Nenhum script é executado.
- SVG renderizado somente via `<img>` (scripts não rodam), com sanitização adicional e bloqueio de referências remotas (`http://`, `https://`, `file://`) dentro do SVG.
- Lottie: desabilitar expressões e qualquer recurso que execute código. **VERIFICAR** o suporte da biblioteca escolhida.
- Importação de zip com proteção contra **zip slip**: rejeitar entradas com `..`, caminhos absolutos ou links simbólicos; permitir apenas extensões da lista (`json`, `png`, `webp`, `gif`, `svg`, `ogg`, `wav`, `woff2`); conferir tipo real do arquivo (não confiar só na extensão); validar tamanhos e contagem de arquivos.
- CSP restritiva no webview do overlay: sem scripts inline ou remotos, imagens só de origem local (**VERIFICAR** a sintaxe de CSP do Tauri v2).
- Todo texto (frases, nomes, autor) renderizado como texto puro, nunca como HTML.
- Sem download automático de pacotes na v1. Uma galeria futura precisa de curadoria, revisão de licença e assinatura dos pacotes.

### 11.7 Ideias futuras

Galeria comunitária curada, pacotes sazonais, personagem que reage ao histórico (ex.: três falhas seguidas), frases com variáveis (`{project}`, `{duration}`), voz curta opcional.

## 12. Multiplataforma

| Recurso | macOS | Windows | Linux |
|---|---|---|---|
| Overlay transparente sempre no topo | sim (`macOSPrivateApi`) | sim | depende do ambiente (Wayland pode limitar) |
| Pausar mídia | AppleScript + opcional Now Playing | SMTC | MPRIS |
| Socket local | Unix socket | named pipe | Unix socket |
| Autostart | Login Item | Registro/Startup | `.desktop` em autostart |
| Credenciais | Keychain | Credential Manager | Secret Service |

Começar por macOS (já validado). Windows em seguida. Linux por último, com fallback para notificação nativa quando o overlay não for confiável.

## 12.1 Abstrações obrigatórias

Tudo que for específico de sistema fica atrás de traits em `miau-sinks` (`MediaController`, `Autostart`, `CredentialStore`), com implementação por `cfg(target_os)`. Nada de `if cfg!` solto na lógica de negócio.

## 13. Empacotamento, assinatura, atualização e CI

**Build:** `tauri build` gera `.app`/`.dmg` (macOS), `.msi`/`.exe` (Windows), `AppImage`/`.deb` (Linux).

**macOS:**
- Assinar com certificado **Developer ID Application** e **notarizar** (conta Apple Developer, cerca de US$ 99/ano). Sem isso, o Gatekeeper bloqueia o app para outras pessoas.
- Build universal (arm64 + x86_64) ou dois artefatos.
- `macOSPrivateApi` impede a Mac App Store; a distribuição é direta e via Homebrew.
- Textos de permissão claros (`NSAppleEventsUsageDescription`) para Automação do Spotify/Music. **VERIFICAR** a chave no `Info.plist` gerado pelo Tauri.

**Windows:** sem assinatura, o SmartScreen avisa. Avaliar assinatura de código (certificado ou serviço de assinatura) quando houver usuários externos.

**Atualização automática:** plugin de updater do Tauri com **assinatura própria das atualizações** (par de chaves; chave privada só em segredo do CI). Publicar `latest.json` nos GitHub Releases. **VERIFICAR** o fluxo atual.

**CI/CD (GitHub Actions):**
- Em PR: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `vitest`, `tsc --noEmit`.
- Em tag `v*`: matriz macOS/Windows/Linux usando `tauri-apps/tauri-action`, com segredos para assinatura/notarização e para a chave do updater, publicando o release com os instaladores e o `latest.json`.
- Nunca commitar certificados, chaves ou tokens. Documentar os segredos necessários no README.

## 14. Distribuição

- GitHub Releases (fonte oficial).
- Homebrew: começar com um tap próprio (`brew install --cask <usuario>/miau/miau`); depois considerar o repositório oficial de casks.
- Windows: instalador `.msi`/`.exe`; `winget` mais tarde.
- Linux: AppImage e `.deb`.
- Página simples (GitHub Pages) com GIF de demonstração, botão de download e a lista de integrações.

## 15. Privacidade e transparência

- Sem telemetria por padrão. Se houver, será opt-in, descrita no onboarding e documentada.
- O app só acessa a rede para o GitHub (quando ligado) e para checar atualizações (com opção de desligar).
- `PRIVACY.md` curto no repositório explicando: o que é lido (status de PRs, eventos locais), o que é guardado (config, token no chaveiro) e o que nunca é enviado.

## 16. Testes

- `miau-core`: regras, deduplicação, DND agendado, migração de config (testes unitários).
- Instalador de hooks: os cenários listados em 6.1, em diretório temporário.
- GitHub: respostas gravadas (fixtures) do GraphQL para testar transições (pendente→sucesso, pendente→falha, novo push, primeira leitura) sem rede.
- IPC: servidor e cliente em teste de integração; CLI com app fechado sai com 0 em menos de 300 ms.
- Mídia: implementação fake da trait para testar "só retoma o que pausou".
- Manual por SO: checklist de onboarding de ponta a ponta numa máquina limpa (VM).

## 17. Marcos da v2

**Marco 7: núcleo e destinos internos**
- Refatorar para workspace; `miau-core` com evento, regras e fila; som em Rust; migração de config v1 para v2.
- Aceite: testes passam; evento de teste mostra gato e toca som sem nenhum script externo.

**Marco 8: IPC e CLI**
- Servidor de socket; `miau-cli` com `notify`, `run`, `hook`; sidecar empacotado.
- Aceite: `miau notify --status ok --message teste` mostra o gato; com o app fechado, o comando sai com 0 sem erro e rápido; `miau run -- false` gera aviso de falha e devolve código 1.

**Marco 9: conector do Claude Code**
- Instalador/desinstalador de hooks com modo manual e backup.
- Aceite: instalar, abrir sessão nova, ver o gato ao fim de uma resposta; desinstalar remove só as entradas do Miau; com `settings.json` somente leitura, o app mostra o modo manual.

**Marco 10: pausa de mídia nativa (macOS)**
- Trait `MediaController`, Spotify/Music via AppleScript, estratégia para mídia de navegador definida e documentada, botão "retomar".
- Aceite: música tocando pausa e retoma sozinha ao clicar; música já pausada continua pausada; nada é iniciado se nada estava tocando.

**Marco 11: monitor de PRs do GitHub**
- Autenticação (gh e device flow), chaveiro, consulta GraphQL, algoritmo de transição, polling adaptativo.
- Aceite: com um PR de CI em andamento, o aviso chega ao terminar; PR já finalizado não dispara ao abrir o app; novo push reinicia o acompanhamento.

**Marco 12: Configurações e onboarding**
- Janelas novas, fluxo de 5 passos, DND agendado, geração e exibição do token HTTP.
- Aceite: numa máquina limpa, o fluxo completo leva menos de 3 minutos e termina com evento de teste.

**Marco 13: personagens e temas**
- Formatos `.miaupack` e `.miautheme`, validador de importação (limites, zip slip, extensões), personagem e temas embutidos, editor "Usar minha imagem", layout, perfis e sobrescrita por fonte.
- Aceite: trocar personagem, tema ou layout altera o overlay sem reiniciar; um PNG qualquer vira personagem pelo editor e pode ser exportado e reimportado; pacote com manifest inválido, zip com `..` no caminho ou SVG com referência remota é rejeitado com mensagem clara; tema com contraste abaixo de 4.5:1 mostra aviso; a opção "Reduzir movimento" troca as animações por fade.

**Marco 14: empacotamento e release**
- Assinatura e notarização (macOS), updater, workflows de CI e release, tap do Homebrew.
- Aceite: `.dmg` baixado de um release abre em outro Mac sem aviso do Gatekeeper e atualiza para uma versão seguinte.

**Marco 15: Windows e Linux**
- Implementações de mídia, autostart, credenciais e IPC por SO.
- Aceite: onboarding e os três tipos de fonte funcionando no Windows; Linux com fallback de notificação documentado.

## 18. Riscos e decisões em aberto

- **Pausa de mídia de navegador no macOS:** depende de APIs restritas pela Apple. Pode virar "só Spotify/Music" no produto final. Decidir depois de testar em várias versões do macOS.
- **Escopo do token do GitHub:** ler PRs privados pode exigir escopo amplo no OAuth App. Avaliar GitHub App ou fine-grained token e documentar o trade-off.
- **Overlay sobre apps em tela cheia e em vários Spaces:** limitado pelo Tauri/janelas do sistema. Pode exigir código nativo.
- **Custo e burocracia de assinatura:** conta Apple Developer e, no Windows, certificado. Sem eles, só distribuição para testadores avançados.
- **Hooks do Claude Code mudam com o tempo:** isolar o parsing do stdin em um módulo único, tolerante a campos novos ou ausentes.
- **Nome e marca:** conferir se "Miau" já é usado como marca em apps semelhantes antes de divulgar.
- **Licença:** escolher (ex.: MIT ou Apache-2.0) e aplicar em todo o código. Garantir que todos os sons e artes sejam originais ou de licença compatível, com créditos em `ASSETS.md`.
- **Personagens importados pelo usuário:** podem violar direitos autorais de terceiros. O app não distribui nem baixa personagens de terceiros, exige `author` e `license` nos pacotes e deixa a responsabilidade do uso pessoal com quem importa. Revisar essa política antes de criar qualquer galeria pública.
- **Lottie e SVG vindos de pacotes:** são a maior superfície de ataque da personalização. Manter a renderização isolada (`<img>`, CSP restritiva, sem expressões) e testar com arquivos maliciosos de exemplo.
