import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState, type Dispatch, type RefObject } from "react";
import { hasVisibleAlert, isEmpty, type QueueAction, type QueueState } from "../state/queue";
import type { Notification } from "../types";
import { overlayIpc } from "./ipc";

const TICK_MS = 100;

/** Posição do mouse dentro do overlay (px CSS), ou null fora dele. Enviada pelo Rust,
 * porque o WebKit não entrega eventos de hover numa janela que não está em foco. */
type Pointer = [number, number] | null;

/** Avisos vindos do Rust; devolve o maior id já visto (para esconder a janela sem perder aviso). */
export function useIncomingNotifications(dispatch: Dispatch<QueueAction>): RefObject<number> {
  const lastSeenId = useRef(0);
  useEffect(() => {
    const unlisten = listen<Notification>("notify", ({ payload }) => {
      lastSeenId.current = Math.max(lastSeenId.current, payload.id);
      dispatch({ type: "add", notification: payload, now: Date.now() });
    });
    return () => void unlisten.then((off) => off());
  }, [dispatch]);
  return lastSeenId;
}

/** Cartão embaixo do mouse. Sobre um cartão: pausa os timers e a janela recebe cliques;
 * fora deles, os cliques passam para o app de baixo. */
export function useHoveredCard(dispatch: Dispatch<QueueAction>): [number | null, (id: number | null) => void] {
  const [hoveredId, setHoveredId] = useState<number | null>(null);
  useEffect(() => {
    const unlisten = listen<Pointer>("overlay-pointer", ({ payload }) => {
      const el = payload ? document.elementFromPoint(payload[0], payload[1]) : null;
      const card = el?.closest<HTMLElement>("[data-card-id]");
      setHoveredId(card ? Number(card.dataset.cardId) : null);
    });
    return () => void unlisten.then((off) => off());
  }, []);
  const overCard = hoveredId !== null;
  useEffect(() => {
    dispatch({ type: overCard ? "pause" : "resume", now: Date.now() });
    overlayIpc.setInteractive(overCard);
  }, [overCard, dispatch]);
  return [hoveredId, setHoveredId];
}

/** Relógio da fila (expiração e fim do fade), só enquanto há cartões. */
export function useQueueClock(queue: QueueState, dispatch: Dispatch<QueueAction>) {
  const hasCards = !isEmpty(queue);
  useEffect(() => {
    if (!hasCards) return;
    const timer = setInterval(() => dispatch({ type: "tick", now: Date.now() }), TICK_MS);
    return () => clearInterval(timer);
  }, [hasCards, dispatch]);
}

/** Os avisos de alerta sumiram da tela: o ícone da bandeja volta ao normal. */
export function useTrayAlert(queue: QueueState) {
  const alertVisible = hasVisibleAlert(queue);
  const hadAlert = useRef(false);
  useEffect(() => {
    if (hadAlert.current && !alertVisible) overlayIpc.alertCleared();
    hadAlert.current = alertVisible;
  }, [alertVisible]);
}

/** Fila vazia: pede ao Rust para esconder a janela. */
export function useHideWhenEmpty(queue: QueueState, lastSeenId: RefObject<number>, onHidden: () => void) {
  const hasCards = !isEmpty(queue);
  const shownSomething = useRef(false);
  useEffect(() => {
    if (hasCards) {
      shownSomething.current = true;
      return;
    }
    if (!shownSomething.current) return;
    shownSomething.current = false;
    onHidden();
    overlayIpc.empty(lastSeenId.current ?? 0);
  }, [hasCards, lastSeenId, onHidden]);
}
