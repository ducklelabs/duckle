// Server-backed filesystem for the web edition (#75 phase 2). The browser can't
// touch the server's disk, so workspace.ts routes its file ops through here to
// the duckle-runner web API (/api/fs/*) instead of the Tauri fs plugin. The
// surface mirrors the subset of @tauri-apps/plugin-fs that workspace.ts uses.
import { isTauri } from './tauri-dialog';

// True in the browser build (no Tauri); false in the desktop app at runtime.
export function isWebBackend(): boolean {
    return !isTauri();
}

async function fsApi<T>(op: string, payload: Record<string, unknown>): Promise<T> {
    const res = await fetch(`/api/fs/${op}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
    });
    if (!res.ok) {
        throw new Error(`fs/${op}: HTTP ${res.status} ${await res.text().catch(() => '')}`);
    }
    return (await res.json()) as T;
}

interface DirEntry {
    name: string;
    isFile: boolean;
    isDirectory: boolean;
}

// Matches the shape of @tauri-apps/plugin-fs that workspace.ts imports.
export const webFs = {
    async exists(path: string): Promise<boolean> {
        return (await fsApi<{ exists: boolean }>('exists', { path })).exists;
    },
    async mkdir(path: string, _opts?: { recursive?: boolean }): Promise<void> {
        await fsApi('mkdir', { path });
    },
    async readTextFile(path: string): Promise<string> {
        return (await fsApi<{ content: string }>('read', { path })).content;
    },
    async writeTextFile(path: string, content: string): Promise<void> {
        await fsApi('write', { path, content });
    },
    async readDir(path: string): Promise<DirEntry[]> {
        return await fsApi<DirEntry[]>('readdir', { path });
    },
    async remove(path: string): Promise<void> {
        await fsApi('remove', { path });
    },
};
