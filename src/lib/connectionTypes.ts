export interface WslConfig {
  distro: string;
  user: string;
  dataRoot: string | null;
}

export interface ConnectionView {
  platform: "windows" | "local";
  state: "connecting" | "connected" | "disconnected";
  config: WslConfig | null;
  dataRoot: string | null;
  home: string | null;
  revision: number;
  message: string | null;
  externalAvailable: boolean;
  connectionId: number;
  distributions: string[];
}
