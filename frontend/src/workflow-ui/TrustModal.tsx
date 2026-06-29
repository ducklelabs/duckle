// Trust scorecard viewer. Calls the engine's pipeline_trust_report for the
// active pipeline and shows an explainable 0-100 score where every lost point
// is an itemized finding (compile status, structural risks, ungoverned PII).
// Read-only. Static by default; the "live schema drift" toggle additionally
// reads each source's live schema and folds breaking drift into the score.
// The same scorecard is what the `duckle review`/`trust_report` MCP tool and the
// CLI report, so the editor agrees with the agent-facing surfaces.

import { useEffect, useState } from 'react';
import { X } from 'lucide-react';
import type { Edge, Node } from '@xyflow/react';
import type { DuckleNodeData } from '../pipeline-types';
import { pipelineTrustReport, type DriftReport, type DriftSource, type TrustReport } from '../tauri-bridge';

interface TrustModalProps {
    nodes: Node<DuckleNodeData>[];
    edges: Edge[];
    workspacePath?: string | null;
    onClose: () => void;
}

// Grade -> badge class. Brand rule: a good score is maya (no green).
function gradeClass(grade: string): string {
    if (grade === 'A' || grade === 'B') return 'trust-grade-good';
    if (grade === 'C') return 'trust-grade-warn';
    return 'trust-grade-bad';
}

// Per-source drift status -> badge class + label.
function driftStatus(s: DriftSource): { cls: string; label: string } {
    switch (s.status) {
        case 'match':
            return { cls: 'trust-info', label: 'match' };
        case 'drift':
            return s.breaking
                ? { cls: 'trust-error', label: 'breaking drift' }
                : { cls: 'trust-warning', label: 'drift' };
        case 'unreadable':
            return { cls: 'trust-warning', label: 'unreadable' };
        default:
            return { cls: 'trust-muted', label: s.status.replace(/_/g, ' ') };
    }
}

function DriftPanel({ drift }: { drift: DriftReport }) {
    const sum = drift.summary;
    const extras: string[] = [];
    if (sum.notIntrospectable > 0) extras.push(`${sum.notIntrospectable} not introspectable`);
    if (sum.unreadable > 0) extras.push(`${sum.unreadable} unreadable`);
    if (sum.noDeclaredSchema > 0) extras.push(`${sum.noDeclaredSchema} without a declared schema`);
    return (
        <div className="trust-drift">
            <div className="trust-drift-head">
                Live schema drift: checked {sum.sourcesChecked} source(s), {sum.sourcesWithDrift} drifted,{' '}
                {sum.breakingSources} breaking
                {extras.length > 0 ? <span className="trust-drift-extra"> ({extras.join(', ')})</span> : null}
            </div>
            {drift.sources.length === 0 ? (
                <div className="dive-panel-msg">No sources to check.</div>
            ) : (
                <ul className="trust-drift-list">
                    {drift.sources.map((s, i) => {
                        const st = driftStatus(s);
                        return (
                            <li key={i} className={`trust-drift-src ${st.cls}`}>
                                <span className="trust-dot" aria-hidden="true" />
                                <span className="trust-drift-body">
                                    <span className="trust-drift-name">
                                        {s.label || s.nodeId}
                                        <span className="trust-drift-status">{st.label}</span>
                                    </span>
                                    {s.note ? <span className="trust-drift-note">{s.note}</span> : null}
                                    {s.missingColumns && s.missingColumns.length > 0 ? (
                                        <span className="trust-drift-line trust-drift-missing">
                                            missing: {s.missingColumns.join(', ')}
                                        </span>
                                    ) : null}
                                    {s.addedColumns && s.addedColumns.length > 0 ? (
                                        <span className="trust-drift-line trust-drift-added">
                                            added: {s.addedColumns.join(', ')}
                                        </span>
                                    ) : null}
                                    {s.typeChanges && s.typeChanges.length > 0 ? (
                                        <span className="trust-drift-line trust-drift-types">
                                            type changes:{' '}
                                            {s.typeChanges
                                                .map((t) => `${t.column} ${t.declared} -> ${t.live}`)
                                                .join(', ')}
                                        </span>
                                    ) : null}
                                </span>
                            </li>
                        );
                    })}
                </ul>
            )}
        </div>
    );
}

export function TrustModal({ nodes, edges, workspacePath, onClose }: TrustModalProps) {
    const [data, setData] = useState<TrustReport | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [loading, setLoading] = useState(true);
    const [checkDrift, setCheckDrift] = useState(false);

    useEffect(() => {
        let cancelled = false;
        setLoading(true);
        setError(null);
        void (async () => {
            try {
                const r = await pipelineTrustReport(nodes, edges, checkDrift, workspacePath ?? null);
                if (!cancelled) {
                    setData(r);
                    setLoading(false);
                }
            } catch (e) {
                if (!cancelled) {
                    setError(e instanceof Error ? e.message : String(e));
                    setLoading(false);
                }
            }
        })();
        return () => {
            cancelled = true;
        };
    }, [nodes, edges, checkDrift, workspacePath]);

    return (
        <div className="dive-modal-backdrop" onClick={onClose}>
            <div className="trust-modal" onClick={(e) => e.stopPropagation()}>
                <div className="lineage-head">
                    <h2 className="lineage-title">Trust score</h2>
                    <button type="button" className="dive-btn" onClick={onClose} aria-label="Close">
                        <X size={16} />
                    </button>
                </div>
                <p className="lineage-sub">
                    An explainable score for this pipeline. Every lost point is a finding below, so
                    you can see exactly what to fix.
                </p>

                <label className="schedule-toggle trust-drift-toggle">
                    <input
                        type="checkbox"
                        checked={checkDrift}
                        onChange={(e) => setCheckDrift(e.target.checked)}
                    />
                    Check live schema drift (reads each source)
                </label>

                {loading ? (
                    <div className="dive-panel-msg">
                        {checkDrift ? 'Scoring pipeline and reading sources...' : 'Scoring pipeline...'}
                    </div>
                ) : null}
                {error ? <div className="dive-panel-msg dive-panel-err">{error}</div> : null}

                {!loading && !error && data ? (
                    <>
                        <div className="trust-scoreline">
                            <div className={`trust-score ${gradeClass(data.grade)}`}>
                                <span className="trust-score-num">{data.score}</span>
                                <span className="trust-score-max">/100</span>
                            </div>
                            <div className={`trust-grade ${gradeClass(data.grade)}`}>{data.grade}</div>
                            <div className="trust-score-meta">
                                <div className="trust-summary">{data.summary}</div>
                                <div className="trust-compile">
                                    {data.compiles ? 'Compiles' : 'Does not compile'}
                                </div>
                            </div>
                        </div>

                        {data.findings.length === 0 ? (
                            <div className="dive-panel-msg trust-clean">
                                No issues found. This pipeline scores a clean {data.score}/100.
                            </div>
                        ) : (
                            <ul className="trust-findings">
                                {data.findings.map((f, i) => (
                                    <li key={i} className={`trust-finding trust-${f.severity}`}>
                                        <span className="trust-dot" aria-hidden="true" />
                                        <span className="trust-finding-body">
                                            <span className="trust-finding-code">{f.code}</span>
                                            <span className="trust-finding-msg">{f.message}</span>
                                        </span>
                                        {f.deduction > 0 ? (
                                            <span className="trust-deduction">-{f.deduction}</span>
                                        ) : null}
                                    </li>
                                ))}
                            </ul>
                        )}

                        {checkDrift && data.drift ? <DriftPanel drift={data.drift} /> : null}
                    </>
                ) : null}
            </div>
        </div>
    );
}
