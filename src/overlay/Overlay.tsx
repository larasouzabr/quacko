import { useCallback, useEffect, useReducer } from "react";
import { BUILTIN_ACCESSORIES } from "../accessory/builtin";
import { useAccessoryOverlay } from "../accessory/useAccessory";
import { getCharacter } from "../character/builtin";
import Card from "../components/Card";
import type { Lang } from "../i18n";
import { DEFAULT_SETTINGS, useSettings } from "../settings";
import { initialQueue, queueReducer } from "../state/queue";
import { useHideWhenEmpty, useHoveredCard, useIncomingNotifications, useQueueClock, useTrayAlert } from "./hooks";
import { overlayIpc } from "./ipc";
import { useOverlaySize } from "./size";

/** Janela do overlay: a pilha de cartões com o personagem escolhido. */
export default function Overlay() {
  const settings = useSettings();
  const [queue, dispatch] = useReducer(queueReducer, DEFAULT_SETTINGS.max_cards, initialQueue);
  useEffect(() => {
    dispatch({ type: "configure", maxCards: settings.max_cards });
  }, [settings.max_cards]);

  const character = getCharacter(settings.character);
  const accessory = settings.accessory_enabled ? BUILTIN_ACCESSORIES[settings.accessory_id] : undefined;
  const overlay = useAccessoryOverlay(accessory, settings.accessory_color, character);
  useOverlaySize(character, settings.pixel_size, settings.max_cards);

  const lastSeenId = useIncomingNotifications(dispatch);
  const [hoveredId, setHoveredId] = useHoveredCard(dispatch);
  useQueueClock(queue, dispatch);
  useTrayAlert(queue);
  const clearHover = useCallback(() => setHoveredId(null), [setHoveredId]);
  useHideWhenEmpty(queue, lastSeenId, clearHover);

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
          character={character}
          overlay={overlay}
          onActivate={() => {
            // Cartão sticky (CI que terminou) só fecha no ×: o clique só abre o link.
            if (!card.n.sticky) dispatch({ type: "dismiss", id: card.n.id, now: Date.now() });
            overlayIpc.cardAction(card.n.url);
          }}
          onDismiss={() => {
            dispatch({ type: "dismiss", id: card.n.id, now: Date.now() });
            overlayIpc.cardAction(null);
          }}
        />
      ))}
    </main>
  );
}
