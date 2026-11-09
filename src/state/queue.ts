// Fila de cartões do overlay. Reducer puro: o tempo entra como `now` nas ações.
import type { Notification } from "../types";

/** Duração da animação de saída (fade); o cartão sai da fila depois disso. */
export const LEAVE_MS = 250;
/** Ao tirar o mouse, cada cartão ganha pelo menos esse tempo antes de sumir. */
export const RESUME_GRACE_MS = 1500;

export interface Card {
  n: Notification;
  /** Quando expira (só vale enquanto a fila não está pausada). */
  expiresAt: number;
  /** Tempo restante guardado durante a pausa. */
  remainingMs: number;
  /** Quando começou a sair; `null` enquanto visível. */
  leavingAt: number | null;
}

export interface QueueState {
  cards: Card[];
  paused: boolean;
  maxCards: number;
}

export type QueueAction =
  | { type: "add"; notification: Notification; now: number }
  | { type: "dismiss"; id: number; now: number }
  | { type: "tick"; now: number }
  | { type: "pause"; now: number }
  | { type: "resume"; now: number }
  | { type: "configure"; maxCards: number };

export function initialQueue(maxCards = 3): QueueState {
  return { cards: [], paused: false, maxCards };
}

/** Mesma origem + mesmo projeto: o novo substitui o anterior. */
function sameSlot(a: Notification, b: Notification): boolean {
  return a.source === b.source && (a.project ?? "") === (b.project ?? "");
}

export function queueReducer(state: QueueState, action: QueueAction): QueueState {
  switch (action.type) {
    case "add": {
      const { notification: n, now } = action;
      const kept = state.cards.filter((c) => c.leavingAt !== null || !sameSlot(c.n, n));
      const card: Card = { n, expiresAt: now + n.ttl_ms, remainingMs: n.ttl_ms, leavingAt: null };
      let cards = [...kept, card]; // o mais novo vai para o fim da lista

      // Acima do limite: os mais antigos começam a sair.
      // Acima do limite, saem os mais antigos; os "sticky" (só fecham no ×) por último.
      // Se todos forem sticky, o mais antigo cede o lugar: senão o aviso novo não apareceria.
      let excess = cards.filter((c) => c.leavingAt === null).length - state.maxCards;
      for (const stickyRound of [false, true]) {
        cards = cards.map((c) => {
          if (excess > 0 && c.leavingAt === null && Boolean(c.n.sticky) === stickyRound && c !== card) {
            excess -= 1;
            return { ...c, leavingAt: now };
          }
          return c;
        });
      }
      return { ...state, cards };
    }

    case "dismiss":
      return {
        ...state,
        cards: state.cards.map((c) =>
          c.n.id === action.id && c.leavingAt === null ? { ...c, leavingAt: action.now } : c,
        ),
      };

    case "tick": {
      const { now } = action;
      let changed = false;
      const cards: Card[] = [];
      for (const c of state.cards) {
        if (c.leavingAt !== null && now - c.leavingAt >= LEAVE_MS) {
          changed = true; // terminou de sair
          continue;
        }
        if (c.leavingAt === null && !c.n.sticky && !state.paused && now >= c.expiresAt) {
          changed = true;
          cards.push({ ...c, leavingAt: now });
          continue;
        }
        cards.push(c);
      }
      return changed ? { ...state, cards } : state;
    }

    case "pause":
      if (state.paused) return state;
      return {
        ...state,
        paused: true,
        cards: state.cards.map((c) => ({ ...c, remainingMs: Math.max(0, c.expiresAt - action.now) })),
      };

    case "configure": {
      const maxCards = Math.max(1, Math.floor(action.maxCards));
      // Vale a partir do próximo aviso; não derruba cartões que já estão na tela.
      return maxCards === state.maxCards ? state : { ...state, maxCards };
    }

    case "resume":
      if (!state.paused) return state;
      return {
        ...state,
        paused: false,
        cards: state.cards.map((c) => ({
          ...c,
          expiresAt: action.now + Math.max(c.remainingMs, RESUME_GRACE_MS),
        })),
      };
  }
}

/** Cartões que ainda estão na tela (inclui os que estão saindo). */
export function isEmpty(state: QueueState): boolean {
  return state.cards.length === 0;
}
