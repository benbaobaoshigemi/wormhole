export type ConnectionStatus = "unconfigured" | "connecting" | "connected" | "peer_offline" | "failed";
export type TransferDirection = "send" | "receive";
export type TransferStatus = "queued" | "transferring" | "completed" | "failed" | "cancelled" | "retrying" | "prepared";

export interface PublicDevice {
  device_id: string;
  device_name: string;
  platform: string;
  port: number;
  protocol_version: number;
  capabilities: string[];
}

export interface ClipboardStatusDto {
  enabled: boolean;
  text_enabled: boolean;
  image_enabled: boolean;
  max_image_bytes: number;
}

export interface PublicSettingsDto {
  device_id: string;
  device_name: string;
  platform: string;
  bind_host: string;
  port: number;
  peer_name: string;
  peer_host: string;
  peer_port: number;
  receive_dir: string;
  auto_connect: boolean;
  clipboard: ClipboardStatusDto & {
    poll_millis?: number;
    remote_hash_window?: number;
  };
  retry_limit: number;
}

export interface SettingsUpdateRequest {
  device_name?: string;
  peer_name?: string;
  peer_host?: string;
  peer_port?: number;
  receive_dir?: string;
  auto_connect?: boolean;
  clipboard_enabled?: boolean;
  clipboard_text_enabled?: boolean;
  clipboard_image_enabled?: boolean;
  max_image_bytes?: number;
  retry_limit?: number;
}

export interface TransferTaskDto {
  task_id: string;
  direction: TransferDirection;
  peer_device_id?: string | null;
  root_name: string;
  item_count: number;
  total_size: number;
  transferred_size: number;
  status: TransferStatus;
  error?: string | null;
  error_code?: string | null;
  save_path?: string | null;
  speed_bytes_per_sec: number;
  eta_seconds?: number | null;
  retry_count: number;
  parent_task_id?: string | null;
  attempt_id?: string | null;
  created_at: string;
  updated_at: string;
}

export interface WormholeEvent {
  ts: string;
  type: string;
  data: Record<string, unknown>;
}

export interface StateDto {
  device: PublicDevice;
  status: ConnectionStatus;
  peer?: PublicDevice | null;
  settings: PublicSettingsDto;
  clipboard: ClipboardStatusDto;
  active_transfer_count: number;
  recent_history_count: number;
  tasks: TransferTaskDto[];
  events: WormholeEvent[];
}
