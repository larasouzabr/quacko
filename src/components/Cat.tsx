import { useEffect, useLayoutEffect, useRef, type CSSProperties } from "react";
import { builtinCharacter } from "../character/builtin";
import { cssPixel, DEFAULT_PIXEL_SIZE, snapOffset } from "../character/pixel";
import type { Character, Mood } from "../character/types";
import { useDevicePixelRatio } from "../hooks/useDevicePixelRatio";

interface CatProps {
  mood: Mood;
  character?: Character;
  pixelSize?: number;
  scale?: number;
  overlay?: Partial<Record<Mood, string>>;
}

export function characterBox(
  character: Character,
  pixelSize: number,
  scale: number,
  dpr: number,
): { width: number; height: number } {
  if (character.pixelArt) {
    const unit = cssPixel(pixelSize, dpr);
    return { width: character.size.width * unit, height: character.size.height * unit };
  }
  return {
    width: Math.round(character.size.width * scale),
    height: Math.round(character.size.height * scale),
  };
}

export default function Cat({
  mood,
  character = builtinCharacter,
  pixelSize = DEFAULT_PIXEL_SIZE,
  scale = 1,
  overlay,
}: CatProps) {
  const dpr = useDevicePixelRatio();
  const ref = useRef<HTMLDivElement>(null);
  const state = character.states[mood];
  const { width, height } = characterBox(character, pixelSize, scale, dpr);
  const rows = Math.ceil(state.frames / state.columns);

  useLayoutEffect(() => {
    if (character.pixelArt) snapToDevicePixels(ref.current, dpr);
  });
  useEffect(() => {
    if (!character.pixelArt) return;
    const resnap = () => snapToDevicePixels(ref.current, dpr);
    document.addEventListener("animationend", resnap);
    window.addEventListener("resize", resnap);
    return () => {
      document.removeEventListener("animationend", resnap);
      window.removeEventListener("resize", resnap);
    };
  }, [character.pixelArt, dpr]);

  const style = {
    width,
    height,
    backgroundImage: [overlay?.[mood], state.url]
      .filter(Boolean)
      .map((url) => `url("${url}")`)
      .join(", "),
    backgroundSize: `${state.columns * width}px ${rows * height}px`,
    "--cat-strip": `${-state.frames * width}px`,
    "--cat-frames": state.frames,
    "--cat-duration": `${state.frames / state.fps}s`,
    "--cat-iterations": state.loop ? "infinite" : "1",
  } as CSSProperties;

  const classes = ["cat-sprite"];
  if (state.frames > 1) classes.push("is-animated");
  if (character.pixelArt) classes.push("is-pixel-art");

  return <div ref={ref} className={classes.join(" ")} data-mood={mood} style={style} aria-hidden="true" />;
}

function snapToDevicePixels(el: HTMLDivElement | null, dpr: number) {
  if (!el) return;
  el.style.translate = "";
  const rect = el.getBoundingClientRect();
  const dx = snapOffset(rect.left, dpr);
  const dy = snapOffset(rect.top, dpr);
  if (dx !== 0 || dy !== 0) el.style.translate = `${dx}px ${dy}px`;
}
