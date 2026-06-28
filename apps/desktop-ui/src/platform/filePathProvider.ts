// This file abstracts file path resolution across environments
// 1. productionDesktopPath: Tauri capabilities (if available)
// 2. developerManualPath: prompt/manual input for developers
// 3. demoOnly: purely mock visual, no API call

// For our current environment, since Tauri isn't fully integrated yet,
// we will fallback to developerManualPath.

export async function requestFilePath(): Promise<string[] | null> {
  // Check if we are in Tauri
  const isTauri = typeof window !== 'undefined' && (window as any).__TAURI__;
  
  if (isTauri) {
    try {
      // In a real Tauri app, we'd use @tauri-apps/api/dialog
      // const { open } = await import('@tauri-apps/api/dialog');
      // const selected = await open({ multiple: true });
      // return Array.isArray(selected) ? selected : (selected ? [selected] : null);
      
      // Since we don't have the package installed, we still fallback to prompt if undefined
    } catch (e) {
      console.warn('Tauri API failed', e);
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
