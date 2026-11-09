# Quacko: personalização de personagens, balões e cores

> Documento independente, extraído da spec v2 (`quacko-spec-v2-produto.md`, seção 11). Descreve como o usuário personaliza o personagem que avisa, o estilo do balão e as cores.
>
> Pré-requisitos: overlay e cartão funcionando (Marco 3 da spec v1) e janela de Configurações (Marco 12 da spec v2). Se algum não existir ainda, implemente a parte visual primeiro e deixe a personalização para depois.
>
> Como usar: coloque na raiz do repositório e peça ao Claude Code: "Leia quacko-spec-personalizacao.md e implemente na ordem da seção 8, um passo por vez. Ao fim de cada passo, rode os critérios de aceite, me diga como testar e pare para eu validar. Itens marcados VERIFICAR devem ser conferidos na documentação atual antes de usar."

Quatro camadas independentes, que o usuário combina livremente:

- **Personagem**: quem aparece avisando (gato, cachorro, robô, uma imagem própria). Pode ser recolorido quando for pixel art com paleta.
- **Balão**: o estilo da caixa de texto (forma, borda, sombra, rabinho, fonte, etiqueta do projeto).
- **Cores**: três cores escolhidas pelo usuário (papel, tinta, destaque), das quais o app deriva o resto.
- **Layout e comportamento**: posição, tamanho, animações. É preferência do usuário e não faz parte dos pacotes.

Dois tipos de pacote: `.quackopack` (personagem) e `.quackotheme` (balão + cores). O balão e as cores formam um **tema**.

Princípio de segurança: **pacotes de personagem e de tema são dados, nunca código.** Isso permite compartilhar pacotes sem risco de executar algo malicioso.

## 1. Pacote de personagem (`.quackopack`)

Pasta ou arquivo zip renomeado para `.quackopack`:

```
meu-personagem/
├─ manifest.json
├─ preview.png
└─ assets/
   ├─ ok.png  (ou .svg/.gif/.webp/lottie .json)
   ├─ fail.png
   ├─ attention.png
   └─ info.png
```

`manifest.json` de um personagem **pixel art** (schema 2):

```json
{
  "schema": 2,
  "id": "gato-pixel",
  "name": "Gato Pixel",
  "author": "Fulana",
  "license": "CC0-1.0",
  "preview": "preview.png",
  "pixel_art": true,
  "grid": { "width": 28, "height": 28 },
  "bubble": "pixel",
  "anchor": "bottom-left",
  "fallback": "info",
  "states": {
    "ok":        { "type": "sprite", "file": "assets/ok.png",        "frames": 2, "columns": 2, "fps": 3, "loop": true },
    "fail":      { "type": "sprite", "file": "assets/fail.png",      "frames": 2, "columns": 2, "fps": 3, "loop": true },
    "attention": { "type": "sprite", "file": "assets/attention.png", "frames": 2, "columns": 2, "fps": 3, "loop": true },
    "info":      { "type": "sprite", "file": "assets/info.png",      "frames": 2, "columns": 2, "fps": 3, "loop": true }
  },
  "palette": {
    "fur": "#fffaf5", "fur_shade": "#eadfd6",
    "patch": "#e8964a", "patch_light": "#f4bd85", "patch_dark": "#c4702f",
    "blush": "#f7a8b0", "outline": "#2b2024"
  },
  "sounds": { "ok": "assets/ok.ogg" },
  "phrases": {
    "pt-BR": {
      "ok": ["Seu pedido finalizou!", "Prontinho!"],
      "fail": ["Algo deu errado por aqui"],
      "attention": ["Preciso de você!"],
      "info": ["Aviso do Quacko"]
    },
    "en": { "ok": ["Your request finished!", "All done!"] }
  }
}
```

Para personagens **não pixel art** (ilustrações, GIFs, Lottie), usar `"pixel_art": false` e `"size": { "width": 160, "height": 160 }` em pixels no lugar de `grid`, sem `palette`.

Compatibilidade: o app também lê `schema: 1` (com `size` em pixels, usado pelo pacote inicial do Gato Pixel, gerado em 6x) e o trata como personagem não pixel art até ser migrado.

Regras gerais:

