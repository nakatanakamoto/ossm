import {
  Suspense,
  useRef,
  useEffect,
  useCallback,
  useSyncExternalStore,
} from "react";
import {
  Theme,
  Box,
  Card,
  Flex,
  Heading,
  Text,
  Button,
  IconButton,
  Spinner,
  Separator,
  ScrollArea,
  Tooltip,
  Badge,
} from "@radix-ui/themes";
import {
  SunIcon,
  MoonIcon,
  ResetIcon,
  Link2Icon,
  LinkBreak2Icon,
} from "@radix-ui/react-icons";
import { Link } from "react-router";
import Scene from "../Scene";
import type { SceneHandle } from "../Scene";
import { useAppearance } from "../hooks/useAppearance";
import { useIsMobile } from "../hooks/useIsMobile";
import {
  DeviceConnection,
  type ConnectionStatus,
} from "../lib/device-connection";
import { MotionPhase, type CoreTelemetryPayload } from "../lib/telemetry";

const device = new DeviceConnection();

function useConnectionStatus(): ConnectionStatus {
  return useSyncExternalStore(
    (onChange) => {
      device.on("status", onChange);
      return () => device.off("status", onChange);
    },
    () => device.status,
  );
}

function useTelemetry(): CoreTelemetryPayload {
  const ref = useRef(device.latestTelemetry);

  return useSyncExternalStore(
    (onChange) => {
      const handler = (payload: CoreTelemetryPayload) => {
        ref.current = payload;
        onChange();
      };
      device.on("telemetry", handler);
      return () => device.off("telemetry", handler);
    },
    () => ref.current,
  );
}

const STATUS_LABEL: Record<ConnectionStatus, string> = {
  disconnected: "Disconnected",
  connecting: "Connecting…",
  connected: "Connected",
  reconnecting: "Device flashing…",
};

const STATUS_COLOR: Record<ConnectionStatus, "red" | "yellow" | "green" | "orange"> = {
  disconnected: "red",
  connecting: "yellow",
  connected: "green",
  reconnecting: "orange",
};

const PHASE_LABEL: Record<MotionPhase, string> = {
  [MotionPhase.Disabled]: "Disabled",
  [MotionPhase.Enabled]: "Enabled",
  [MotionPhase.Ready]: "Ready",
  [MotionPhase.Moving]: "Moving",
  [MotionPhase.Stopping]: "Stopping",
  [MotionPhase.Paused]: "Paused",
};

export function DevicePage() {
  const sceneRef = useRef<SceneHandle>(null);
  const [appearance, toggleAppearance] = useAppearance();
  const isMobile = useIsMobile();
  const status = useConnectionStatus();
  const telemetry = useTelemetry();

  useEffect(() => {
    return () => {
      device.disconnect();
    };
  }, []);

  const handleConnect = useCallback(async () => {
    try {
      await device.connect();
    } catch (err) {
      console.error("Failed to connect:", err);
    }
  }, []);

  const handleDisconnect = useCallback(async () => {
    await device.disconnect();
  }, []);

  const getPosition = useCallback(() => device.position, []);

  const isConnected = status === "connected";
  const isDisconnected = status === "disconnected";

  return (
    <Theme accentColor="purple" radius="large" appearance={appearance}>
      <Flex direction={isMobile ? "column" : "row"} height="100vh">
        <Box
          style={{
            flex: isMobile ? undefined : 1,
            height: isMobile ? "30vh" : "100vh",
            minHeight: 0,
            position: "relative",
          }}
        >
          <Suspense
            fallback={
              <Flex align="center" justify="center" height="100%" gap="3">
                <Spinner size="3" />
                <Text size="2">Loading model…</Text>
              </Flex>
            }
          >
            <Scene
              ref={sceneRef}
              getPosition={getPosition}
              zoom={isMobile ? 900 : 1500}
            />
          </Suspense>
        </Box>

        <Box
          style={{
            width: isMobile ? undefined : "360px",
            height: isMobile ? "70vh" : "100vh",
            flexShrink: 0,
          }}
          p="3"
        >
          <Card
            size="2"
            style={{
              height: "100%",
              display: "flex",
              flexDirection: "column",
            }}
          >
            <Flex justify="between" align="center" mb="3">
              <Heading size="5">Device</Heading>
              <Tooltip
                content={
                  appearance === "light"
                    ? "Switch to dark mode"
                    : "Switch to light mode"
                }
              >
                <IconButton
                  variant="ghost"
                  size="2"
                  onClick={toggleAppearance}
                  aria-label="Toggle theme"
                >
                  {appearance === "light" ? <SunIcon /> : <MoonIcon />}
                </IconButton>
              </Tooltip>
            </Flex>

            <Separator size="4" mb="3" />

            <ScrollArea style={{ flex: 1 }} scrollbars="vertical">
              <Flex direction="column" gap="4" pr="1">
                <Flex align="center" gap="2">
                  <Badge color={STATUS_COLOR[status]} variant="soft" size="2">
                    {STATUS_LABEL[status]}
                  </Badge>
                </Flex>

                {isDisconnected && (
                  <Button onClick={handleConnect} variant="solid" size="3">
                    <Link2Icon /> Connect
                  </Button>
                )}

                {isConnected && (
                  <Button
                    onClick={handleDisconnect}
                    variant="outline"
                    size="3"
                  >
                    <LinkBreak2Icon /> Disconnect
                  </Button>
                )}

                {status === "reconnecting" && (
                  <Flex align="center" gap="2">
                    <Spinner size="2" />
                    <Text size="2" color="gray">
                      Waiting for device to reappear…
                    </Text>
                  </Flex>
                )}

                {(isConnected || status === "reconnecting") && (
                  <>
                    <Separator size="4" />
                    <TelemetryDisplay telemetry={telemetry} />
                  </>
                )}

                <Separator size="4" />

                <Button
                  variant="outline"
                  onClick={() => sceneRef.current?.resetView()}
                  style={{ width: "100%" }}
                >
                  <ResetIcon /> Reset View
                </Button>

                <Button variant="outline" asChild style={{ width: "100%" }}>
                  <Link to="/">Back to Simulator</Link>
                </Button>
              </Flex>
            </ScrollArea>
          </Card>
        </Box>
      </Flex>
    </Theme>
  );
}

function TelemetryDisplay({
  telemetry,
}: {
  telemetry: CoreTelemetryPayload;
}) {
  return (
    <Flex direction="column" gap="2">
      <Text size="2" weight="medium">
        Telemetry
      </Text>
      <Flex direction="column" gap="1">
        <TelemetryRow label="Phase" value={PHASE_LABEL[telemetry.phase]} />
        <TelemetryRow
          label="Position"
          value={`${(telemetry.position * 100).toFixed(1)}%`}
        />
        <TelemetryRow
          label="Velocity"
          value={`${(telemetry.velocity * 100).toFixed(1)}%`}
        />
        <TelemetryRow
          label="Torque"
          value={`${(telemetry.torque * 100).toFixed(1)}%`}
        />
        <TelemetryRow label="Sequence" value={String(telemetry.sequence)} />
      </Flex>
    </Flex>
  );
}

function TelemetryRow({ label, value }: { label: string; value: string }) {
  return (
    <Flex justify="between">
      <Text size="2" color="gray">
        {label}
      </Text>
      <Text size="2" style={{ fontVariantNumeric: "tabular-nums" }}>
        {value}
      </Text>
    </Flex>
  );
}
