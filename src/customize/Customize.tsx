import { useEffect, useState } from "react";
import { BUILTIN_ACCESSORIES, DEFAULT_ACCESSORY_ID } from "../accessory/builtin";
import { PRESETS, rgbToHex } from "../accessory/recolor";
import { useAccessoryOverlay } from "../accessory/useAccessory";
import { BUILTIN_CHARACTERS, getCharacter } from "../character/builtin";
import { MOODS } from "../character/types";
import Cat from "../components/Cat";
import type { Lang } from "../i18n";
import type { CustomizeApi } from "./api";
import { customizeTexts } from "./texts";

export default function Customize({ api }: { api: CustomizeApi }) {
  const [lang, setLang] = useState<Lang>("pt-BR");
  const [character, setCharacter] = useState("pato-pixel");
  const [enabled, setEnabled] = useState(false);
  const [color, setColor] = useState("#ff6996");
  const [saved, setSaved] = useState(false);
  const t = customizeTexts(lang);

  useEffect(() => {
    document.body.classList.add("welcome-page");
    api.settings().then(
      (s) => {
        setLang(s.lang);
        setCharacter(s.character);
        setEnabled(s.accessory_enabled);
        setColor(s.accessory_color);
      },
      (e) => console.warn(e),
    );
  }, [api]);

  const save = (next: { character?: string; enabled?: boolean; color?: string }) => {
    const appearance = {
      character: next.character ?? character,
      accessoryEnabled: next.enabled ?? enabled,
      accessoryColor: next.color ?? color,
    };
    setCharacter(appearance.character);
    setEnabled(appearance.accessoryEnabled);
    setColor(appearance.accessoryColor);
    api.save(appearance).then(
      () => setSaved(true),
      (e) => console.warn(e),
    );
  };

  const current = getCharacter(character);
  const accessory = enabled ? BUILTIN_ACCESSORIES[DEFAULT_ACCESSORY_ID] : undefined;
  const overlay = useAccessoryOverlay(accessory, color, current);
  const bowName = BUILTIN_ACCESSORIES[DEFAULT_ACCESSORY_ID]?.name[lang] ?? t.bow;

  return (
    <main className="welcome customize">
      <h1>{t.title}</h1>

      <section className="step">
        <h2>{t.character}</h2>
        <div className="choices">
          {Object.values(BUILTIN_CHARACTERS).map((c) => (
            <button
              key={c.id}
              className={c.id === current.id ? "choice is-selected" : "choice"}
              onClick={() => save({ character: c.id })}
              aria-pressed={c.id === current.id}
            >
              <Cat mood="info" character={c} pixelSize={2} />
              <span>{c.name}</span>
            </button>
          ))}
        </div>
      </section>

      <section className="step">
        <h2>{t.accessories}</h2>
        <label className="toggle">
          <input type="checkbox" checked={enabled} onChange={(e) => save({ enabled: e.target.checked })} />
          <span>
            {bowName}: {enabled ? t.on : t.off}
          </span>
        </label>
        <div className={enabled ? "colors" : "colors is-disabled"}>
          <span className="label">{t.color}</span>
          {Object.entries(PRESETS).map(([name, palette]) => {
            const hex = rgbToHex(palette.main);
            return (
              <button
                key={name}
                className={hex === color.toLowerCase() ? "swatch is-selected" : "swatch"}
                style={{ background: hex }}
                title={t.presets[name] ?? name}
                aria-label={t.presets[name] ?? name}
                disabled={!enabled}
                onClick={() => save({ color: hex })}
              />
            );
          })}
          <label className="custom-color" title={t.custom}>
            <input type="color" value={color} disabled={!enabled} onChange={(e) => save({ color: e.target.value })} />
            <span>{t.custom}</span>
          </label>
        </div>
        <p className="hint">{t.hint}</p>
      </section>

      <section className="step">
        <h2>{t.preview}</h2>
        <div className="preview-row">
          {MOODS.map((mood) => (
            <figure key={mood}>
              <Cat mood={mood} character={current} pixelSize={3} overlay={overlay} />
              <figcaption>{t.moods[mood]}</figcaption>
            </figure>
          ))}
        </div>
      </section>

      {saved && <p className="hint">{t.saved}</p>}
    </main>
  );
}
