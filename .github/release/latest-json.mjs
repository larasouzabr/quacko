// Monta o latest.json que o app consulta para se atualizar sozinho e anexa ao rascunho
// da release, antes de publicar. Usa os .sig que o tauri-action subiu nos jobs de build.
// Recusa publicar se uma assinatura não for da chave pública embutida no app
// (src-tauri/tauri.conf.json), porque aí ninguém conseguiria atualizar.
// Uso no CI: node .github/release/latest-json.mjs <id do rascunho> <tag>
// (precisa de GH_TOKEN e GITHUB_REPOSITORY)

import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

// Chave e assinatura do minisign: base64 de um texto cuja segunda linha é base64 de
// [algoritmo (2 bytes)][id da chave (8 bytes)][...]. Basta comparar o id.
function keyId(minisignText, what) {
  const text = Buffer.from(minisignText.trim(), "base64").toString("utf8");
  const line = text.split("\n")[1] ?? "";
  const bytes = Buffer.from(line.trim(), "base64");
  if (bytes.length < 10) throw new Error(`${what}: formato inesperado`);
  // Mesma ordem que o minisign mostra no comentário do .pub.
  return Buffer.from(bytes.subarray(2, 10)).reverse().toString("hex").toUpperCase();
}

// A CLI do Tauri pode gravar a versão no comentário assinado ("version:1.2.3").
function signedVersion(sig) {
  const text = Buffer.from(sig.trim(), "base64").toString("utf8");
  const trusted = text.split("\n").find((l) => l.startsWith("trusted comment: "));
  const field = trusted
    ?.slice("trusted comment: ".length)
    .split("\t")
    .find((f) => f.startsWith("version:"));
  return field?.slice("version:".length);
}

export function buildLatest({ version, tag, repo, pubkey, sigs, date }) {
  const expected = keyId(pubkey, "chave pública do tauri.conf.json");
  const pick = (suffix) => {
    const found = sigs.filter((s) => s.name.endsWith(`${suffix}.sig`));
    if (found.length !== 1) {
      throw new Error(`esperava 1 assinatura *${suffix}.sig, achei ${found.length}`);
    }
    const { name, sig } = found[0];
    const id = keyId(sig, name);
    if (id !== expected) {
      throw new Error(`${name} foi assinado com a chave ${id}, mas o app confia na ${expected}`);
    }
    const signed = signedVersion(sig);
    if (signed !== undefined && signed !== version) {
      throw new Error(`${name} foi assinado para a versão ${signed}, não ${version}`);
    }
    const file = name.slice(0, -".sig".length);
    return {
      signature: sig.trim(),
      url: `https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(file)}`,
    };
  };
  const mac = pick(".app.tar.gz");
  const windows = pick("-setup.exe");
  return {
    version,
    pub_date: date,
    platforms: {
      "darwin-aarch64": mac,
      "darwin-x86_64": mac,
      "windows-x86_64": windows,
    },
  };
}

async function main() {
  const [releaseId, tag] = process.argv.slice(2);
  const repo = process.env.GITHUB_REPOSITORY;
  const token = process.env.GH_TOKEN;
  if (!/^\d+$/.test(releaseId ?? "") || !/^v\d+\.\d+\.\d+$/.test(tag ?? "") || !repo || !token) {
    throw new Error("uso: latest-json.mjs <id do rascunho> <tag>, com GH_TOKEN e GITHUB_REPOSITORY");
  }
  const api = `https://api.github.com/repos/${repo}/releases`;
  const headers = { Authorization: `Bearer ${token}`, "X-GitHub-Api-Version": "2022-11-28" };
  const call = async (url, init = {}) => {
    const res = await fetch(url, { ...init, headers: { ...headers, ...init.headers } });
    if (!res.ok) throw new Error(`${init.method ?? "GET"} ${url}: ${res.status} ${await res.text()}`);
    return res;
  };

  const assets = await (await call(`${api}/${releaseId}/assets?per_page=100`)).json();
  const sigs = [];
  for (const asset of assets.filter((a) => a.name.endsWith(".sig"))) {
    const res = await call(`${api}/assets/${asset.id}`, {
      headers: { Accept: "application/octet-stream" },
    });
    sigs.push({ name: asset.name, sig: await res.text() });
  }

  const pubkey = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8")).plugins.updater.pubkey;
  const latest = buildLatest({
    version: tag.slice(1),
    tag,
    repo,
    pubkey,
    sigs,
    date: new Date().toISOString(),
  });

  for (const old of assets.filter((a) => a.name === "latest.json")) {
    await call(`${api}/assets/${old.id}`, { method: "DELETE" });
  }
  await call(`https://uploads.github.com/repos/${repo}/releases/${releaseId}/assets?name=latest.json`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(latest, null, 2),
  });
  console.log(JSON.stringify(latest.platforms, null, 2));
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  main().catch((e) => {
    console.error(`::error::${e.message}`);
    process.exit(1);
  });
}
