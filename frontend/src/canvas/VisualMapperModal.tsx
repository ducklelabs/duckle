import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import {
    AlertCircle,
    ArrowRight,
    GripVertical,
    Plus,
    RotateCcw,
    Save,
    X,
    Zap,
} from 'lucide-react';
import type { Column, DataType, DuckleNodeData } from '../pipeline-types';
import { DATA_TYPES } from '../pipeline-types';
import type { Node, Edge } from '@xyflow/react';
import { resolveInputPortSchemas } from '../schema-resolve';

export type MappingRow = {
    id: string;
    name: string;
    type: DataType;
    expression: string;
};

export type JoinType = 'inner' | 'left';

export type LookupConfig = {
    port: string; // lookup_1 | lookup_2 | lookup_3
    leftKey: string; // column(s) on main; comma-separated for composite keys
    rightKey: string; // column(s) on the lookup; comma-separated for composite keys
    joinType: JoinType;
};

export type MapperState = {
    outputs: MappingRow[];
    filter?: string;
    lookups?: LookupConfig[];
};

type Props = {
    nodeId: string;
    nodeLabel: string;
    nodes: Node<DuckleNodeData>[];
    edges: Edge[];
    initialState: MapperState;
    onSave: (state: MapperState, derivedSchema: Column[]) => void;
    onCancel: () => void;
};

function newRowId(): string {
    return 'r_' + Date.now().toString(36) + '_' + Math.random().toString(36).slice(2, 6);
}

function refFor(portId: string, colName: string): string {
    return portId + '.' + colName;
}

const SQL_FUNCS: { label: string; insert: string }[] = [
    { label: 'COALESCE', insert: 'COALESCE($, NULL)' },
    { label: 'UPPER', insert: 'UPPER($)' },
    { label: 'LOWER', insert: 'LOWER($)' },
    { label: 'TRIM', insert: 'TRIM($)' },
    { label: 'LENGTH', insert: 'LENGTH($)' },
    { label: 'CAST', insert: 'CAST($ AS STRING)' },
    { label: 'NOW', insert: 'NOW()' },
    { label: 'CONCAT', insert: "CONCAT($, '')" },
    { label: 'CASE', insert: 'CASE WHEN $ THEN  ELSE  END' },
];

