// This file abstracts file path resolution across environments
// 1. productionDesktopPath: Tauri capabilities (if available)
// 2. developerManualPath: prompt/manual input for developers
// 3. demoOnly: purely mock visual, no API call

export async function requestFilePath(): Promise<string[] | null> {
  // Check if we are in Tauri
  const isTauri = typeof window !== 'undefined' && (window as any).__TAURI__;
  
  if (isTauri) {
    try {
      // In Tauri v2, we import from @tauri-apps/plugin-dialog
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: true,
        directory: false,
        title: "Wormhole - Select Files to Send"
      });
      
      if (!selected) return null;
      return Array.isArray(selected) ? selected : [selected];
    } catch (e) {
      console.warn('Tauri dialog plugin failed, falling back to manual prompt', e);
    }
  }

  // developerManualPath fallback
  return requestDeveloperManualPath();
}

export async function requestDeveloperManualPath(): Promise<string[] | null> {
  const userInput = window.prompt("Developer Path Mode: Please enter an absolute file path to send. (Leave empty to cancel, do NOT use mock paths!)");
  
  if (!userInput || userInput.trim() === '') {
    return null; // Canceled
  }
  
  const path = userInput.trim();
  
  // Basic validation to prevent obvious mock paths
  if (path.includes('dummy') || path.includes('mock') || !path.includes('/') && !path.includes('\\')) {
    alert("Invalid or mock path detected. Only real absolute paths are permitted for /local/transfer/send.");
    return null;
  }
  
  return [path];
}
export async function requestDirectoryPath(): Promise<string[] | null> {
  const isTauri = typeof window !== 'undefined' && (window as any).__TAURI__;
  
  if (isTauri) {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: false,
        directory: true,
        title: "Wormhole - Select Folder to Send"
      });
      
      if (!selected) return null;
      return [selected];
    } catch (e) {
      console.warn('Tauri dialog plugin failed', e);
    }
  }

  // developerManualPath fallback
  const userInput = window.prompt("Developer Path Mode: Please enter an absolute folder path to send.");
  if (!userInput || userInput.trim() === '') return null;
  return [userInput.trim()];
}
