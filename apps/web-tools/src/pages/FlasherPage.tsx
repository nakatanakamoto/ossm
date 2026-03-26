import { useState, useCallback, useRef, useMemo, useEffect } from "react";
import { ESPLoader, Transport } from "esptool-js";
import CryptoJS from "crypto-js";
import {
  Box,
  Card,
  Flex,
  Heading,
  Text,
  Select,
  Button,
  Callout,
  Separator,
  Progress,
  ScrollArea,
  Code,
} from "@radix-ui/themes";
import {
  InfoCircledIcon,
  ExclamationTriangleIcon,
} from "@radix-ui/react-icons";
import { useGithubReleases } from "../hooks/useGithubReleases";

const BAUD = 921600;

// Friendly labels for known firmware binaries; anything else falls back to the filename
const BOARD_LABELS: Record<string, string> = {
  "ossm-alt": "OSSM Alt",
  waveshare: "Waveshare ESP32-S3-RS485-CAN",
};

type FlashState =
  | "idle"
  | "connecting"
  | "connected"
  | "flashing"
  | "done"
  | "error";

export default function FlasherPage() {
  const {
    releases,
    loading: releasesLoading,
    error: releasesError,
  } = useGithubReleases();
  const [selectedRelease, setSelectedRelease] = useState<string>("");
  const [selectedBoard, setSelectedBoard] = useState<string>("");
  const [flashState, setFlashState] = useState<FlashState>("idle");
  const [progress, setProgress] = useState(0);
  const [logs, setLogs] = useState<string[]>([]);
  const logEndRef = useRef<HTMLDivElement>(null);

  const transportRef = useRef<Transport | null>(null);
  const espLoaderRef = useRef<ESPLoader | null>(null);
  const deviceRef = useRef<SerialPort | null>(null);

  const webSerialSupported =
    typeof navigator !== "undefined" && "serial" in navigator;

  useEffect(() => {
    return () => {
      transportRef.current?.disconnect().catch(() => {});
    };
  }, []);

  // Derive available boards from the selected release's .bin assets
  const boards = useMemo(() => {
    const release = releases.find((r) => r.tag_name === selectedRelease);
    if (!release) return [];
    return release.assets
      .filter((a) => a.name.endsWith(".bin"))
      .map((a) => {
        const stem = a.name.replace(/\.bin$/, "");
        return {
          value: a.name,
          label: BOARD_LABELS[stem] ?? a.name,
          asset: a,
        };
      });
  }, [releases, selectedRelease]);

  const appendLog = useCallback((line: string) => {
    setLogs((prev) => [...prev, line]);
    // Auto-scroll on next frame
    requestAnimationFrame(() => {
      logEndRef.current?.scrollIntoView({ behavior: "smooth" });
    });
  }, []);

  const espLoaderTerminal = useRef({
    clean() {},
    writeLine(_data: string) {},
    write(_data: string) {},
  }).current;

  const handleConnect = useCallback(async () => {
    if (flashState === "connected" || flashState === "connecting") {
      // Disconnect
      if (transportRef.current) {
        await transportRef.current.disconnect();
      }
      transportRef.current = null;
      espLoaderRef.current = null;
      deviceRef.current = null;
      setFlashState("idle");
      setLogs([]);
      return;
    }

    try {
      setFlashState("connecting");
      appendLog("Requesting serial port...");

      const device = await navigator.serial.requestPort();
      deviceRef.current = device;

      const transport = new Transport(device);
      transportRef.current = transport;

      const espLoader = new ESPLoader({
        transport,
        baudrate: BAUD,
        romBaudrate: 115200,
        terminal: espLoaderTerminal,
      });
      espLoaderRef.current = espLoader;

      const chip = await espLoader.main();
      appendLog(`Connected to ${chip}`);
      setFlashState("connected");
    } catch (err) {
      appendLog(
        `Connection failed: ${err instanceof Error ? err.message : err}`,
      );
      setFlashState("error");
    }
  }, [flashState, appendLog, espLoaderTerminal]);

  const handleFlash = useCallback(async () => {
    const espLoader = espLoaderRef.current;
    if (!espLoader || !selectedBoard) return;

    const board = boards.find((b) => b.value === selectedBoard);
    if (!board) {
      appendLog(`No binary selected`);
      setFlashState("error");
      return;
    }

    const asset = board.asset;

    try {
      setFlashState("flashing");
      setProgress(0);

      appendLog(`Downloading ${asset.name} (${formatBytes(asset.size)})...`);
      const response = await fetch(`/api/github-asset/${asset.id}`);
      if (!response.ok) {
        throw new Error(`Download failed: ${response.status}`);
      }
      const arrayBuffer = await response.arrayBuffer();
      const binaryStr = Array.from(new Uint8Array(arrayBuffer), (byte) =>
        String.fromCharCode(byte),
      ).join("");
      appendLog(`Downloaded ${arrayBuffer.byteLength} bytes`);

      appendLog("Flashing...");
      await espLoader.writeFlash({
        fileArray: [{ data: binaryStr, address: 0 }],
        flashSize: "keep",
        flashMode: "keep",
        flashFreq: "keep",
        eraseAll: false,
        compress: true,
        reportProgress: (
          _fileIndex: number,
          written: number,
          total: number,
        ) => {
          setProgress(Math.round((written / total) * 100));
        },
        calculateMD5Hash: (image: string) => {
          return CryptoJS.MD5(CryptoJS.enc.Latin1.parse(image)).toString();
        },
      });

      appendLog("Resetting device...");
      await espLoader.after();
      appendLog("Flash complete!");
      setFlashState("done");
    } catch (err) {
      appendLog(`Flash failed: ${err instanceof Error ? err.message : err}`);
      setFlashState("error");
    }
  }, [boards, selectedBoard, appendLog]);

  // Auto-select the first release when loaded
  if (releases.length > 0 && !selectedRelease) {
    setSelectedRelease(releases[0].tag_name);
  }

  // Auto-select first board when boards change (release changed or loaded)
  if (boards.length > 0 && !boards.some((b) => b.value === selectedBoard)) {
    setSelectedBoard(boards[0].value);
  }

  const isConnected =
    flashState === "connected" ||
    flashState === "flashing" ||
    flashState === "done";
  const isFlashing = flashState === "flashing";

  return (
    <Flex
      direction="column"
      align="center"
      style={{ flex: 1, overflow: "auto" }}
      p="4"
      gap="4"
    >
      <Box style={{ width: "100%", maxWidth: 600 }}>
        <Heading size="5" mb="4">
          Firmware Flasher
        </Heading>

        {!webSerialSupported && (
          <Callout.Root color="red" mb="4">
            <Callout.Icon>
              <ExclamationTriangleIcon />
            </Callout.Icon>
            <Callout.Text>
              Web Serial is not supported in this browser. Please use Chrome or
              Edge.
            </Callout.Text>
          </Callout.Root>
        )}

        <Card size="2">
          <Flex direction="column" gap="4">
            <Flex direction="column" gap="2">
              <Text size="2" weight="medium" as="label">
                Release
              </Text>
              {releasesLoading ? (
                <Text size="2" color="gray">
                  Loading releases...
                </Text>
              ) : releasesError ? (
                <Callout.Root color="red" size="1">
                  <Callout.Icon>
                    <ExclamationTriangleIcon />
                  </Callout.Icon>
                  <Callout.Text>{releasesError}</Callout.Text>
                </Callout.Root>
              ) : (
                <Select.Root
                  value={selectedRelease}
                  onValueChange={setSelectedRelease}
                  disabled={isConnected}
                >
                  <Select.Trigger style={{ width: "100%" }} />
                  <Select.Content>
                    {releases.map((r) => (
                      <Select.Item key={r.tag_name} value={r.tag_name}>
                        {r.tag_name}
                        {r.name && r.name !== r.tag_name ? ` — ${r.name}` : ""}
                      </Select.Item>
                    ))}
                  </Select.Content>
                </Select.Root>
              )}
            </Flex>

            <Flex direction="column" gap="2">
              <Text size="2" weight="medium" as="label">
                Board
              </Text>
              <Select.Root
                value={selectedBoard}
                onValueChange={setSelectedBoard}
                disabled={isConnected || boards.length === 0}
              >
                <Select.Trigger style={{ width: "100%" }} />
                <Select.Content>
                  {boards.map((b) => (
                    <Select.Item key={b.value} value={b.value}>
                      {b.label}
                    </Select.Item>
                  ))}
                </Select.Content>
              </Select.Root>
            </Flex>

            <Separator size="4" />

            <Flex gap="2">
              <Button
                onClick={handleConnect}
                disabled={!webSerialSupported || isFlashing}
                variant={isConnected ? "outline" : "solid"}
                style={{ flex: 1 }}
              >
                {flashState === "connecting"
                  ? "Connecting..."
                  : isConnected
                    ? "Disconnect"
                    : "Connect"}
              </Button>
              <Button
                onClick={handleFlash}
                disabled={!isConnected || isFlashing || !selectedRelease}
                variant="solid"
                color="green"
                style={{ flex: 1 }}
              >
                {isFlashing ? "Flashing..." : "Flash"}
              </Button>
            </Flex>

            {isFlashing && (
              <Flex direction="column" gap="2">
                <Flex justify="between">
                  <Text size="2" weight="medium">
                    Progress
                  </Text>
                  <Text size="2" color="gray">
                    {progress}%
                  </Text>
                </Flex>
                <Progress value={progress} />
              </Flex>
            )}

            {flashState === "done" && (
              <Callout.Root color="green" size="1">
                <Callout.Icon>
                  <InfoCircledIcon />
                </Callout.Icon>
                <Callout.Text>
                  Flash complete! Your device has been updated.
                </Callout.Text>
              </Callout.Root>
            )}
          </Flex>
        </Card>

        {logs.length > 0 && (
          <Card size="2" mt="4">
            <Text size="2" weight="medium" mb="2" as="p">
              Log
            </Text>
            <ScrollArea
              style={{
                height: 200,
                background: "var(--gray-2)",
                borderRadius: "var(--radius-2)",
              }}
              scrollbars="vertical"
            >
              <Code
                size="1"
                style={{ whiteSpace: "pre-wrap", display: "block" }}
              >
                <Box p="2">{logs.join("\n")}</Box>
              </Code>
              <div ref={logEndRef} />
            </ScrollArea>
          </Card>
        )}
      </Box>
    </Flex>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
