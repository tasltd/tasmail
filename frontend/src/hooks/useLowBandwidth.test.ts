import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { isSlowConnection, useLowBandwidthStore } from './useLowBandwidth';

describe('isSlowConnection', () => {
  const originalConnection = Object.getOwnPropertyDescriptor(navigator, 'connection');

  afterEach(() => {
    if (originalConnection) {
      Object.defineProperty(navigator, 'connection', originalConnection);
    } else {
      // Remove the property if it didn't exist originally
      Object.defineProperty(navigator, 'connection', { value: undefined, configurable: true });
    }
  });

  it('returns false when navigator.connection is undefined', () => {
    Object.defineProperty(navigator, 'connection', { value: undefined, configurable: true });
    expect(isSlowConnection()).toBe(false);
  });

  it('returns true when saveData is true', () => {
    Object.defineProperty(navigator, 'connection', {
      value: { effectiveType: '4g', downlink: 10, saveData: true },
      configurable: true,
    });
    expect(isSlowConnection()).toBe(true);
  });

  it('returns true when effectiveType is 2g', () => {
    Object.defineProperty(navigator, 'connection', {
      value: { effectiveType: '2g', downlink: 10, saveData: false },
      configurable: true,
    });
    expect(isSlowConnection()).toBe(true);
  });

  it('returns true when downlink < 0.5', () => {
    Object.defineProperty(navigator, 'connection', {
      value: { effectiveType: '3g', downlink: 0.3, saveData: false },
      configurable: true,
    });
    expect(isSlowConnection()).toBe(true);
  });

  it('returns false on a fast connection', () => {
    Object.defineProperty(navigator, 'connection', {
      value: { effectiveType: '4g', downlink: 10, saveData: false },
      configurable: true,
    });
    expect(isSlowConnection()).toBe(false);
  });
});

describe('useLowBandwidthStore', () => {
  beforeEach(() => {
    // Reset store to defaults
    useLowBandwidthStore.setState({ enabled: false, autoDetect: true, textOnly: false });
  });

  it('has correct default state', () => {
    const state = useLowBandwidthStore.getState();
    expect(state.enabled).toBe(false);
    expect(state.autoDetect).toBe(true);
    expect(state.textOnly).toBe(false);
  });

  it('setEnabled changes enabled state', () => {
    useLowBandwidthStore.getState().setEnabled(true);
    expect(useLowBandwidthStore.getState().enabled).toBe(true);
  });

  it('setTextOnly changes textOnly state', () => {
    useLowBandwidthStore.getState().setTextOnly(true);
    expect(useLowBandwidthStore.getState().textOnly).toBe(true);
  });
});
