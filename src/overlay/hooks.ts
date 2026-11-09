import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState, type Dispatch, type RefObject } from "react";
import { hasVisibleAlert, isEmpty, type QueueAction, type QueueState } from "../state/queue";
import type { Notification } from "../types";
import { overlayIpc } from "./ipc";

const TICK_MS = 100;

type Pointer = [number, number] | null;

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

export function useQueueClock(queue: QueueState, dispatch: Dispatch<QueueAction>) {
  const hasCards = !isEmpty(queue);
  useEffect(() => {
    if (!hasCards) return;
    const timer = setInterval(() => dispatch({ type: "tick", now: Date.now() }), TICK_MS);
    return () => clearInterval(timer);
  }, [hasCards, dispatch]);
}

export function useTrayAlert(queue: QueueState) {
  const alertVisible = hasVisibleAlert(queue);
  const hadAlert = useRef(false);
  useEffect(() => {
    if (hadAlert.current && !alertVisible) overlayIpc.alertCleared();
    hadAlert.current = alertVisible;
  }, [alertVisible]);
}

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