- Estados: `ok`, `fail`, `attention`, `info`. O que faltar usa o estado indicado em `fallback`; se nenhum existir, usa o personagem padrão do app.
- Tipos suportados: `svg`, `image` (PNG/WebP), `gif`/`apng`/`webp` animados, `sprite` (grade de quadros com `frames`, `columns`, `fps`) e `lottie` (JSON).
- Para `image` estática, o campo `animation` aplica uma animação pronta do app: `bounce`, `wiggle`, `shake`, `pulse` ou `none`. Assim qualquer PNG vira um personagem, sem saber animar.
- `phrases`: o app sorteia uma frase da lista do idioma atual. Uma `message` vinda no evento tem prioridade. Idioma ausente cai no inglês e depois nos textos padrão do app.
- `sounds` é opcional; o usuário escolhe se usa o som do personagem ou o som das configurações.
- `anchor` define de que lado o personagem fica em relação ao balão. O usuário pode espelhar nas configurações.
- `bubble` é uma **sugestão** de preset de balão (ver seção 3). Nunca é imposta: o usuário decide se aceita ao escolher o personagem.

Regras específicas de **pixel art** (`"pixel_art": true`):

- Os PNGs ficam em **1x**: cada pixel do desenho é 1 pixel da imagem, e os quadros ficam lado a lado (neste exemplo, folha de 56x28 com 2 quadros de 28x28). Nada de ampliar o arquivo.
- O app amplia por CSS em **múltiplos inteiros** (o "tamanho do pixel", ver seção 4), com `image-rendering: pixelated`. Ampliar por fator não inteiro deforma os pixels.
- **Paleta (`palette`)**: lista os papéis de cor que o sprite usa. Cada valor é a cor **exata** presente nos PNGs. É isso que permite recolorir sem criar arte nova (ver seção 2). Papéis não declarados não são recoloráveis. Os efeitos (coração, exclamação, gota de suor) e o brilho dos olhos têm cores fixas.
- Papéis sugeridos: `fur`, `fur_shade`, `patch`, `patch_light`, `patch_dark`, `blush`, `outline`.

Limites (validados na importação): pacote até 5 MB, cada arquivo até 2 MB, no máximo 24 arquivos, animações até 10 s. Para não pixel art, dimensões até 1024 px. Para pixel art, grade até 128x128 pixels de desenho.

## 2. Criar e recolorir personagem sem código (editor embutido)

Nas Configurações, aba Aparência:

**"Usar minha imagem"**

1. Escolher uma imagem para todos os estados ou uma por estado (arrastar e soltar PNG, GIF, WebP, SVG).
2. Marcar **"é pixel art"** para a imagem ser ampliada sem borrar.
3. Para imagem estática, escolher a animação por estado (bounce, wiggle, shake, pulse, nenhuma).
4. Ajustar tamanho, espelhar e posição em relação ao balão.
5. Editar as frases por estado e por idioma (campos de texto simples).
6. Pré-visualizar cada estado ao vivo; salvar com um nome.
7. **Exportar** como `.quackopack` para compartilhar.

**"Recolorir"** (só para personagens pixel art com `palette`)

- Um seletor de cor por papel (pelagem, sombra, mancha, mancha clara, mancha escura, bochecha, contorno), com pré-visualização ao vivo dos 4 estados.
- Atalhos de pelagem prontos (ex.: laranja e branco, preto, cinza, siamês, rosa) e botão "Restaurar original".
- Funcionamento: o app desenha o sprite em um canvas local e troca cada cor exata pela cor escolhida. Sem rede, sem arquivos novos. A escolha fica no perfil do usuário (seção 5) e pode ser exportada junto ao `.quackopack` como paleta alternativa.
- Aviso de contraste quando o usuário deixar o contorno muito parecido com o pelo (o personagem some).

Direitos autorais: o app **não inclui** personagens de terceiros e não baixa pacotes sozinho. Imagens que o usuário importa são responsabilidade dele. O formato exige os campos `author` e `license`, que aparecem na tela de confirmação de importação.

## 3. Tema (`.quackotheme`): balão e cores

Arquivo JSON, sem CSS livre (evita injeção e quebras de layout). O tema escolhe um **preset** de balão e pode ajustar cada escolha individualmente.

