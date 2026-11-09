import { useEffect, useState } from "react";

export function useDevicePixelRatio(): number {
  const [dpr, setDpr] = useState(() => window.devicePixelRatio || 1);
  useEffect(() => {
    let query: MediaQueryList | undefined;
    const watch = () => {
      const current = window.devicePixelRatio || 1;
      setDpr(current);
      query?.removeEventListener("change", watch);
      query = window.matchMedia(`(resolution: ${current}dppx)`);
      query.addEventListener("change", watch);
    };
    watch();
    return () => query?.removeEventListener("change", watch);
  }, []);
  return dpr;
}
