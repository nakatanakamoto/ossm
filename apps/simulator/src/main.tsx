import { createRoot } from "react-dom/client";
import { BrowserRouter, Routes, Route } from "react-router";
import "@radix-ui/themes/styles.css";
import { AppearanceProvider } from "./AppearanceProvider";
import { SimulatorProvider } from "./SimulatorProvider";
import App from "./App";
import { DevicePage } from "./pages/DevicePage";

createRoot(document.getElementById("root")!).render(
  <AppearanceProvider>
    <BrowserRouter>
      <Routes>
        <Route
          path="/"
          element={
            <SimulatorProvider fallback={<p>Loading simulator…</p>}>
              <App />
            </SimulatorProvider>
          }
        />
        <Route path="/device" element={<DevicePage />} />
      </Routes>
    </BrowserRouter>
  </AppearanceProvider>,
);
