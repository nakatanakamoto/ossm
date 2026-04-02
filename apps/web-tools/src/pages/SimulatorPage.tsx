import { Suspense, useCallback, useEffect, useRef, useState } from "react";
import { useSimulator } from "../hooks/useSimulator";
import { useEngineState } from "../hooks/useEngineState";
import { useIsMobile } from "../hooks/useIsMobile";
import { useAppearance } from "../hooks/useAppearance";
import { usePersistedState } from "../hooks/usePersistedState";
import { Box, Button, Card, Flex } from "@radix-ui/themes";
import { PlayIcon, PauseIcon, ReloadIcon } from "@radix-ui/react-icons";
import Scene from "../Scene";
import type { SceneHandle } from "../Scene";
import { Chart } from "../Chart";
import {
  TrajectorySidebar,
  useTrajectoryData,
  DEFAULTS,
  type UnitMode,
} from "../TrajectoryPanel";

const glassStyle = {
  pointerEvents: "auto" as const,
  backdropFilter: "blur(12px)",
  backgroundColor: "var(--color-panel-translucent)",
};

export default function SimulatorPage() {
  const simulator = useSimulator();
  const sceneRef = useRef<SceneHandle>(null);
  const isMobile = useIsMobile();
  const [appearance] = useAppearance();
  const playbackState = useEngineState(simulator);

  const [pattern, setPattern] = usePersistedState("ossm:pattern", 0);
  const [depth, setDepth] = usePersistedState("ossm:depth", 0.75);
  const [stroke, setStroke] = usePersistedState("ossm:stroke", 0.5);
  const [velocity, setVelocity] = usePersistedState("ossm:velocity", 0.75);
  const [sensation, setSensation] = usePersistedState("ossm:sensation", 0.0);
  const [timestep, setTimestep] = usePersistedState("ossm:timestep", 20);
  const [duration, setDuration] = usePersistedState("ossm:duration", 20);
  const [unitMode, setUnitMode] = usePersistedState<UnitMode>("ossm:unitMode", "relative");
  const [wasPlaying, setWasPlaying] = usePersistedState("ossm:playing", false);
  const [focused, setFocused] = useState("position");

  const canvasRef = useRef<HTMLDivElement>(null);
  const viewportRef = useRef<HTMLDivElement>(null);
  const [viewportInsets, setViewportInsets] = useState({ top: 0, left: 0, width: 0, height: 0 });
  const [scrubPosition, setScrubPosition] = useState<number | null>(null);
  const lastPatternRef = useRef<number | null>(null);
  const mountedRef = useRef(false);

  // Sync inputs to simulator
  useEffect(() => {
    simulator.set_depth(depth);
    simulator.set_stroke(stroke);
    simulator.set_velocity(velocity);
    simulator.set_sensation(sensation);

    if (!mountedRef.current) {
      mountedRef.current = true;
      if (wasPlaying && pattern >= 0) {
        lastPatternRef.current = pattern;
        simulator.play(pattern);
      }
      return;
    }
  }, [simulator, depth, stroke, velocity, sensation]);

  // Play on pattern change
  useEffect(() => {
    if (!mountedRef.current) return;
    if (pattern >= 0 && pattern !== lastPatternRef.current) {
      lastPatternRef.current = pattern;
      setWasPlaying(true);
      simulator.play(pattern);
    }
  }, [simulator, pattern, setWasPlaying]);

  useEffect(() => {
    const canvas = canvasRef.current;
    const viewport = viewportRef.current;
    if (!canvas || !viewport) {
      setViewportInsets({ top: 0, left: 0, width: 0, height: 0 });
      return;
    }

    const update = () => {
      const canvasRect = canvas.getBoundingClientRect();
      const vpRect = viewport.getBoundingClientRect();
      setViewportInsets({
        top: vpRect.top - canvasRect.top,
        left: vpRect.left - canvasRect.left,
        width: vpRect.width,
        height: vpRect.height,
      });
    };

    const observer = new ResizeObserver(update);
    observer.observe(canvas);
    observer.observe(viewport);
    return () => observer.disconnect();
  }, [isMobile]);

  const { data, chartSeries, stats } = useTrajectoryData({
    pattern, depth, stroke, velocity, sensation, timestep, duration, unitMode,
  });

  const handlePlay = useCallback(() => {
    setWasPlaying(true);
    lastPatternRef.current = pattern;
    simulator.play(pattern);
  }, [simulator, pattern, setWasPlaying]);

  const handlePause = useCallback(() => {
    simulator.pause();
  }, [simulator]);

  const handleResume = useCallback(() => {
    simulator.resume();
  }, [simulator]);

  const handleResetDefaults = useCallback(() => {
    setPattern(DEFAULTS.pattern);
    setDepth(DEFAULTS.depth);
    setStroke(DEFAULTS.stroke);
    setVelocity(DEFAULTS.velocity);
    setSensation(DEFAULTS.sensation);
    setTimestep(DEFAULTS.timestep);
    setDuration(DEFAULTS.duration);
    setUnitMode(DEFAULTS.unitMode);
  }, [setPattern, setDepth, setStroke, setVelocity, setSensation, setTimestep, setDuration, setUnitMode]);

  const handleScrub = useCallback(
    (index: number | null) => {
      setScrubPosition(index != null ? data.position[index] : null);
    },
    [data],
  );

  return (
    <Box
      ref={canvasRef}
      style={{ flex: 1, position: "relative", overflow: "hidden" }}
    >
      <Suspense fallback={null}>
        <Scene
          ref={sceneRef}
          simulator={simulator}
          zoom={isMobile ? 900 : 1500}
          viewportInsets={viewportInsets}
          overridePosition={scrubPosition}
        />
      </Suspense>

      <div
        style={{
          position: "absolute",
          inset: 0,
          padding: 12,
          display: "grid",
          gridTemplateColumns: isMobile ? "1fr" : "1fr 280px",
          gridTemplateRows: isMobile ? "1fr auto auto" : "1fr auto auto",
          gridTemplateAreas: isMobile
            ? `"viewport" "buttons" "sidebar"`
            : `"viewport sidebar" "buttons sidebar" "chart sidebar"`,
          gap: isMobile ? 8 : 12,
          pointerEvents: "none",
        }}
      >
        <Flex
          ref={viewportRef}
          direction="column"
          justify="end"
          style={{ gridArea: "viewport", minHeight: 0 }}
        />

        <Flex gap="2" align="center" style={{ gridArea: "buttons" }}>
          <Button
            variant="outline"
            size="2"
            style={glassStyle}
            onClick={
              playbackState === "playing" || playbackState === "homing"
                ? handlePause
                : playbackState === "paused"
                  ? handleResume
                  : handlePlay
            }
          >
            {playbackState === "playing" || playbackState === "homing" ? (
              <PauseIcon />
            ) : (
              <PlayIcon />
            )}
            {playbackState === "playing" || playbackState === "homing"
              ? "Pause"
              : playbackState === "paused"
                ? "Resume"
                : "Play"}
          </Button>
          <Box style={{ flex: 1 }} />
          <Button
            variant="outline"
            size="2"
            style={glassStyle}
            onClick={() => sceneRef.current?.resetView()}
          >
            <ReloadIcon />
            Reset View
          </Button>
        </Flex>

        {!isMobile && (
          <Card
            size="2"
            style={{
              gridArea: "chart",
              pointerEvents: "auto",
              backdropFilter: "blur(12px)",
              backgroundColor: "var(--color-panel-translucent)",
            }}
          >
            <Chart.Legend
              series={chartSeries}
              focused={focused}
              onFocusChange={setFocused}
            />
            <Chart.Canvas
              series={chartSeries}
              xData={data.time}
              focused={focused}
              appearance={appearance}
              height={175}
              formatXTick={(v) => `${Math.round(v)}s`}
              onScrub={handleScrub}
              mt="2"
            />
          </Card>
        )}

        <Card
          size="2"
          style={{
            gridArea: "sidebar",
            pointerEvents: "auto",
            minHeight: 0,
            height: "100%",
            display: "flex",
            flexDirection: "column",
            overflow: "auto",
            backdropFilter: "blur(12px)",
            backgroundColor: "var(--color-panel-translucent)",
          }}
        >
          <TrajectorySidebar
            pattern={pattern}
            onPatternChange={setPattern}
            depth={depth}
            onDepthChange={setDepth}
            stroke={stroke}
            onStrokeChange={setStroke}
            velocity={velocity}
            onVelocityChange={setVelocity}
            sensation={sensation}
            onSensationChange={setSensation}
            compact={isMobile}
            unitMode={unitMode}
            onUnitModeChange={setUnitMode}
            duration={duration}
            onDurationValueChange={setDuration}
            timestep={timestep}
            onTimestepChange={setTimestep}
            stats={stats}
            onResetDefaults={handleResetDefaults}
          />
        </Card>
      </div>
    </Box>
  );
}
