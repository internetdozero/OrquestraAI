import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { I18nProvider } from "./i18n/I18nContext";
import "./index.css";

const container = document.getElementById("root");

if (!container) {
  throw new Error("Root container #root was not found in index.html");
}

createRoot(container).render(
  <StrictMode>
    <I18nProvider>
      <App />
    </I18nProvider>
  </StrictMode>,
);
