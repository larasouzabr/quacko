// Grava a versão da release nos arquivos que o build lê. Roda só no CI, sem commit:
// o app embute a versão do Cargo.toml (CARGO_PKG_VERSION) e o instalador usa a do tauri.conf.json.
// Uso: node .github/release/set-version.mjs 0.2.0

import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2] ?? "";
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`versão inválida: "${version}"`);
  process.exit(1);
}

for (const file of ["package.json", "src-tauri/tauri.conf.json"]) {
  const data = JSON.parse(readFileSync(file, "utf8"));
  data.version = version;
  writeFileSync(file, JSON.stringify(data, null, 2) + "\n");
}

for (const file of ["src-tauri/Cargo.toml", "src-tauri/hook/Cargo.toml"]) {
  const text = readFileSync(file, "utf8");
  if (!/^version = "[^"]*"/m.test(text)) {
    console.error(`${file} não tem a linha version`);
    process.exit(1);
  }
  writeFileSync(file, text.replace(/^version = "[^"]*"/m, `version = "${version}"`));
}

console.log(`versão ${version} gravada`);
