import assert from "node:assert/strict";
import { test } from "node:test";

import { buildLatest } from "./latest-json.mjs";

// Arquivos no formato do minisign, só com o que o script lê (o id da chave).
const b64 = (data) => Buffer.from(data).toString("base64");
const id = (hex) => Buffer.from(hex, "hex").reverse();
const pubkey = (hex) =>
  b64(`untrusted comment: minisign public key: ${hex}\n${b64(Buffer.concat([Buffer.from("Ed"), id(hex), Buffer.alloc(32)]))}\n`);
const sig = (hex, trusted = "timestamp:1700000000\tfile:x") =>
  b64(
    `untrusted comment: signature from tauri secret key\n${b64(Buffer.concat([Buffer.from("ED"), id(hex), Buffer.alloc(64)]))}\ntrusted comment: ${trusted}\n${b64(Buffer.alloc(64))}`,
  );

const KEY = "948A077B4DCF674F";
const base = {
  version: "0.4.0",
  tag: "v0.4.0",
  repo: "larasouzabr/quacko",
  pubkey: pubkey(KEY),
  date: "2026-11-14T00:00:00.000Z",
};
const sigs = (key = KEY, trusted) => [
  { name: "Quacko_0.4.0_universal.app.tar.gz.sig", sig: sig(key, trusted) },
  { name: "Quacko_0.4.0_x64-setup.exe.sig", sig: sig(key, trusted) },
];

test("monta o latest.json com mac universal e windows", () => {
  const latest = buildLatest({ ...base, sigs: sigs() });
  assert.equal(latest.version, "0.4.0");
  assert.equal(latest.platforms["darwin-aarch64"], latest.platforms["darwin-x86_64"]);
  assert.equal(
    latest.platforms["darwin-aarch64"].url,
    "https://github.com/larasouzabr/quacko/releases/download/v0.4.0/Quacko_0.4.0_universal.app.tar.gz",
  );
  assert.equal(
    latest.platforms["windows-x86_64"].url,
    "https://github.com/larasouzabr/quacko/releases/download/v0.4.0/Quacko_0.4.0_x64-setup.exe",
  );
  assert.equal(latest.platforms["windows-x86_64"].signature, sigs()[1].sig);
});

test("recusa assinatura de outra chave", () => {
  assert.throws(() => buildLatest({ ...base, sigs: sigs("0123456789ABCDEF") }), /chave 0123456789ABCDEF.*948A077B4DCF674F/);
});

test("recusa assinatura feita para outra versão", () => {
  assert.throws(
    () => buildLatest({ ...base, sigs: sigs(KEY, "timestamp:1\tfile:x\tversion:0.3.0") }),
    /versão 0\.3\.0/,
  );
  assert.ok(buildLatest({ ...base, sigs: sigs(KEY, "timestamp:1\tfile:x\tversion:0.4.0") }));
});

test("exige exatamente uma assinatura por plataforma", () => {
  assert.throws(() => buildLatest({ ...base, sigs: sigs().slice(0, 1) }), /-setup\.exe\.sig, achei 0/);
  assert.throws(() => buildLatest({ ...base, sigs: [...sigs(), sigs()[0]] }), /app\.tar\.gz\.sig, achei 2/);
});