```json
{
  "schema": 2,
  "id": "rosa-pastel",
  "name": "Rosa Pastel",
  "author": "Fulana",
  "license": "CC0-1.0",
  "mode": "light",
  "colors": { "paper": "#fff0f5", "ink": "#4a2c3a", "accent": "#e57aa3" },
  "status": { "ok": "#4caf50", "fail": "#e53935", "attention": "#ffb300", "info": "#5c6bc0" },
  "bubble": {
    "preset": "pixel",
    "shape": "pixel",
    "border": "single",
    "shadow": "hard",
    "tail": "side",
    "font": "sans",
    "chip": "notched",
    "decor": []
  }
}
```

`preset` define os valores iniciais; qualquer campo informado depois sobrescreve o preset. Valores desconhecidos ou combinações inválidas são ignorados e caem no valor do preset, sem erro.

**Escolhas do balão** (listas fechadas):

| Campo | Valores |
|---|---|
| `shape` | `pixel`, `rounded`, `cloud`, `rect`, `note` |
| `border` | `none`, `single`, `double`, `window` (com barra de título) |
| `shadow` | `none`, `hard` (deslocada, sem blur), `soft` |
| `tail` | `side` (rabinho lateral), `thought` (bolinhas), `none`, `nametag` (aba com nome) |
| `font` | `sans`, `mono`, `pixel`, `italic` |
| `chip` | `pill`, `notched`, `text` (etiqueta do projeto) |
| `decor` | lista de `tape`, `dogear`, `window_dots`, `continue_arrow` |

**Presets da v2** (cada um é só um conjunto dessas escolhas):

| Preset | shape | border | shadow | tail | font | chip | decor |
|---|---|---|---|---|---|---|---|
| `pixel` (padrão) | pixel | single | hard | side | sans | notched | |
| `soft` | rounded | none | soft | side | sans | pill | |
| `thought` | cloud | single | hard | thought | sans | text | |
| `terminal` | rect | window | none | none | mono | text | window_dots |

Presets para depois da v2: `rpg` (shape pixel, border double, tail nametag, decor continue_arrow) e `note` (shape note, shadow soft, tail none, font italic, decor tape e dogear).

**Cores**:

- O usuário escolhe **3 cores**: `paper` (fundo do balão), `ink` (texto e contorno) e `accent` (destaque). O app deriva o restante: texto secundário (`muted`), fundo da etiqueta, cor da sombra e as cores de cada estado (`status.*`, que o tema pode sobrescrever).
- `mode`: `light`, `dark` ou `auto` (segue o sistema). Em `auto`, o tema pode trazer `colors_dark`; se não trouxer, o app deriva uma variante escura (inverte a luminosidade de `paper` e `ink` e clareia os destaques). No tema escuro, o contorno `pixel` passa a ser claro para não sumir.
- **Verificação de contraste ao salvar ou importar**, com aviso (o usuário pode ignorar, mas o aviso aparece):
  - texto sobre o papel: pelo menos 4.5:1 (WCAG AA);
  - texto secundário e etiqueta: pelo menos 4.5:1;
  - ícones e contornos de estado contra o papel: pelo menos 3:1.
  Quando falhar, o app sugere a cor mais próxima que passa.
- **Nunca usar só a cor para indicar o estado**: cada estado tem ícone próprio (✓, ✕, !, i) e texto.
- `font`: `sans` (fonte do sistema, padrão e recomendada para a mensagem), `mono`, `pixel` (fonte embutida com licença livre, guardada dentro do app, sem CDN; usar por padrão só no título, porque frases longas ficam cansativas) e `italic`. Opcionalmente, um arquivo `woff2` (até 300 KB) dentro do pacote, com licença declarada.
- Temas embutidos: Claro, Escuro, Automático, Alto contraste, mais os presets acima.
- Respeitar `prefers-reduced-motion` e uma opção própria "Reduzir movimento" que troca bounce/shake por fade e desliga o balanço do personagem.

**Acoplamento ao pixel do personagem**: nos formatos `pixel` e `cloud`, o contorno, os degraus dos cantos e a sombra usam o **mesmo tamanho de pixel do personagem**. Pixels de tamanhos diferentes na mesma tela quebram o visual. Se o personagem não for pixel art, o balão `pixel` usa 4 px por pixel.

## 4. Layout e comportamento (preferências do usuário)

