import { PlayCircle, CheckCircle2, XCircle, MinusCircle } from 'lucide-react';
import type { RunResult, NodeRunStatus } from '../tauri-bridge';

type Props = {
    runResult: RunResult | null;
    isRunning: boolean;
    nodeLabels: Record<string, string>;
};

function StatusIcon({ status }: { status: NodeRunStatus['status'] }) {
    if (status === 'ok') return <CheckCircle2 size={13} className="run-row-ok" />;
    if (status === 'error') return <XCircle size={13} className="run-row-err" />;
    return <MinusCircle size={13} className="run-row-idle" />;
}

// Per-node result of the most recent run: status, row count, duration,
// and any error. Backed by the RunResult the engine returns (and the
// streamed stage_finished events App folds into it live during a run).
export default function RunView({ runResult, isRunning, nodeLabels }: Props) {
    if (!runResult) {
        return (
            <div className="empty-state">
                <PlayCircle size={32} strokeWidth={1.4} className="empty-icon" />
                <div className="empty-title">Run output</div>
                <div className="empty-desc">
                    Per-node row counts, timings, and errors from the last run will appear here.
                    Press Run to execute the pipeline.
                </div>
            </div>
        );
    }

    const entries = Object.entries(runResult.nodes);

    return (
        <div className="run-view">
            <div className={`run-summary run-summary-${runResult.status}`}>
                <span className="run-summary-status">
                    {isRunning ? 'running' : runResult.status}
                </span>
                <span className="run-summary-meta">
                    {entries.length} node{entries.length === 1 ? '' : 's'} ·{' '}
                    {runResult.duration_ms} ms
                </span>
            </div>
            {runResult.error ? (
                <pre className="run-error-body">
                    {runResult.category ? (
                        <span className="run-error-category">{runResult.category}</span>
                    ) : null}
                    {runResult.error}
                </pre>
            ) : null}
            {runResult.messages && runResult.messages.length > 0 ? (
                <ul className="run-messages">
                    {runResult.messages.map((m, i) => (
                        <li key={i} className={`run-message run-message-${m.level}`}>
                            <span className="run-message-level">{m.level}</span>
                            <span className="run-message-node">
                                {nodeLabels[m.node_id] ?? m.node_id}
                            </span>
                            <span className="run-message-text">{m.message}</span>
                        </li>
                    ))}
                </ul>
            ) : null}
            <table className="run-table">
                <thead>
                    <tr>
                        <th></th>
                        <th>Node</th>
                        <th>Kind</th>
                        <th className="run-num">Rows</th>
                        <th className="run-num">Time</th>
                    </tr>
                </thead>
                <tbody>
                    {entries.map(([nodeId, st]) => (
                        <tr key={nodeId} className={`run-row run-row-${st.status}`}>
                            <td>
                                <StatusIcon status={st.status} />
                            </td>
                            <td>
                                <div className="run-node-label">
                                    {nodeLabels[nodeId] ?? nodeId}
                                </div>
                                {st.error ? (
                                    <div className="run-node-error">
                                        {st.category ? (
                                            <span className="run-error-category">
                                                {st.category}
                                            </span>
                                        ) : null}
                                        {st.error}
                                    </div>
                                ) : null}
                                {st.sql ? (
                                    <details className="run-node-sql">
                                        <summary>Failing SQL</summary>
                                        <pre>{st.sql}</pre>
                                    </details>
                                ) : null}
                            </td>
                            <td>{st.kind ?? ''}</td>
                            <td className="run-num">
                                {st.rows != null ? st.rows.toLocaleString() : '-'}
                            </td>
                            <td className="run-num">
                                {st.duration_ms != null ? `${st.duration_ms} ms` : '-'}
                            </td>
                        </tr>
                    ))}
                </tbody>
            </table>
        </div>
    );
}
