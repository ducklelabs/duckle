import type { Node } from '@xyflow/react';
import type { DuckleNodeData } from './pipeline-types';
import type { ContextPayload, RepoItem, RoutinePayload } from './repo-types';

/**
 * Resolve a pipeline's nodes for execution:
 *   1. Inline a referenced SQL routine into Custom-SQL nodes.
 *   2. Substitute `${var}` / `${context.var}` references in field values
 *      with the workspace's context variables.
 *   3. Map a child-pipeline reference (Run Job / Iterate / Foreach / Try)
 *      stored as a workspace pipeline id to its on-disk file path, which
 *      is what the engine reads.
 *
 * Run on the working nodes right before they're sent to the engine, so
 * the canvas keeps the un-substituted, editable values.
 */

// Props that hold a reference to another pipeline the engine will read
// from disk. The dropdown stores a portable pipeline id; the engine needs
// a file path, so we resolve here at run time.
const PIPELINE_REF_KEYS = [
    'pipelineRef',
    'iteratePipelineRef',
    'foreachPipelineRef',
    'fallbackPipelineRef',
];

function joinPath(dir: string, ...parts: string[]): string {
    const sep = dir.includes('\\') && !dir.includes('/') ? '\\' : '/';
    return [dir.replace(/[/\\]+$/, ''), ...parts].join(sep);
}

export function buildContextVars(repo: RepoItem[]): Record<string, string> {
    const out: Record<string, string> = {};
    for (const item of repo) {
        if (item.type !== 'context') continue;
        const payload = item.payload as ContextPayload | undefined;
        if (!payload?.variables) continue;
        for (const v of payload.variables) {
            // Both the bare key and a context-namespaced key resolve.
            out[v.key] = v.value;
            out[`${item.name}.${v.key}`] = v.value;
        }
    }
    return out;
}

function substituteString(value: string, vars: Record<string, string>): string {
    return value.replace(/\$\{([^}]+)\}/g, (match, expr) => {
        const key = String(expr).trim();
        return Object.prototype.hasOwnProperty.call(vars, key) ? vars[key]! : match;
    });
}

export function substituteDeep(value: unknown, vars: Record<string, string>): unknown {
    if (typeof value === 'string') return substituteString(value, vars);
    if (Array.isArray(value)) return value.map(v => substituteDeep(v, vars));
    if (value && typeof value === 'object') {
        const out: Record<string, unknown> = {};
        for (const [k, v] of Object.entries(value)) out[k] = substituteDeep(v, vars);
        return out;
    }
    return value;
}

export function resolveForRun(
    nodes: Node<DuckleNodeData>[],
    repo: RepoItem[],
    workspacePath?: string | null,
): Node<DuckleNodeData>[] {
    const vars = buildContextVars(repo);
    const sqlRoutines = new Map<string, string>();
    // Map a workspace pipeline id (or name) to its on-disk file path so a
    // dropdown-stored id resolves to something the engine can read.
    const pipelinePaths = new Map<string, string>();
    for (const item of repo) {
        if (item.type === 'routine') {
            const payload = item.payload as RoutinePayload | undefined;
            if (payload?.language === 'sql' && payload.code) {
                sqlRoutines.set(item.id, payload.code);
                sqlRoutines.set(item.name, payload.code);
            }
        } else if (item.type === 'pipeline' && workspacePath) {
            const file = joinPath(workspacePath, 'pipelines', `${item.id}.json`);
            pipelinePaths.set(item.id, file);
            pipelinePaths.set(item.name, file);
        }
    }
    const hasVars = Object.keys(vars).length > 0;

    return nodes.map(node => {
        const props = { ...(node.data.properties ?? {}) } as Record<string, unknown>;

        // Inline a referenced SQL routine when there's no inline SQL.
        if (node.data.componentId === 'code.sql' || node.data.componentId === 'code.sqltemplate') {
            const ref = typeof props.routineRef === 'string' ? props.routineRef : '';
            const inline = typeof props.sql === 'string' ? props.sql.trim() : '';
            if (ref && !inline && sqlRoutines.has(ref)) {
                props.sql = sqlRoutines.get(ref);
            }
        }

        const resolved = hasVars
            ? (substituteDeep(props, vars) as Record<string, unknown>)
            : props;

        // Resolve child-pipeline ids to file paths. A value that isn't a
        // known pipeline id/name (a hand-typed literal path from before the
        // picker existed) is left untouched.
        if (pipelinePaths.size > 0) {
            for (const key of PIPELINE_REF_KEYS) {
                const v = resolved[key];
                if (typeof v === 'string' && pipelinePaths.has(v)) {
                    resolved[key] = pipelinePaths.get(v);
                }
            }
        }

        return { ...node, data: { ...node.data, properties: resolved } };
    });
}
