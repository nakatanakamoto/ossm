import { createRoot } from "react-dom/client";
import { BrowserRouter, Routes, Route } from "react-router";
import "@radix-ui/themes/styles.css";
import "./global.css";
import { AppearanceProvider } from "./AppearanceProvider";
import { SimulatorProvider } from "./SimulatorProvider";
import Layout from "./Layout";
import FlasherPage from "./pages/FlasherPage";
import SimulatorPage from "./pages/SimulatorPage";

createRoot(document.getElementById("root")!).render(
  <BrowserRouter>
    <AppearanceProvider>
      <SimulatorProvider fallback={<p>Loading simulator…</p>}>
        <Routes>
          <Route element={<Layout />}>
            <Route index element={<SimulatorPage />} />
            <Route path="firmware" element={<FlasherPage />}>
              <Route path="release/:release/:board" element={null} />
              <Route path="release/:release" element={null} />
              <Route path="release" element={null} />
              <Route path="pr/:pr/:board" element={null} />
              <Route path="pr/:pr" element={null} />
            </Route>
            <Route path="flasher" element={<FlasherPage />} />
          </Route>
        </Routes>
      </SimulatorProvider>
    </AppearanceProvider>
  </BrowserRouter>,
);
