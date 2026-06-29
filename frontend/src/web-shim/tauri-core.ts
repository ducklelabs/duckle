// Web-build shim for `@tauri-apps/api/core`.
//
// In the desktop app the frontend talks to the Rust backend through Tauri's
// `invoke()`. For the server/web build (issue #75, phase 2) there is no Tauri:
// vite aliases `@tauri-apps/api/core` to this module (gated on DUCKLE_WEB), so
// every `invoke(cmd, args)` becomes an HTTP POST to the duckle-runner web API.
// Only `invoke` and `Channel` are imported from core across the frontend.

export async function invoke<T = unknown>(
    cmd: string,
    args?: Record<string, unknown>,
): Promise<T> {
    const res = await fetch(`/api/cmd/${cmd}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(args ?? {}),
    });
    if (!res.ok) {
        // Desktop-only commands are not implemented on the web backend, which
        // answers 404. The shared frontend invokes some of these unconditionally
        // (settings, MCP, build), so keep the editor resilient by resolving a 404
        // to null - the same no-op those call sites already tolerate - while
        // surfacing it for debugging. Other statuses are real errors and throw.
        if (res.status === 404) {
            console.warn(`invoke: command '${cmd}' is not available on the web backend (404)`);
            return null as T;
        }
        const detail = await res.text().catch(() => '');
        throw new Error(`${cmd}: HTTP ${res.status} ${detail}`);
    }
    const text = await res.text();
    return (text ? JSON.parse(text) : null) as T;
}

// Streaming run-progress channel. Stubbed for the spike: the run command will
// return its final result over HTTP; live per-node progress lands later via SSE.
// `toJSON` gives the backend a sentinel where Tauri would serialize the channel.
export class Channel<T = unknown> {
    onmessage: ((message: T) => void) | null = null;
    toJSON(): string {
        return '__DUCKLE_WEB_CHANNEL__';
    }
}

// Convenience exports a few call sites may reach for; harmless no-ops on web.
export function convertFileSrc(filePath: string): string {
    return filePath;
}

// The Tauri fs/dialog plugins import `Resource` from core. They are only ever
// called behind isTauri() guards (never in the browser), but Rollup still
// bundles them, so this stub just has to exist for the build to resolve.
export class Resource {
    readonly rid: number = 0;
    async close(): Promise<void> {}
}
