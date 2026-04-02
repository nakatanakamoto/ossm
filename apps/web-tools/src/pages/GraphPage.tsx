import { useCallback } from "react";
import { Box, Card, Flex, Text } from "@radix-ui/themes";
import { useAppearance } from "../hooks/useAppearance";
import { useIsMobile } from "../hooks/useIsMobile";
import { usePersistedState } from "../hooks/usePersistedState";
import { Chart } from "../Chart";
import {
  TrajectorySidebar,
  useTrajectoryData,
  DEFAULTS,
  type UnitMode,
} from "../TrajectoryPanel";

export default function GraphPage() {
  const [appearance] = useAppearance();
  const isMobile = useIsMobile();

  const [pattern, setPattern] = usePersistedState("ossm:pattern", 0);
  const [depth, setDepth] = usePersistedState("ossm:depth", 0.75);
  const [stroke, setStroke] = usePersistedState("ossm:stroke", 0.5);
  const [velocity, setVelocity] = usePersistedState("ossm:velocity", 0.75);
  const [sensation, setSensation] = usePersistedState("ossm:sensation", 0.0);
  const [timestep, setTimestep] = usePersistedState("ossm:timestep", 20);
  const [duration, setDuration] = usePersistedState("ossm:duration", 20);
  const [unitMode, setUnitMode] = usePersistedState<UnitMode>("ossm:unitMode", "relative");

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

  const { data, chartSeries, stats } = useTrajectoryData({
    pattern, depth, stroke, velocity, sensation, timestep, duration, unitMode,
  });

  const charts = (
    <Flex direction="column" p="3" gap="3">
      {chartSeries.map((s) => (
        <Card size="2" key={s.key}>
          <Flex align="center" gap="2" mb="2">
            <Box
              style={{
                width: 12,
                height: 12,
                borderRadius: 2,
                backgroundColor: s.color,
              }}
            />
            <Text size="2" weight="medium">
              {s.label}
            </Text>
            {s.unit && (
              <Text size="1" color="gray">
                ({s.unit})
              </Text>
            )}
          </Flex>
          <Chart.Canvas
            series={[s]}
            xData={data.time}
            focused={s.key}
            appearance={appearance}
            height={180}
            formatXTick={(v) => `${Math.round(v)}s`}
          />
        </Card>
      ))}
    </Flex>
  );

  const sidebar = (
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
      unitMode={unitMode}
      onUnitModeChange={setUnitMode}
      duration={duration}
      onDurationValueChange={setDuration}
      timestep={timestep}
      onTimestepChange={setTimestep}
      stats={stats}
      onResetDefaults={handleResetDefaults}
      compact={isMobile}
      p="3"
    />
  );

  return (
    <Flex
      direction={isMobile ? "column-reverse" : "row"}
      style={{ flex: 1, minHeight: 0, minWidth: 0 }}
    >
      <Box
        style={{
          ...(isMobile
            ? { flexShrink: 0, maxHeight: "50%", borderTop: "1px solid var(--gray-5)" }
            : { width: 280, flexShrink: 0, borderRight: "1px solid var(--gray-5)" }),
          minHeight: 0,
          overflow: "auto",
        }}
      >
        {sidebar}
      </Box>
      <Box style={{ flex: 1, minHeight: 0, minWidth: 0, overflow: "auto" }}>
        {charts}
      </Box>
    </Flex>
  );
}