export default function VisualMapperModal({
    nodeId,
    nodeLabel,
    nodes,
    edges,
    initialState,
    onSave,
    onCancel,
}: Props) {
    const [outputs, setOutputs] = useState<MappingRow[]>(initialState.outputs);
    const [filter, setFilter] = useState<string>(initialState.filter ?? '');
    const [lookups, setLookups] = useState<LookupConfig[]>(initialState.lookups ?? []);
    const [focusedRow, setFocusedRow] = useState<string | null>(null);
    const exprRefs = useRef<Map<string, HTMLTextAreaElement>>(new Map());

    const inputPorts = useMemo(
        () => resolveInputPortSchemas(nodeId, nodes, edges),
        [nodeId, nodes, edges],
    );

    // Connected lookup ports (main is the driving input, never a lookup).
    const lookupPorts = useMemo(
        () => inputPorts.filter(p => p.portId.startsWith('lookup_')),
        [inputPorts],
    );
    const mainCols = useMemo(
        () => inputPorts.find(p => p.portId === 'main')?.schema ?? [],
        [inputPorts],
    );

    const lookupFor = useCallback(
        (port: string): LookupConfig =>
            lookups.find(l => l.port === port) ?? {
                port,
                leftKey: '',
                rightKey: '',
                joinType: 'left',
            },
        [lookups],
    );

    const updateLookup = useCallback((port: string, patch: Partial<LookupConfig>) => {
        setLookups(ls => {
            if (ls.some(l => l.port === port)) {
                return ls.map(l => (l.port === port ? { ...l, ...patch } : l));
            }
            return [...ls, { port, leftKey: '', rightKey: '', joinType: 'left', ...patch }];
        });
    }, []);

    useEffect(() => {
        const onKey = (e: KeyboardEvent) => {
            if (e.key === 'Escape') onCancel();
        };
        document.addEventListener('keydown', onKey);
        return () => document.removeEventListener('keydown', onKey);
    }, [onCancel]);

    const addRow = useCallback(() => {
        const next: MappingRow = {
            id: newRowId(),
            name: 'col_' + (outputs.length + 1),
            type: 'string',
            expression: '',
        };
        setOutputs(o => [...o, next]);
        setTimeout(() => exprRefs.current.get(next.id)?.focus(), 0);
    }, [outputs.length]);

    const removeRow = useCallback((id: string) => {
        setOutputs(o => o.filter(r => r.id !== id));
    }, []);

    const updateRow = useCallback((id: string, patch: Partial<MappingRow>) => {
        setOutputs(o => o.map(r => (r.id === id ? { ...r, ...patch } : r)));
    }, []);

    const inferAllFromInputs = useCallback(() => {
        const rows: MappingRow[] = [];
        const seen = new Set<string>();
        for (const port of inputPorts) {
            for (const col of port.schema) {
                const name = col.name;
                if (seen.has(name)) continue;
                seen.add(name);
                rows.push({
                    id: newRowId(),
                    name,
                    type: col.type,
                    expression: refFor(port.portId, col.name),
                });
            }
        }
        setOutputs(rows);
    }, [inputPorts]);

    const handleSave = useCallback(() => {
        const derived: Column[] = outputs.map(r => ({
            name: r.name || 'col',
            type: r.type,
            nullable: true,
        }));
        // Only persist lookups that are wired and have both join keys set, so
        // the engine never sees a half-configured join.
        const connected = new Set(lookupPorts.map(p => p.portId));
        const cleanLookups = lookups.filter(
            l => connected.has(l.port) && l.leftKey.trim() !== '' && l.rightKey.trim() !== '',
        );
        onSave(
            {
                outputs,
                filter: filter.trim() || undefined,
                lookups: cleanLookups.length ? cleanLookups : undefined,
            },
            derived,
        );
    }, [outputs, filter, lookups, lookupPorts, onSave]);

    const handleColumnDragStart = (
        e: React.DragEvent<HTMLDivElement>,
        portId: string,
        col: Column,
    ) => {
        const ref = refFor(portId, col.name);
        e.dataTransfer.setData('application/duckle-mapper-ref', ref);
        e.dataTransfer.setData('text/plain', ref);
        e.dataTransfer.effectAllowed = 'copy';
    };

    const handleExpressionDrop = (e: React.DragEvent<HTMLTextAreaElement>, rowId: string) => {
        const ref =
            e.dataTransfer.getData('application/duckle-mapper-ref') ||
            e.dataTransfer.getData('text/plain');
        if (!ref) return;
        e.preventDefault();
        const row = outputs.find(r => r.id === rowId);
        if (!row) return;
        const ta = e.currentTarget;
        const start = ta.selectionStart ?? row.expression.length;
        const end = ta.selectionEnd ?? row.expression.length;
        const next = row.expression.slice(0, start) + ref + row.expression.slice(end);
        updateRow(rowId, { expression: next });
        setTimeout(() => {
            const el = exprRefs.current.get(rowId);
            if (el) {
                el.focus();
                const newCursor = start + ref.length;
                el.setSelectionRange(newCursor, newCursor);
            }
        }, 0);
    };

    const handleExpressionDragOver = (e: React.DragEvent<HTMLTextAreaElement>) => {
        if (
            e.dataTransfer.types.includes('application/duckle-mapper-ref') ||
            e.dataTransfer.types.includes('text/plain')
        ) {
            e.preventDefault();
            e.dataTransfer.dropEffect = 'copy';
        }
    };

    const insertAtFocused = (snippet: string) => {
        if (!focusedRow) return;
        const row = outputs.find(r => r.id === focusedRow);
        if (!row) return;
        const ta = exprRefs.current.get(focusedRow);
        if (!ta) return;
        const start = ta.selectionStart ?? row.expression.length;
        const end = ta.selectionEnd ?? row.expression.length;
        const next = row.expression.slice(0, start) + snippet + row.expression.slice(end);
        updateRow(focusedRow, { expression: next });
        setTimeout(() => {
            ta.focus();
            const newCursor = start + snippet.length;
            ta.setSelectionRange(newCursor, newCursor);
        }, 0);
    };

    // Track which output rows reference which input columns (for highlighting)
    const refMap = useMemo(() => {
        const map = new Map<string, Set<string>>(); // input ref -> set of output row ids
        for (const row of outputs) {
            for (const port of inputPorts) {
                for (const col of port.schema) {
                    const ref = refFor(port.portId, col.name);
                    if (row.expression.includes(ref)) {
                        const set = map.get(ref) ?? new Set();
                        set.add(row.id);
                        map.set(ref, set);
                    }
                }
            }
        }
        return map;
    }, [outputs, inputPorts]);

    return createPortal(
        <div className="modal-backdrop">
            <div className="modal modal-mapper">
                <div className="modal-header modal-mapper-header">
                    <div>
                        <div className="modal-title">Visual Mapper</div>
                        <div className="modal-subtitle">{nodeLabel}  ·  #{nodeId.slice(0, 6)}</div>
                    </div>
                    <div className="modal-mapper-header-actions">
                        <button
                            type="button"
                            className="btn btn-secondary mapper-action"
                            onClick={inferAllFromInputs}
                            title="Generate output columns from all inputs"
                        >
                            <Zap size={14} />
                            Auto-map
                        </button>
                        <button
                            type="button"
                            className="btn btn-secondary mapper-action"
                            onClick={() => setOutputs([])}
                            title="Clear all output columns"
                        >
                            <RotateCcw size={14} />
                            Reset
                        </button>
                        <button
                            type="button"
                            className="modal-close"
                            onClick={onCancel}
                            aria-label="Close"
                        >
                            <X size={16} />
                        </button>
                    </div>
                </div>

                <div className="mapper-body">
                    {/* INPUTS panel */}
                    <div className="mapper-pane mapper-inputs-pane">
                        <div className="mapper-pane-header">
                            INPUTS
                            <span className="mapper-pane-count">
                                {inputPorts.reduce((a, p) => a + p.schema.length, 0)} columns
                            </span>
                        </div>
                        <div className="mapper-pane-body">
                            {inputPorts.length === 0 ? (
                                <div className="mapper-empty">
                                    <AlertCircle size={20} />
                                    <div>No upstream inputs.</div>
                                    <div className="mapper-empty-desc">
                                        Connect a source (main + optional lookups) to the Map node
                                        first.
                                    </div>
                                </div>
                            ) : (
                                inputPorts.map(port => (
                                    <div className="mapper-input-port" key={port.portId}>
                                        <div className="mapper-input-port-name">
                                            {port.portId}
                                            <span className="mapper-input-port-count">
                                                {port.schema.length}
                                            </span>
                                        </div>
                                        {port.schema.map(col => {
                                            const ref = refFor(port.portId, col.name);
                                            const used = refMap.has(ref);
                                            return (
                                                <div
                                                    key={col.name}
                                                    className={
                                                        'mapper-input-col' +
                                                        (used ? ' is-used' : '')
                                                    }
                                                    draggable
                                                    onDragStart={e =>
                                                        handleColumnDragStart(e, port.portId, col)
                                                    }
                                                    title={'Drag to expression: ' + ref}
                                                >
                                                    <GripVertical
                                                        size={11}
                                                        className="mapper-input-grip"
                                                    />
                                                    <span className="mapper-input-col-name">
                                                        {col.name}
                                                    </span>
                                                    <span className="mapper-input-col-type">
                                                        {col.type}
                                                    </span>
                                                    {used ? (
                                                        <ArrowRight
                                                            size={11}
                                                            className="mapper-input-used"
                                                        />
                                                    ) : null}
                                                </div>
                                            );
                                        })}
                                    </div>
                                ))
                            )}
                        </div>
                    </div>

                    {/* OUTPUT panel */}
                    <div className="mapper-pane mapper-output-pane">
                        <div className="mapper-pane-header">
                            OUTPUT
                            <span className="mapper-pane-count">
                                {outputs.length} column{outputs.length === 1 ? '' : 's'}
                            </span>
                            <span className="mapper-pane-spacer" />
                            <button
                                type="button"
                                className="mapper-add-row"
                                onClick={addRow}
                            >
                                <Plus size={12} /> Add column
                            </button>
                        </div>
                        <div className="mapper-pane-body">
                            {outputs.length === 0 ? (
                                <div className="mapper-empty">
                                    <div>No output columns yet.</div>
                                    <div className="mapper-empty-desc">
                                        Click <b>Add column</b>, or <b>Auto-map</b> to copy every
                                        upstream column into the output.
                                    </div>
                                </div>
                            ) : (
                                <div className="mapper-output-table">
                                    <div className="mapper-output-row mapper-output-header">
                                        <div className="mapper-cell-name">Name</div>
                                        <div className="mapper-cell-type">Type</div>
                                        <div className="mapper-cell-expr">Expression</div>
                                        <div className="mapper-cell-action" />
                                    </div>
                                    {outputs.map((row, i) => (
                                        <div
                                            key={row.id}
                                            className={
                                                'mapper-output-row' +
                                                (focusedRow === row.id ? ' is-focused' : '')
                                            }
                                        >
                                            <div className="mapper-cell-name">
                                                <input
                                                    type="text"
                                                    className="schema-input"
                                                    value={row.name}
                                                    onChange={e =>
                                                        updateRow(row.id, { name: e.target.value })
                                                    }
                                                    spellCheck={false}
                                                />
                                            </div>
                                            <div className="mapper-cell-type">
                                                <select
                                                    className="schema-input"
                                                    value={row.type}
                                                    onChange={e =>
                                                        updateRow(row.id, {
                                                            type: e.target.value as DataType,
                                                        })
                                                    }
                                                >
                                                    {DATA_TYPES.map(t => (
                                                        <option key={t} value={t}>
                                                            {t}
                                                        </option>
                                                    ))}
                                                </select>
                                            </div>
                                            <div className="mapper-cell-expr">
                                                <textarea
                                                    ref={el => {
                                                        if (el) exprRefs.current.set(row.id, el);
                                                        else exprRefs.current.delete(row.id);
                                                    }}
                                                    className="mapper-expression"
                                                    value={row.expression}
                                                    placeholder="drag input column or type expression"
                                                    onChange={e =>
                                                        updateRow(row.id, {
                                                            expression: e.target.value,
                                                        })
                                                    }
                                                    onFocus={() => setFocusedRow(row.id)}
                                                    onDragOver={handleExpressionDragOver}
                                                    onDrop={e => handleExpressionDrop(e, row.id)}
                                                    rows={1}
                                                    spellCheck={false}
                                                />
                                            </div>
                                            <div className="mapper-cell-action">
                                                <button
                                                    type="button"
                                                    className="schema-remove"
                                                    onClick={() => removeRow(row.id)}
                                                    aria-label={'Remove ' + row.name}
                                                    title={'Remove row ' + (i + 1)}
                                                >
                                                    <X size={12} />
                                                </button>
                                            </div>
                                        </div>
                                    ))}
                                </div>
                            )}
                        </div>
                    </div>

                    {/* HELPER panel */}
                    <div className="mapper-pane mapper-helper-pane">
                        <div className="mapper-pane-header">FUNCTIONS</div>
                        <div className="mapper-pane-body mapper-helper-body">
                            <div className="mapper-helper-hint">
                                Click to insert at cursor in the focused expression.
                            </div>
                            {SQL_FUNCS.map(f => (
                                <button
                                    type="button"
                                    key={f.label}
                                    className="mapper-func"
                                    onClick={() => insertAtFocused(f.insert)}
                                    disabled={!focusedRow}
                                >
                                    <code>{f.label}</code>
                                </button>
                            ))}
                            {lookupPorts.length > 0 ? (
                                <>
                                    <div className="mapper-helper-section">JOINS</div>
                                    {lookupPorts.map(port => {
                                        const cfg = lookupFor(port.portId);
                                        return (
                                            <div className="mapper-join" key={port.portId}>
                                                <div className="mapper-join-head">
                                                    <span className="mapper-join-port">
                                                        {port.portId}
                                                    </span>
                                                    <select
                                                        className="schema-input mapper-join-type"
                                                        value={cfg.joinType}
                                                        onChange={e =>
                                                            updateLookup(port.portId, {
                                                                joinType: e.target
                                                                    .value as JoinType,
                                                            })
                                                        }
                                                        title="Join type"
                                                    >
                                                        <option value="left">Left join</option>
                                                        <option value="inner">Inner join</option>
                                                    </select>
                                                </div>
                                                <div className="mapper-join-on">
                                                    <span className="mapper-join-side">main</span>
                                                    <input
                                                        className="schema-input mapper-join-key"
                                                        list={`mapper-main-cols`}
                                                        value={cfg.leftKey}
                                                        placeholder="key"
                                                        spellCheck={false}
                                                        onChange={e =>
                                                            updateLookup(port.portId, {
                                                                leftKey: e.target.value,
                                                            })
                                                        }
                                                    />
                                                    <span className="mapper-join-eq">=</span>
                                                    <span className="mapper-join-side">
                                                        {port.portId}
                                                    </span>
                                                    <input
                                                        className="schema-input mapper-join-key"
                                                        list={`mapper-cols-${port.portId}`}
                                                        value={cfg.rightKey}
                                                        placeholder="key"
                                                        spellCheck={false}
                                                        onChange={e =>
                                                            updateLookup(port.portId, {
                                                                rightKey: e.target.value,
                                                            })
                                                        }
                                                    />
                                                    <datalist id={`mapper-cols-${port.portId}`}>
                                                        {port.schema.map(c => (
                                                            <option key={c.name} value={c.name} />
                                                        ))}
                                                    </datalist>
                                                </div>
                                            </div>
                                        );
                                    })}
                                    <datalist id="mapper-main-cols">
                                        {mainCols.map(c => (
                                            <option key={c.name} value={c.name} />
                                        ))}
                                    </datalist>
                                    <div className="mapper-helper-hint">
                                        Reference lookup columns in expressions as
                                        <code> lookup_1.col</code>. Composite keys: comma-separate.
                                    </div>
                                </>
                            ) : null}
                            <div className="mapper-helper-section">FILTER</div>
                            <textarea
                                className="mapper-expression"
                                value={filter}
                                placeholder="optional WHERE clause"
                                onChange={e => setFilter(e.target.value)}
                                rows={3}
                                spellCheck={false}
                            />
                            <div className="mapper-helper-hint">
                                Applied after expressions; rows where this is false are dropped.
                            </div>
                        </div>
                    </div>
                </div>

                <div className="modal-footer modal-mapper-footer">
                    <div className="modal-mapper-status">
                        <span>{outputs.length} output column{outputs.length === 1 ? '' : 's'}</span>
                        <span>·</span>
                        <span>
                            {Array.from(refMap.keys()).length} input column
                            {refMap.size === 1 ? '' : 's'} referenced
                        </span>
                    </div>
                    <button type="button" className="btn btn-secondary" onClick={onCancel}>
                        Cancel
                    </button>
                    <button
                        type="button"
                        className="btn btn-primary"
                        onClick={handleSave}
                        disabled={outputs.length === 0}
                    >
                        <Save size={13} />
                        Apply mapping
                    </button>
                </div>
            </div>
        </div>,
        document.body,
    );
}