- Posição: quatro cantos; monitor (onde está o cursor, principal ou específico); margem.
- **Tamanho**:
  - personagem pixel art: **"tamanho do pixel"**, com valores inteiros (3, 4, 5, 6 ou 8 px por pixel de desenho). O balão acompanha o mesmo valor. Não oferecer porcentagem livre, porque fator não inteiro deforma o pixel art;
  - demais personagens: escala de 50% a 200%.
- **Telas com escala fracionária** (ex.: Windows em 125% ou 150%): normalizar o tamanho do pixel para que `px × devicePixelRatio` seja inteiro. Caso contrário, alguns pixels do desenho ficam maiores que outros.
- Lado do personagem e espelhamento.
- Duração padrão e comportamento ao passar o mouse (pausar o timer).
- Animação de entrada e saída: bounce, slide, fade ou nenhuma. Em estilos `pixel`, a entrada em passos (`steps()`), sem suavização, combina melhor.
- Elementos visíveis: título, projeto, personagem (modo "só balão" quando desligado).
- Opacidade geral.
- Botão **"Pré-visualizar"** que dispara os 4 estados com as escolhas atuais.

## 5. Gerenciamento e combinações

- Diretórios no app data: `packs/characters/<id>/` e `packs/themes/<id>.quackotheme`.
- Importar: arrastar o arquivo para a janela de Configurações ou botão "Importar". Mostrar resumo (nome, autor, licença, tamanho, estados incluídos, aviso de contraste) e pedir confirmação.
- Remover e exportar pelos mesmos controles. Embutidos não podem ser removidos, só ocultados.
- **Perfis de aparência**: combinação salva de personagem (com a paleta recolorida, se houver) + tema + layout (ex.: "Trabalho", "Divertido").
- **Sugestão de balão pelo personagem**: ao escolher um personagem cujo `manifest` traga `bubble`, o app **oferece** o preset sugerido ("Usar o balão pixel que combina com este gato?") e só aplica se o usuário aceitar.
- **Sobrescrita por fonte**: cada fonte pode usar um perfil diferente. Exemplo: Claude Code com o gato laranja e balão pixel, CI com o robô e balão terminal. Campo opcional `"profile"` em `sources.<fonte>` na config.

## 6. Segurança dos pacotes

- Pacote é dado, não código. Nenhum script é executado.
- Cores aceitas somente como `#rrggbb` ou `#rrggbbaa`. Todas as escolhas de balão vêm de listas fechadas; o preset e os campos são validados contra essas listas. Nada é interpretado como CSS.
- A recoloração substitui cores exatas em um canvas local; a paleta não pode referenciar arquivos nem URLs.
- SVG renderizado somente via `<img>` (scripts não rodam), com sanitização adicional e bloqueio de referências remotas (`http://`, `https://`, `file://`) dentro do SVG.
- Lottie: desabilitar expressões e qualquer recurso que execute código. **VERIFICAR** o suporte da biblioteca escolhida.
- Importação de zip com proteção contra **zip slip**: rejeitar entradas com `..`, caminhos absolutos ou links simbólicos; permitir apenas extensões da lista (`json`, `png`, `webp`, `gif`, `svg`, `ogg`, `wav`, `woff2`); conferir tipo real do arquivo (não confiar só na extensão); validar tamanhos e contagem de arquivos.
- CSP restritiva no webview do overlay: sem scripts inline ou remotos, imagens só de origem local (**VERIFICAR** a sintaxe de CSP do Tauri v2).
- Todo texto (frases, nomes, autor) renderizado como texto puro, nunca como HTML.
- Sem download automático de pacotes na v1. Uma galeria futura precisa de curadoria, revisão de licença e assinatura dos pacotes.

## 7. Ideias futuras

Presets `rpg` e `note`, balão de vidro translúcido, balão de quadrinhos para `attention`, toast com barra de cor, galeria comunitária curada, pacotes sazonais, personagem que reage ao histórico (ex.: três falhas seguidas), frases com variáveis (`{project}`, `{duration}`), voz curta opcional.

## 8. Marco de implementação

