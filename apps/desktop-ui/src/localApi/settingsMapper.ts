import type { PublicSettingsDto, SettingsUpdateRequest } from "./dto";

export interface SettingsFormState {
  device_name: string;
  peer_name: string;
  peer_host: string;
  peer_port: number;
  receive_dir: string;
  auto_connect: boolean;
  clipboard_enabled: boolean;
  clipboard_text_enabled: boolean;
  clipboard_image_enabled: boolean;
  max_image_bytes: number;
  retry_limit: number;
}

export function settingsToForm(settings: PublicSettingsDto): SettingsFormState {
  return {
    device_name: settings.device_name,
    peer_name: settings.peer_name,
    peer_host: settings.peer_host,
    peer_port: settings.peer_port,
    receive_dir: settings.receive_dir,
    auto_connect: settings.auto_connect,
    clipboard_enabled: settings.clipboard.enabled,
    clipboard_text_enabled: settings.clipboard.text_enabled,
    clipboard_image_enabled: settings.clipboard.image_enabled,
    max_image_bytes: settings.clipboard.max_image_bytes,
    retry_limit: settings.retry_limit,
  };
}

export function settingsFormToUpdate(form: SettingsFormState): SettingsUpdateRequest {
  return {
    device_name: form.device_name.trim(),
    peer_name: form.peer_name.trim(),
    peer_host: form.peer_host.trim(),
    peer_port: Number(form.peer_port),
    receive_dir: form.receive_dir.trim(),
    auto_connect: form.auto_connect,
    clipboard_enabled: form.clipboard_enabled,
    clipboard_text_enabled: form.clipboard_text_enabled,
    clipboard_image_enabled: form.clipboard_image_enabled,
    max_image_bytes: Number(form.max_image_bytes),
    retry_limit: Number(form.retry_limit),
  };
}
