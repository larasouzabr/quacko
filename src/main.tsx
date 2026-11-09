import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles.css";
import { tauriWelcomeApi } from "./welcome/api";
import Welcome from "./welcome/Welcome";

// Mesmo bundle para as duas janelas: o overlay (index.html) e as boas-vindas (#welcome).
const view = location.hash === "#welcome" ? <Welcome api={tauriWelcomeApi} /> : <App />;

createRoot(document.getElementById("root")!).render(<StrictMode>{view}</StrictMode>);
