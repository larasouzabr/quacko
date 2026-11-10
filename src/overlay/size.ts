import { useEffect } from "react";
import type { Character } from "../character/types";
import { characterBox } from "../components/Cat";
import { useDevicePixelRatio } from "../hooks/useDevicePixelRatio";
import { overlayIpc } from "./ipc";

const STACK_PAD_X = 16;
const STACK_PAD_Y = 14;
const CARD_GAP = 8;
const CAT_BUBBLE_GAP = 2;
const BUBBLE_WIDTH = 278;
const BUBBLE_MAX_HEIGHT = 148;

export function overlaySize(cat: { width: number; height: number }, maxCards: number, textScale = 100) {
  const scale = Math.min(Math.max(textScale, 100), 150) / 100;
  const cardHeight = Math.max(cat.height, BUBBLE_MAX_HEIGHT * scale);
  return {
    width: Math.ceil(STACK_PAD_X + cat.width + CAT_BUBBLE_GAP + BUBBLE_WIDTH * scale),
    height: Math.ceil(STACK_PAD_Y + maxCards * cardHeight + (maxCards - 1) * CARD_GAP),
  };
}

export function useOverlaySize(character: Character, pixelSize: number, maxCards: number, textScale = 100) {
  const dpr = useDevicePixelRatio();
  const size = overlaySize(characterBox(character, pixelSize, 1, dpr), maxCards, textScale);
  useEffect(() => {
    overlayIpc.setSize(size.width, size.height);
  }, [size.width, size.height]);
}
