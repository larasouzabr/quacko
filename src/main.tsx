import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import Overlay from "./overlay/Overlay";
import "./styles.css";
import { tauriCustomizeApi } from "./customize/api";
import Customize from "./customize/Customize";
import { tauriWelcomeApi } from "./welcome/api";
import Welcome from "./welcome/Welcome";

const view =
  location.hash === "#welcome" ? (
    <Welcome api={tauriWelcomeApi} />
  ) : location.hash === "#customize" ? (
    <Customize api={tauriCustomizeApi} />
  ) : (
    <Overlay />
  );

createRoot(document.getElementById("root")!).render(<StrictMode>{view}</StrictMode>);
