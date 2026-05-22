import { Channel, invoke } from '@tauri-apps/api/core';
import { isTauri } from './tauri-dialog';
import type { Column } from './pipeline-types';
import type { Edge, Node } from '@xyflow/react';
import type { DuckleNodeData } from './pipeline-types';

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

// ---- Pipeline execution ------------------------------------------------

export type NodeRunStatus = {
    status: 'ok' | 'error' | 'running';
    kind?: 'view' | 'sink';
    rows?: number;
    duration_ms?: number;
    error?: string;
};

export type NodePreview = {
    node_id: string;
    columns: Column[];
    rows: Record<string, unknown>[];
};

export type RunResult = {
    status: 'ok' | 'error' | 'cancelled';
    duration_ms: number;
    nodes: Record<string, NodeRunStatus>;
    preview: NodePreview[];
    error?: string;
};

export type PipelineEvent =
    | { type: 'started'; total_stages: number }
    | { type: 'stage_started'; node_id: string; label: string; kind: 'view' | 'sink' }
    | {
          type: 'stage_finished';
          node_id: string;
          kind: 'view' | 'sink';
          status: 'ok' | 'error';
          rows?: number;
          duration_ms: number;
          error?: string;
      }
    | { type: 'cancelled' }
    | { type: 'finished'; status: 'ok' | 'error' | 'cancelled'; duration_ms: number };

export async function runPipeline(
    nodes: Node<DuckleNodeData>[],
    edges: Edge[],
    onEvent?: (evt: PipelineEvent) => void,
    pipelineId?: string,
    workspacePath?: string | null,
): Promise<RunResult | null> {
    if (!isTauri()) return null;
    const channel = new Channel<PipelineEvent>();
    if (onEvent) channel.onmessage = onEvent;
    try {
        return await invoke<RunResult>('run_pipeline', {
            pipeline: { nodes, edges },
            onEvent: channel,
            pipelineId: pipelineId ?? null,
            workspacePath: workspacePath ?? null,
        });
    } catch (err) {
        console.error('runPipeline failed', err);
        return {
            status: 'error',
            duration_ms: 0,
            nodes: {},
            preview: [],
            error: String(err),
        };
    }
}

export async function runPipelinePartial(
    nodes: Node<DuckleNodeData>[],
    edges: Edge[],
    targetNodeId: string,
    onEvent?: (evt: PipelineEvent) => void,
    pipelineId?: string,
    workspacePath?: string | null,
): Promise<RunResult | null> {
    if (!isTauri()) return null;
    const channel = new Channel<PipelineEvent>();
    if (onEvent) channel.onmessage = onEvent;
    try {
        return await invoke<RunResult>('run_pipeline_partial', {
            pipeline: { nodes, edges },
            targetNodeId,
            onEvent: channel,
            pipelineId: pipelineId ?? null,
            workspacePath: workspacePath ?? null,
        });
    } catch (err) {
        console.error('runPipelinePartial failed', err);
        return {
            status: 'error',
            duration_ms: 0,
            nodes: {},
            preview: [],
            error: String(err),
        };
    }
}

export type RunRecord = {
    at: string;
    status: string;
    duration_ms: number;
    rows: number;
    node_count: number;
    trigger: string;
    error?: string;
};

export async function runHistory(
    workspacePath: string,
    pipelineId: string,
): Promise<RunRecord[]> {
    if (!isTauri()) return [];
    try {
        return await invoke<RunRecord[]>('run_history', {
            workspacePath,
            pipelineId,
        });
    } catch (err) {
        console.warn('runHistory failed', err);
        return [];
    }
}

// ---- Engine install (first-run guided setup) ---------------------------

export type EngineStatus = {
    id: string;
    name: string;
    description: string;
    required: boolean;
    installed: boolean;
    version?: string;
    path?: string;
    available: boolean;
};

export type InstallProgress =
    | { phase: 'downloading'; received: number; total?: number }
    | { phase: 'extracting' }
    | { phase: 'verifying' }
    | { phase: 'done'; path: string }
    // Set by the frontend on a caught install error (the Rust command
    // returns Err rather than streaming this).
    | { phase: 'failed'; error: string };

export async function engineStatus(): Promise<EngineStatus[]> {
    if (!isTauri()) return [];
    try {
        return await invoke<EngineStatus[]>('engine_status');
    } catch (err) {
        console.warn('engineStatus failed', err);
        return [];
    }
}

export async function engineInstall(
    engine: string,
    onProgress?: (p: InstallProgress) => void,
): Promise<string> {
    const channel = new Channel<InstallProgress>();
    if (onProgress) channel.onmessage = onProgress;
    return await invoke<string>('engine_install', { engine, onProgress: channel });
}

export async function cancelPipeline(): Promise<void> {
    if (!isTauri()) return;
    try {
        await invoke('cancel_pipeline');
    } catch (err) {
        console.warn('cancelPipeline failed', err);
    }
}

export type StageSql = {
    node_id: string;
    label: string;
    kind: 'view' | 'sink';
    sql: string;
};

export async function compilePipelineSql(
    nodes: Node<DuckleNodeData>[],
    edges: Edge[],
): Promise<StageSql[] | null> {
    if (!isTauri()) return null;
    try {
        return await invoke<StageSql[]>('compile_pipeline', {
            pipeline: { nodes, edges },
        });
    } catch (err) {
        console.warn('compilePipelineSql failed', err);
        return null;
    }
}

// ---- Schedules ---------------------------------------------------------

export type ScheduleKind =
    | { type: 'cron'; expr: string }
    | { type: 'interval'; seconds: number }
    | { type: 'file_watch'; path: string; recursive: boolean };

export type Schedule = {
    id: string;
    pipeline_id: string;
    name: string;
    enabled: boolean;
    kind: ScheduleKind;
    last_run_at?: string;
    last_run_status?: 'ok' | 'error' | 'cancelled';
    last_run_duration_ms?: number;
    last_run_error?: string;
    next_run_at?: string;
};

export async function scheduleSetWorkspace(path: string | null): Promise<void> {
    if (!isTauri()) return;
    try {
        await invoke('schedule_set_workspace', { path: path ?? '' });
    } catch (err) {
        console.warn('scheduleSetWorkspace failed', err);
    }
}

export async function scheduleList(): Promise<Schedule[]> {
    if (!isTauri()) return [];
    try {
        return await invoke<Schedule[]>('schedule_list');
    } catch (err) {
        console.warn('scheduleList failed', err);
        return [];
    }
}

export async function scheduleUpsert(schedule: Schedule): Promise<Schedule | null> {
    if (!isTauri()) return null;
    return await invoke<Schedule>('schedule_upsert', { schedule });
}

export async function scheduleDelete(id: string): Promise<void> {
    if (!isTauri()) return;
    await invoke('schedule_delete', { id });
}

export async function scheduleRunNow(id: string): Promise<RunResult | null> {
    if (!isTauri()) return null;
    return await invoke<RunResult>('schedule_run_now', { id });
}
