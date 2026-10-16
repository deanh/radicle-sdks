/** T0-only test probes, not re-exported by radicle-sdk. */
import {
  probeTimestamp as nativeTimestamp,
  probePanic as nativePanic,
  probeSyncPanic as nativeSyncPanic,
} from '../dist/native.cjs';
import { run, sync } from './bridge.js';
export function probeTimestamp(decimal: string, seconds: boolean): Promise<number> {
  return run(() => nativeTimestamp(decimal, seconds));
}
export function probePanic(): Promise<void> {
  return run(() => nativePanic());
}
export function probeSyncPanic(): void {
  sync(() => nativeSyncPanic());
}
