import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { I18nProvider } from "./i18n/I18nContext";
import { useI18n } from "./hooks/useI18n";
import { installPerfConsole } from "./lib/perf";
import "./index.css";

// Console handle for the opt-in boot/view timings (`__ownhubPerf`).
installPerfConsole();

const container = document.getElementById("root");

if (!container) {
  throw new Error("Root container #root was not found in index.html");
}

/** Top-level crash net: a shell render failure still offers recovery. */
function AppWithRecovery() {
  const { t } = useI18n();
  return (
    <ErrorBoundary
      area="app"
      copy={{
        title: t.common.unexpected,
        message: t.common.viewCrashedHint,
        retry: t.skills.tryAgain,
        reload: t.common.reloadApp,
      }}
    >
      <App />
    </ErrorBoundary>
  );
}

createRoot(container).render(
  <StrictMode>
    <I18nProvider>
      <AppWithRecovery />
    </I18nProvider>
  </StrictMode>,
);
