import { Suspense, useEffect, useRef, useState, useCallback, useMemo } from "react";
import init, { Simulator } from "sim-wasm";
import wasmUrl from "sim-wasm/sim_wasm_bg.wasm?url";
import {
  Theme,
  Box,
  Flex,
  Heading,
  Text,
  Slider,
  Spinner,
  Select,
  Button,
} from "@radix-ui/themes";
import Scene from "./Scene";

type PlaybackState = "stopped" | "playing" | "paused";

export default function App() {
  const simRef = useRef<Simulator | null>(null);
  const [ready, setReady] = useState(false);
  const [depth, setDepth] = useState(1.0);
  const [stroke, setStroke] = useState(1.0);
  const [velocity, setVelocity] = useState(0.5);
  const [sensation, setSensation] = useState(0.0);
  const [selectedPattern, setSelectedPattern] = useState(0);
  const [playbackState, setPlaybackState] = useState<PlaybackState>("stopped");

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

  const patterns = useMemo<
    { index: number; name: string; description: string }[]
  >(() => {
    const sim = simRef.current;
    if (!sim) return [];
    const count = sim.pattern_count();
    return Array.from({ length: count }, (_, i) => ({
      index: i,
      name: sim.pattern_name(i),
      description: sim.pattern_description(i),
    }));
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

  const handlePlay = useCallback(() => {
    simRef.current?.play(selectedPattern);
    setPlaybackState("playing");
  }, [selectedPattern]);

  const handlePause = useCallback(() => {
    simRef.current?.pause();
    setPlaybackState("paused");
  }, []);

  const handleResume = useCallback(() => {
    simRef.current?.resume();
    setPlaybackState("playing");
  }, []);

  const handleStop = useCallback(() => {
    simRef.current?.stop();
    setPlaybackState("stopped");
  }, []);

  const handlePatternChange = useCallback((value: string) => {
    const index = Number(value);
    setSelectedPattern(index);
    simRef.current?.play(index);
    setPlaybackState("playing");
  }, []);

  if (!ready) {
    return (
      <Theme accentColor="purple" radius="large">
        <Flex align="center" justify="center" height="100vh" gap="3">
          <Spinner size="3" />
          <Text size="3">Loading simulator…</Text>
        </Flex>
      </Theme>
    );
  }

  return (
    <Theme accentColor="purple" radius="large">
      <Flex direction="column" height="100vh" maxWidth="800px" mx="auto">
        <Box flexShrink="0" height="600px">
          <Suspense
            fallback={
              <Flex align="center" justify="center" height="100%" gap="3">
                <Spinner size="3" />
                <Text size="2">Loading model…</Text>
              </Flex>
            }
          >
            <Scene simulator={simRef.current!} />
          </Suspense>
        </Box>

        <Box flexGrow="1" p="4" overflowY="auto">
          <Heading size="5" mb="4">
            OSSM Simulator
          </Heading>

          <Flex direction="column" gap="4">
            <Box>
              <Text size="2" weight="medium" mb="1" as="label">
                Pattern
              </Text>
              <Select.Root
                value={String(selectedPattern)}
                onValueChange={handlePatternChange}
              >
                <Select.Trigger style={{ width: "100%" }} />
                <Select.Content>
                  {patterns.map((p) => (
                    <Select.Item key={p.index} value={String(p.index)}>
                      {p.name}
                    </Select.Item>
                  ))}
                </Select.Content>
              </Select.Root>
              {patterns[selectedPattern] && (
                <Text size="1" color="gray" mt="1">
                  {patterns[selectedPattern].description}
                </Text>
              )}
            </Box>

            <Flex gap="2">
              {playbackState !== "playing" ? (
                <Button
                  onClick={
                    playbackState === "paused" ? handleResume : handlePlay
                  }
                  variant="solid"
                >
                  {playbackState === "paused" ? "Resume" : "Play"}
                </Button>
              ) : (
                <Button onClick={handlePause} variant="soft">
                  Pause
                </Button>
              )}
              <Button
                onClick={handleStop}
                variant="outline"
                disabled={playbackState === "stopped"}
              >
                Stop
              </Button>
            </Flex>

            <LabeledSlider
              label="Depth"
              value={depth}
              min={0}
              max={1}
              step={0.01}
              onChange={updateDepth}
            />
            <LabeledSlider
              label="Stroke"
              value={stroke}
              min={0}
              max={1}
              step={0.01}
              onChange={updateStroke}
            />
            <LabeledSlider
              label="Velocity"
              value={velocity}
              min={0}
              max={1}
              step={0.01}
              onChange={updateVelocity}
            />
            <LabeledSlider
              label="Sensation"
              value={sensation}
              min={-1}
              max={1}
              step={0.01}
              onChange={updateSensation}
            />
          </Flex>
        </Box>
      </Flex>
    </Theme>
  );
}

function LabeledSlider({
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
    <Box>
      <Flex justify="between" mb="1">
        <Text size="2" weight="medium">
          {label}
        </Text>
        <Text size="2" color="gray">
          {value}
        </Text>
      </Flex>
      <Slider
        min={min}
        max={max}
        step={step}
        value={[value]}
        onValueChange={(values) => onChange(values[0])}
      />
    </Box>
  );
}
