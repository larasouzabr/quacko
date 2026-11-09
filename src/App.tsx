import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useReducer, useRef, useState } from "react";
import { builtinCharacter } from "./character/builtin";
import { DEFAULT_PIXEL_SIZE } from "./character/pixel";
import Card from "./components/Card";
import { characterBox } from "./components/Cat";
import { useDevicePixelRatio } from "./hooks/useDevicePixelRatio";
import type { Lang } from "./i18n";
import { initialQueue, isEmpty, queueReducer } from "./state/queue";
import type { Notification, Settings } from "./types";

// Valores usados até o Rust responder (ou se o app rodar fora do Tauri).
const DEFAULT_SETTINGS: Settings = { lang: "pt-BR", max_cards: 3, pixel_size: DEFAULT_PIXEL_SIZE };
const TICK_MS = 100;

// Medidas da pilha (px CSS), iguais às de styles.css, para calcular o tamanho da janela.
const STACK_PAD_X = 16;
const STACK_PAD_Y = 14;
const CARD_GAP = 8;
const CAT_BUBBLE_GAP = 2;
const BUBBLE_WIDTH = 278;
/** Altura máxima do balão (3 linhas de mensagem + projeto), com a margem de baixo. */
const BUBBLE_MAX_HEIGHT = 132; // +11 da barra de progresso

/** Janela do tamanho exato para `maxCards` cartões com o personagem atual. */
function overlaySize(cat: { width: number; height: number }, maxCards: number) {
  const cardHeight = Math.max(cat.height, BUBBLE_MAX_HEIGHT);
  return {
    width: Math.ceil(STACK_PAD_X + cat.width + CAT_BUBBLE_GAP + BUBBLE_WIDTH),
    height: Math.ceil(STACK_PAD_Y + maxCards * cardHeight + (maxCards - 1) * CARD_GAP),
  };
}

/** Posição do mouse dentro do overlay (px CSS), ou null fora dele. Enviada pelo Rust,
 * porque o WebKit não entrega eventos de hover numa janela que não está em foco. */
type Pointer = [number, number] | null;

function quiet(promise: Promise<unknown>) {
  promise.catch((e) => console.warn(e));
}

export default function App() {
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [queue, dispatch] = useReducer(queueReducer, DEFAULT_SETTINGS.max_cards, initialQueue);
  const [hoveredId, setHoveredId] = useState<number | null>(null);
  const lastSeenId = useRef(0);
  const shownSomething = useRef(false);

  // Idioma e limite de cartões: lidos no início e atualizados quando mudam no menu.
  useEffect(() => {
    invoke<Settings>("get_settings").then(setSettings, (e) => console.warn(e));
    const unlisten = listen<Settings>("settings-changed", ({ payload }) => setSettings(payload));
    return () => void unlisten.then((off) => off());
  }, []);

  useEffect(() => {
    dispatch({ type: "configure", maxCards: settings.max_cards });
  }, [settings.max_cards]);

  // A janela acompanha o tamanho do personagem (tamanho do pixel) e o limite de cartões.
  const dpr = useDevicePixelRatio();
  const cat = characterBox(builtinCharacter, settings.pixel_size, 1, dpr);
  const size = overlaySize(cat, settings.max_cards);
  useEffect(() => {
    quiet(invoke("set_overlay_size", { width: size.width, height: size.height }));
  }, [size.width, size.height]);

  // Avisos vindos do Rust.
  useEffect(() => {
    const unlisten = listen<Notification>("notify", ({ payload }) => {
      lastSeenId.current = Math.max(lastSeenId.current, payload.id);
      dispatch({ type: "add", notification: payload, now: Date.now() });
    });
    return () => void unlisten.then((off) => off());
  }, []);

  // Posição do mouse: descobre qual cartão está embaixo do cursor.
  useEffect(() => {
    const unlisten = listen<Pointer>("overlay-pointer", ({ payload }) => {
      const el = payload ? document.elementFromPoint(payload[0], payload[1]) : null;
      const card = el?.closest<HTMLElement>("[data-card-id]");
      setHoveredId(card ? Number(card.dataset.cardId) : null);
    });
    return () => void unlisten.then((off) => off());
  }, []);

  // Mouse sobre um cartão: pausa os timers e deixa a janela receber cliques.
  // Fora dos cartões, a janela deixa os cliques passarem para o app de baixo.
  const overCard = hoveredId !== null;
  useEffect(() => {
    dispatch({ type: overCard ? "pause" : "resume", now: Date.now() });
    quiet(invoke("set_interactive", { interactive: overCard }));
  }, [overCard]);

  // Relógio da fila (expiração e fim do fade).
  const hasCards = !isEmpty(queue);
  useEffect(() => {
    if (!hasCards) return;
    const timer = setInterval(() => dispatch({ type: "tick", now: Date.now() }), TICK_MS);
    return () => clearInterval(timer);
  }, [hasCards]);

  // Fila vazia: pede ao Rust para esconder a janela.
  useEffect(() => {
    if (hasCards) {
      shownSomething.current = true;
      return;
    }
    if (!shownSomething.current) return;
    shownSomething.current = false;
    setHoveredId(null);
    quiet(invoke("overlay_empty", { lastSeenId: lastSeenId.current }));
  }, [hasCards]);

  return (
    <main className="stack">
      {/* A fila guarda o mais novo no fim; na tela ele vai para o topo, perto da borda. */}
      {[...queue.cards].reverse().map((card) => (
        <Card
          key={card.n.id}
          notification={card.n}
          lang={settings.lang as Lang}
          leaving={card.leavingAt !== null}
          hovered={hoveredId === card.n.id}
          pixelSize={settings.pixel_size}
          onActivate={() => {
            // Cartão sticky (CI que terminou) só fecha no ×: o clique só abre o link.
            if (!card.n.sticky) dispatch({ type: "dismiss", id: card.n.id, now: Date.now() });
            quiet(invoke("card_action", { url: card.n.url }));
          }}
          onDismiss={() => {
            dispatch({ type: "dismiss", id: card.n.id, now: Date.now() });
            quiet(invoke("card_action", { url: null }));
          }}
        />
      ))}
    </main>
  );
}
