/**
 * Hook for low-bandwidth mode detection and management.
 * Provides a toggle and auto-detection based on Network Information API.
 */
import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface LowBandwidthState {
  enabled: boolean;
  autoDetect: boolean;
  textOnly: boolean;
  setEnabled: (enabled: boolean) => void;
  setAutoDetect: (auto: boolean) => void;
  setTextOnly: (textOnly: boolean) => void;
}

export const useLowBandwidthStore = create<LowBandwidthState>()(
  persist(
    (set) => ({
      enabled: false,
      autoDetect: true,
      textOnly: false,
      setEnabled: (enabled) => set({ enabled }),
      setAutoDetect: (autoDetect) => set({ autoDetect }),
      setTextOnly: (textOnly) => set({ textOnly }),
    }),
    { name: 'tasmail-low-bandwidth' },
  ),
);

// Added: Network Information API type (not in all browsers)
interface NetworkInformation {
  effectiveType: string;
  downlink: number;
  saveData: boolean;
}

// Added: Check if we're on a slow connection using Navigator.connection
export function isSlowConnection(): boolean {
  const nav = navigator as Navigator & { connection?: NetworkInformation };
  if (!nav.connection) return false;

  const { effectiveType, saveData, downlink } = nav.connection;

  // saveData preference from browser
  if (saveData) return true;

  // 2G or slow-3G connections
  if (effectiveType === '2g' || effectiveType === 'slow-2g') return true;

  // Very low downlink (less than 0.5 Mbps)
  if (downlink < 0.5) return true;

  return false;
}

// Added: Hook that combines store state with auto-detection
export function useLowBandwidth(): {
  isLowBandwidth: boolean;
  textOnly: boolean;
  store: LowBandwidthState;
} {
  const store = useLowBandwidthStore();

  const isLowBandwidth = store.enabled || (store.autoDetect && isSlowConnection());

  return {
    isLowBandwidth,
    textOnly: store.textOnly,
    store,
  };
}
