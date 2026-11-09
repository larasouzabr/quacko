import type { Notification } from "../types";

export const LEAVE_MS = 250;
export const RESUME_GRACE_MS = 1500;

export interface Card {
  n: Notification;
  expiresAt: number;
  remainingMs: number;
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

function sameSlot(a: Notification, b: Notification): boolean {
  return a.source === b.source && (a.project ?? "") === (b.project ?? "");
}

export function queueReducer(state: QueueState, action: QueueAction): QueueState {
  switch (action.type) {
    case "add": {
      const { notification: n, now } = action;
      const kept = state.cards.filter((c) => c.leavingAt !== null || !sameSlot(c.n, n));
      const card: Card = { n, expiresAt: now + n.ttl_ms, remainingMs: n.ttl_ms, leavingAt: null };
      let cards = [...kept, card];

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
          changed = true;
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

export function isAlert(n: Notification): boolean {
  return n.status === "attention" || (n.status === "fail" && n.source === "ci");
}

export function hasVisibleAlert(state: QueueState): boolean {
  return state.cards.some((c) => c.leavingAt === null && isAlert(c.n));
}

export function isEmpty(state: QueueState): boolean {
  return state.cards.length === 0;
}