- Formatos `.quackopack` e `.quackotheme` (schema 2, lendo também schema 1), validador de importação (limites, zip slip, extensões, cores, listas fechadas), personagem e temas embutidos.
- **Migrar o Gato Pixel para 1x:** no gerador (`tools/make_pixel_cat.py`), usar `SCALE = 1` e acrescentar no manifest `schema: 2`, `pixel_art: true`, `grid` (28x28), `bubble: "pixel"` e `palette` com os papéis `fur`, `fur_shade`, `patch`, `patch_light`, `patch_dark`, `blush`, `outline` (valores iguais às cores do dicionário `PAL`: `W`, `S`, `G`, `g`, `D`, `P`, `K`).
- Tamanho do pixel inteiro (3, 4, 5, 6, 8) com `image-rendering: pixelated`, normalizado pelo `devicePixelRatio`.
- Balão por presets (`pixel`, `soft`, `thought`, `terminal`) e escolhas individuais; 3 cores (`paper`, `ink`, `accent`) com derivação das demais e verificação de contraste.
- Recolorir o personagem por troca de cor exata em canvas, com atalhos de pelagem e "Restaurar original".
- Editor "Usar minha imagem", layout, perfis e sobrescrita por fonte; sugestão de balão pelo personagem.
- Aceite:
  - trocar personagem, preset, cores ou layout altera o overlay sem reiniciar;
  - o pixel art fica nítido em todos os tamanhos de pixel, inclusive em tela com escala 125% ou 150% (nenhum pixel maior que os outros);
  - recolorir o gato muda pelagem e manchas nos 4 estados ao vivo, sem tocar nos efeitos (coração, exclamação, gota) e no brilho dos olhos;
  - um PNG qualquer vira personagem pelo editor e pode ser exportado e reimportado;
  - pacote com manifest inválido, zip com `..` no caminho, SVG com referência remota, cor em formato inválido ou valor fora das listas é rejeitado ou ignorado com mensagem clara;
  - tema com contraste abaixo de 4.5:1 (texto) ou 3:1 (ícones) mostra aviso e sugere correção;
  - a opção "Reduzir movimento" troca as animações por fade.

Ordem sugerida (cada passo termina com algo testável):

1. **Formatos e validador:** `.quackopack` e `.quackotheme` (schema 2, lendo também schema 1), importação e exportação, validações da seção 6.
2. **Pixel nítido:** migrar o Gato Pixel para 1x, tamanho do pixel inteiro, normalização pelo `devicePixelRatio`.
3. **Balão por presets:** `pixel`, `soft`, `thought`, `terminal` e as escolhas individuais.
4. **Cores:** 3 cores editáveis, derivação das demais, verificação de contraste, tema escuro derivado.
5. **Recolorir o personagem:** troca de cor exata em canvas, atalhos de pelagem, restaurar original.
6. **Editor e perfis:** "Usar minha imagem", perfis de aparência, sobrescrita por fonte, sugestão de balão pelo personagem.

## 9. Riscos e decisões em aberto

- **Personagens importados pelo usuário:** podem violar direitos autorais de terceiros. O app não distribui nem baixa personagens de terceiros, exige `author` e `license` nos pacotes e deixa a responsabilidade do uso pessoal com quem importa. Revisar essa política antes de criar qualquer galeria pública.
- **Lottie e SVG vindos de pacotes:** são a maior superfície de ataque da personalização. Manter a renderização isolada (`<img>`, CSP restritiva, sem expressões) e testar com arquivos maliciosos de exemplo.
- **Recoloração só funciona em pixel art com paleta:** depende de o sprite usar cores exatas e sem anti-aliasing. Para GIF, Lottie e ilustrações lisas não se aplica; documentar isso na tela de recolorir.
- **Pixel art e escala:** tamanhos fracionários (porcentagem livre, telas com 125% ou 150%) deformam os pixels. Manter o "tamanho do pixel" inteiro e normalizado pelo `devicePixelRatio`, e testar em Windows e em monitores externos.
- **Combinações de escolhas do balão:** nem toda combinação fica boa (ex.: `window` com `thought`). Cada preset novo exige testes visuais nos 4 estados e nos temas claro e escuro; por isso a v2 entrega só 4 presets.
- **Contraste escolhido pelo usuário:** o app avisa, mas não impede. Decidir se um tema com contraste muito baixo pode ser aplicado como padrão do sistema ou só como escolha explícita.
