import { describe, expect, it } from "vitest";
import { BUILTIN_CHARACTERS, builtinCharacter, DEFAULT_CHARACTER_ID, getCharacter } from "./builtin";
import { buildCharacter } from "./load";
import { MOODS, type CharacterManifest } from "./types";

const sprite = (file: string) => ({ type: "sprite", file, frames: 2, columns: 2, fps: 3, loop: true });

function manifest(overrides: Partial<CharacterManifest> = {}): CharacterManifest {
  return {
    schema: 2,
    id: "teste",
    name: "Teste",
    author: "Alguém",
    license: "CC0-1.0",
    pixel_art: true,
    grid: { width: 28, height: 28 },
    palette: { fur: "#FFFAF5", outline: "#2b2024" },
    bubble: "pixel",
    fallback: "info",
    states: {
      ok: sprite("assets/ok.png"),
      fail: sprite("assets/fail.png"),
      attention: sprite("assets/attention.png"),
      info: sprite("assets/info.png"),
    },
    ...overrides,
  };
}

const resolve = (file: string) => `url:${file}`;

describe("buildCharacter", () => {
  it("lê grade, quadros e fps do manifest (schema 2, pixel art)", () => {
    const c = buildCharacter(manifest(), resolve);
    expect(c.pixelArt).toBe(true);
    expect(c.size).toEqual({ width: 28, height: 28 });
    expect(c.bubble).toBe("pixel");
    expect(c.states.ok).toEqual({ url: "url:assets/ok.png", frames: 2, columns: 2, fps: 3, loop: true });
  });

  it("usa o estado de fallback quando um estado falta", () => {
    const m = manifest();
    delete m.states.attention;
    const c = buildCharacter(m, resolve);
    expect(c.states.attention.url).toBe("url:assets/info.png");
  });

  it("falha quando falta estado e o fallback não existe", () => {
    const m = manifest({ fallback: "nao-existe" });
    delete m.states.fail;
    expect(() => buildCharacter(m, resolve)).toThrow(/fail/);
  });

  it("falha quando o arquivo não está no pacote", () => {
    expect(() => buildCharacter(manifest(), (f) => (f.includes("ok") ? undefined : f))).toThrow(/ok\.png/);
  });

  it("recusa tipos ainda não suportados", () => {
    const m = manifest();
    m.states.ok = { type: "lottie", file: "assets/ok.json" };
    expect(() => buildCharacter(m, resolve)).toThrow(/lottie/);
  });

  it("recusa grade inválida (zero, fracionária ou acima de 128)", () => {
    for (const grid of [{ width: 0, height: 28 }, { width: 28.5, height: 28 }, { width: 129, height: 28 }]) {
      expect(() => buildCharacter(manifest({ grid }), resolve), JSON.stringify(grid)).toThrow(/grid/);
    }
    expect(() => buildCharacter(manifest({ grid: undefined }), resolve)).toThrow(/grid/);
  });

  it("schema 1 (size em px) é lido como não pixel art", () => {
    const c = buildCharacter(
      manifest({ schema: 1, pixel_art: undefined, grid: undefined, size: { width: 168, height: 168 } }),
      resolve,
    );
    expect(c.pixelArt).toBe(false);
    expect(c.size).toEqual({ width: 168, height: 168 });
    expect(c.palette).toEqual({});
  });

  it("schema 2 sem pixel art usa size e ignora a paleta", () => {
    const c = buildCharacter(manifest({ pixel_art: false, size: { width: 160, height: 160 } }), resolve);
    expect(c.pixelArt).toBe(false);
    expect(c.palette).toEqual({});
    expect(() => buildCharacter(manifest({ pixel_art: false, size: { width: 5000, height: 160 } }), resolve)).toThrow(/size/);
  });

  it("schema desconhecido é recusado", () => {
    expect(() => buildCharacter(manifest({ schema: 3 }), resolve)).toThrow(/schema/);
  });

  it("paleta aceita só #rrggbb/#rrggbbaa e nomes simples", () => {
    const c = buildCharacter(
      manifest({
        palette: {
          fur: "#FFFAF5",
          glow: "#11223344",
          bad1: "red",
          bad2: "#fff",
          bad3: "url(x.png)",
          "Bad Name": "#000000",
          num: 123,
        },
      }),
      resolve,
    );
    expect(c.palette).toEqual({ fur: "#fffaf5", glow: "#11223344" });
  });

  it("sugestão de balão fora da lista é ignorada", () => {
    expect(buildCharacter(manifest({ bubble: "neon" }), resolve).bubble).toBeUndefined();
  });

  it("valores numéricos inválidos caem em padrões seguros", () => {
    const m = manifest();
    m.states.ok = { type: "sprite", file: "a.png", frames: -1, columns: 0, fps: 0 };
    expect(buildCharacter(m, resolve).states.ok).toMatchObject({ frames: 1, fps: 1 });
  });

  it("quadros que não cabem numa linha mostram só o primeiro", () => {
    const m = manifest();
    m.states.ok = { type: "sprite", file: "a.png", frames: 8, columns: 4, fps: 12 };
    expect(buildCharacter(m, resolve).states.ok.frames).toBe(1);
  });

  it("imagem estática vira sprite de 1 quadro", () => {
    const m = manifest();
    m.states.info = { type: "image", file: "info.png" };
    expect(buildCharacter(m, resolve).states.info).toMatchObject({ frames: 1, columns: 1 });
  });
});

describe("personagens embutidos", () => {
  it("o pato é o padrão; o gato continua disponível", () => {
    expect(DEFAULT_CHARACTER_ID).toBe("pato-pixel");
    expect(builtinCharacter.id).toBe("pato-pixel");
    expect(Object.keys(BUILTIN_CHARACTERS).sort()).toEqual(["gato-pixel", "pato-pixel"]);
    expect(getCharacter("gato-pixel").id).toBe("gato-pixel");
  });

  it("id desconhecido ou vazio cai no padrão", () => {
    expect(getCharacter("capivara-pixel").id).toBe("pato-pixel");
    expect(getCharacter(undefined).id).toBe("pato-pixel");
  });

  for (const id of ["pato-pixel", "gato-pixel"]) {
    it(`${id}: pixel art em 1x (grade 28x28), 4 estados de 2 quadros a 3 fps`, () => {
      const c = getCharacter(id);
      expect(c.pixelArt).toBe(true);
      expect(c.size).toEqual({ width: 28, height: 28 });
      expect(c.bubble).toBe("pixel");
      for (const mood of MOODS) {
        const s = c.states[mood];
        expect(s.url, mood).toContain(`${id}/assets/${mood}.png`);
        expect([s.frames, s.columns, s.fps], mood).toEqual([2, 2, 3]);
      }
    });
  }

  it("o gato mantém a paleta recolorível", () => {
    expect(Object.keys(getCharacter("gato-pixel").palette).sort()).toEqual(
      ["blush", "fur", "fur_shade", "outline", "patch", "patch_dark", "patch_light"],
    );
  });
});
