import { useEffect, useRef, useState, useCallback } from "react";
import init, { Simulator } from "sim-wasm";
import wasmUrl from "sim-wasm/sim_wasm_bg.wasm?url";

export default function App() {
  const simRef = useRef<Simulator | null>(null);
  const [ready, setReady] = useState(false);
  const [position, setPosition] = useState(0);
  const [depth, setDepth] = useState(1.0);
  const [stroke, setStroke] = useState(1.0);
  const [velocity, setVelocity] = useState(0.5);
  const [sensation, setSensation] = useState(0);

  useEffect(() => {
    let cancelled = false;
    init(wasmUrl).then(() => {
      if (cancelled) return;
      simRef.current = new Simulator(10.0);
      setReady(true);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!ready) return;
    let raf: number;
    function animate() {
      const sim = simRef.current;
      if (sim) setPosition(sim.get_position());
      raf = requestAnimationFrame(animate);
    }
    raf = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(raf);
  }, [ready]);

  const updateDepth = useCallback((v: number) => {
    setDepth(v);
    simRef.current?.set_depth(v);
  }, []);

  const updateStroke = useCallback((v: number) => {
    setStroke(v);
    simRef.current?.set_stroke(v);
  }, []);

  const updateVelocity = useCallback((v: number) => {
    setVelocity(v);
    simRef.current?.set_velocity(v);
  }, []);

  const updateSensation = useCallback((v: number) => {
    setSensation(v);
    simRef.current?.set_sensation(v);
  }, []);

  if (!ready) return <p>Loading simulator…</p>;

  return (
    <div
      style={{ fontFamily: "system-ui", maxWidth: 480, margin: "2rem auto" }}
    >
      <h1 style={{ fontSize: "1.25rem" }}>OSSM Simulator</h1>

      <div
        style={{
          height: 32,
          background: "#eee",
          borderRadius: 4,
          marginBottom: "1.5rem",
          position: "relative",
          overflow: "hidden",
        }}
      >
        <div
          style={{
            width: `${position * 100}%`,
            height: "100%",
            background: "#3b82f6",
            transition: "width 16ms linear",
          }}
        />
        <span
          style={{
            position: "absolute",
            top: "50%",
            left: "50%",
            transform: "translate(-50%, -50%)",
            fontSize: "0.875rem",
            fontWeight: 600,
          }}
        >
          {position.toFixed(3)}
        </span>
      </div>

      <Slider
        label="Depth"
        value={depth}
        min={0}
        max={1}
        step={0.01}
        onChange={updateDepth}
      />
      <Slider
        label="Stroke"
        value={stroke}
        min={0}
        max={1}
        step={0.01}
        onChange={updateStroke}
      />
      <Slider
        label="Velocity"
        value={velocity}
        min={0}
        max={1}
        step={0.01}
        onChange={updateVelocity}
      />
      <Slider
        label="Sensation"
        value={sensation}
        min={-100}
        max={100}
        step={1}
        onChange={updateSensation}
      />
    </div>
  );
}

function Slider({
  label,
  value,
  min,
  max,
  step,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (v: number) => void;
}) {
  return (
    <div style={{ marginBottom: "1rem" }}>
      <label
        style={{
          display: "flex",
          justifyContent: "space-between",
          fontSize: "0.875rem",
        }}
      >
        <span>{label}</span>
        <span>{value}</span>
      </label>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        style={{ width: "100%" }}
      />
    </div>
  );
}
