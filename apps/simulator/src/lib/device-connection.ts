import {
  TelemetryDecoder,
  MotionPhase,
  type CoreTelemetryPayload,
} from "./telemetry";

export type ConnectionStatus =
  | "disconnected"
  | "connecting"
  | "connected"
  | "reconnecting";

export interface DeviceConnectionEvents {
  telemetry: (payload: CoreTelemetryPayload) => void;
  log: (text: string) => void;
  status: (status: ConnectionStatus) => void;
}

type Listener<K extends keyof DeviceConnectionEvents> = DeviceConnectionEvents[K];

/**
 * Manages a Web Serial connection to an OSSM device.
 * Reads the byte stream, splits COBS telemetry frames from ASCII log lines,
 * and exposes the latest telemetry state for polling.
 */
export class DeviceConnection {
  private port: SerialPort | null = null;
  private reader: ReadableStreamDefaultReader<Uint8Array> | null = null;
  private running = false;
  private decoder = new TelemetryDecoder();
  private listeners = new Map<string, Set<Function>>();

  private _status: ConnectionStatus = "disconnected";
  private _latestTelemetry: CoreTelemetryPayload = {
    position: 0,
    velocity: 0,
    torque: 0,
    phase: MotionPhase.Disabled,
    sequence: 0,
  };

  get status() {
    return this._status;
  }

  get position() {
    return this._latestTelemetry.position;
  }

  get phase() {
    return this._latestTelemetry.phase;
  }

  get latestTelemetry() {
    return this._latestTelemetry;
  }

  on<K extends keyof DeviceConnectionEvents>(
    event: K,
    listener: Listener<K>,
  ) {
    if (!this.listeners.has(event)) this.listeners.set(event, new Set());
    this.listeners.get(event)!.add(listener);
  }

  off<K extends keyof DeviceConnectionEvents>(
    event: K,
    listener: Listener<K>,
  ) {
    this.listeners.get(event)?.delete(listener);
  }

  private emit<K extends keyof DeviceConnectionEvents>(
    event: K,
    ...args: Parameters<DeviceConnectionEvents[K]>
  ) {
    const set = this.listeners.get(event);
    if (!set) return;
    for (const fn of set) (fn as Function)(...args);
  }

  private setStatus(status: ConnectionStatus) {
    this._status = status;
    this.emit("status", status);
  }

  /**
   * Prompt the user to select a serial port and begin reading telemetry.
   * If a port was previously granted, `getPorts()` will return it without
   * showing the dialog again.
   */
  async connect(baudRate = 115200) {
    if (this._status === "connected" || this._status === "connecting") return;

    this.setStatus("connecting");

    try {
      // Try previously-granted ports first, fall back to picker
      const ports = await navigator.serial.getPorts();
      this.port = ports[0] ?? (await navigator.serial.requestPort());

      await this.port.open({ baudRate });
      this.running = true;
      this.setStatus("connected");

      this.port.addEventListener("disconnect", this.onDisconnect);
      this.readLoop();
    } catch (err) {
      this.setStatus("disconnected");
      throw err;
    }
  }

  async disconnect() {
    this.running = false;
    this.port?.removeEventListener("disconnect", this.onDisconnect);

    try {
      await this.reader?.cancel();
    } catch {
      // reader may already be released
    }
    this.reader = null;

    try {
      await this.port?.close();
    } catch {
      // port may already be closed
    }
    this.port = null;

    this.setStatus("disconnected");
  }

  private onDisconnect = () => {
    this.running = false;
    this.reader = null;
    this.port = null;
    this.setStatus("reconnecting");
    this.attemptReconnect();
  };

  private async attemptReconnect() {
    const maxAttempts = 30;
    const intervalMs = 1000;

    for (let i = 0; i < maxAttempts; i++) {
      await sleep(intervalMs);
      if (this._status !== "reconnecting") return;

      try {
        const ports = await navigator.serial.getPorts();
        if (ports.length === 0) continue;

        this.port = ports[0];
        await this.port.open({ baudRate: 115200 });
        this.running = true;
        this.decoder = new TelemetryDecoder();
        this.setStatus("connected");

        this.port.addEventListener("disconnect", this.onDisconnect);
        this.readLoop();
        return;
      } catch {
        // port not ready yet, keep trying
      }
    }

    this.setStatus("disconnected");
  }

  private async readLoop() {
    if (!this.port?.readable) return;

    try {
      this.reader = this.port.readable.getReader();

      while (this.running) {
        const { value, done } = await this.reader.read();
        if (done || !value) break;

        const frames = this.decoder.push(value);
        for (const frame of frames) {
          if (frame.type === "telemetry") {
            this._latestTelemetry = frame.payload;
            this.emit("telemetry", frame.payload);
          } else {
            this.emit("log", frame.text);
          }
        }
      }
    } catch {
      // read error — likely disconnect, handled by onDisconnect
    } finally {
      try {
        this.reader?.releaseLock();
      } catch {
        // already released
      }
      this.reader = null;
    }
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
