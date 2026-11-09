import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles.css";
import { tauriCustomizeApi } from "./customize/api";
import Customize from "./customize/Customize";
import { tauriWelcomeApi } from "./welcome/api";
import Welcome from "./welcome/Welcome";

// Mesmo bundle para as três janelas: overlay (index.html), boas-vindas (#welcome) e
// Personalizar (#customize).
const view =
  location.hash === "#welcome" ? (
    <Welcome api={tauriWelcomeApi} />
  ) : location.hash === "#customize" ? (
    <Customize api={tauriCustomizeApi} />
  ) : (
    <App />
  );

createRoot(document.getElementById("root")!).render(<StrictMode>{view}</StrictMode>);
