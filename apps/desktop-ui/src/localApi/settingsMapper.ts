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
  max_concurrent_tasks: number;
  parallel_chunk_uploads: number;
  chunk_size_bytes: number;
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
    max_concurrent_tasks: settings.transfer?.max_concurrent_tasks ?? 2,
    parallel_chunk_uploads: settings.transfer?.parallel_chunk_uploads ?? 4,
    chunk_size_bytes: settings.transfer?.chunk_size_bytes ?? 2097152,
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
    max_concurrent_tasks: Number(form.max_concurrent_tasks),
    parallel_chunk_uploads: Number(form.parallel_chunk_uploads),
    chunk_size_bytes: Number(form.chunk_size_bytes),
  };
}
