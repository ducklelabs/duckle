import { invoke } from '@tauri-apps/api/core';
import { isTauri } from './tauri-dialog';
import type { Column } from './pipeline-types';

type AutodetectPayload = {
    columns: Column[];
    sampleRows: Record<string, unknown>[];
};

/**
 * Call into the Rust `autodetect_schema` Tauri command when running
 * under Tauri. Returns `null` in browser mode or on failure, so the
 * caller can fall back to a mock.
 */
export async function tauriAutodetect(
    format: string,
    options: Record<string, unknown>,
): Promise<AutodetectPayload | null> {
    if (!isTauri()) return null;
    try {
        return await invoke<AutodetectPayload>('autodetect_schema', { format, options });
    } catch (err) {
        console.warn('Tauri autodetect failed for ' + format, err);
        return null;
    }
}
