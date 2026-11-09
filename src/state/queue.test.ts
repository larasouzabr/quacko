import { describe, expect, it } from "vitest";
import type { Notification } from "../types";
import { initialQueue, isEmpty, LEAVE_MS, queueReducer, RESUME_GRACE_MS, type QueueState } from "./queue";

let nextId = 1;
function notif(partial: Partial<Notification> = {}): Notification {
  return {
    id: nextId++,
    source: "claude",
    status: "ok",
    message: null,
    title: null,
    project: `p${nextId}`,
    url: null,
    ttl_ms: 8000,
    ...partial,
  };
}

function add(state: QueueState, n: Notification, now = 0) {
  return queueReducer(state, { type: "add", notification: n, now });
}

const visible = (s: QueueState) => s.cards.filter((c) => c.leavingAt === null).map((c) => c.n.id);

describe("fila de cartões", () => {
  it("o mais novo vai para o fim da lista", () => {
    const a = notif();
    const b = notif();
    const s = add(add(initialQueue(), a), b);
    expect(visible(s)).toEqual([a.id, b.id]);
  });

  it("limite de 3: o mais antigo começa a sair", () => {
    const ns = [notif(), notif(), notif(), notif()];
    let s = initialQueue(3);
    for (const n of ns) s = add(s, n, 100);
    expect(visible(s)).toEqual([ns[1].id, ns[2].id, ns[3].id]);
    expect(s.cards[0]).toMatchObject({ n: { id: ns[0].id }, leavingAt: 100 });
    s = queueReducer(s, { type: "tick", now: 100 + LEAVE_MS });
    expect(s.cards.map((c) => c.n.id)).toEqual([ns[1].id, ns[2].id, ns[3].id]);
  });

  it("mesmo source + project substitui em vez de empilhar", () => {
    const a = notif({ source: "ci", project: "repo", message: "1" });
    const b = notif({ source: "claude", project: "repo" });
    const c = notif({ source: "ci", project: "repo", message: "2" });
    const s = add(add(add(initialQueue(), a), b), c);
    expect(s.cards.map((x) => x.n.id)).toEqual([b.id, c.id]);
  });

  it("projeto ausente conta como o mesmo lugar", () => {
    const a = notif({ project: null });
    const b = notif({ project: null });
    const s = add(add(initialQueue(), a), b);
    expect(s.cards.map((x) => x.n.id)).toEqual([b.id]);
  });

  it("expira pelo ttl e depois sai da fila", () => {
    let s = add(initialQueue(), notif({ ttl_ms: 2000 }), 0);
    s = queueReducer(s, { type: "tick", now: 1999 });
    expect(visible(s)).toHaveLength(1);
    s = queueReducer(s, { type: "tick", now: 2000 });
    expect(visible(s)).toHaveLength(0);
    expect(isEmpty(s)).toBe(false); // ainda fazendo o fade
    s = queueReducer(s, { type: "tick", now: 2000 + LEAVE_MS });
    expect(isEmpty(s)).toBe(true);
  });

  it("cada cartão expira no seu tempo", () => {
    const a = notif({ ttl_ms: 2000 });
    const b = notif({ ttl_ms: 5000 });
    let s = add(add(initialQueue(), a, 0), b, 0);
    s = queueReducer(s, { type: "tick", now: 3000 });
    expect(visible(s)).toEqual([b.id]);
  });

  it("dispensar inicia a saída só daquele cartão", () => {
    const a = notif();
    const b = notif();
    let s = add(add(initialQueue(), a), b);
    s = queueReducer(s, { type: "dismiss", id: a.id, now: 10 });
    expect(visible(s)).toEqual([b.id]);
    s = queueReducer(s, { type: "tick", now: 10 + LEAVE_MS });
    expect(s.cards.map((c) => c.n.id)).toEqual([b.id]);
  });

  it("pausa segura o timer e retomar devolve o tempo restante", () => {
    let s = add(initialQueue(), notif({ ttl_ms: 8000 }), 0);
    s = queueReducer(s, { type: "pause", now: 3000 });
    s = queueReducer(s, { type: "tick", now: 60_000 });
    expect(visible(s)).toHaveLength(1);
    s = queueReducer(s, { type: "resume", now: 60_000 });
    s = queueReducer(s, { type: "tick", now: 64_999 });
    expect(visible(s)).toHaveLength(1);
    s = queueReducer(s, { type: "tick", now: 65_000 });
    expect(visible(s)).toHaveLength(0);
  });

  it("ao retomar, cartão quase vencido ganha um tempo mínimo", () => {
    let s = add(initialQueue(), notif({ ttl_ms: 2000 }), 0);
    s = queueReducer(s, { type: "pause", now: 1900 });
    s = queueReducer(s, { type: "resume", now: 5000 });
    expect(s.cards[0].expiresAt).toBe(5000 + RESUME_GRACE_MS);
  });

  it("cartão novo durante a pausa também fica parado", () => {
    let s = queueReducer(initialQueue(), { type: "pause", now: 0 });
    s = add(s, notif({ ttl_ms: 2000 }), 0);
    s = queueReducer(s, { type: "tick", now: 10_000 });
    expect(visible(s)).toHaveLength(1);
  });

  it("cartões saindo terminam o fade mesmo com a fila pausada", () => {
    const a = notif();
    let s = add(initialQueue(), a, 0);
    s = queueReducer(s, { type: "dismiss", id: a.id, now: 0 });
    s = queueReducer(s, { type: "pause", now: 0 });
    s = queueReducer(s, { type: "tick", now: LEAVE_MS });
    expect(isEmpty(s)).toBe(true);
  });

  it("limite configurável vale a partir do próximo aviso", () => {
    let s = initialQueue(3);
    s = queueReducer(s, { type: "configure", maxCards: 1 });
    const a = notif();
    const b = notif();
    s = add(add(s, a), b);
    expect(visible(s)).toEqual([b.id]);
  });

  it("limite inválido vira no mínimo 1", () => {
    expect(queueReducer(initialQueue(3), { type: "configure", maxCards: 0 }).maxCards).toBe(1);
  });

  it("tick sem mudanças devolve o mesmo estado (evita re-render)", () => {
    const s = add(initialQueue(), notif(), 0);
    expect(queueReducer(s, { type: "tick", now: 1 })).toBe(s);
  });
});
