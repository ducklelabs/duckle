import { useEffect, useState } from 'react';
import {
    engineInstall,
    engineStatus,
    type EngineStatus,
    type InstallProgress,
} from './tauri-bridge';

/**
 * Non-blocking "a newer DuckDB engine is available" bar. Existing users whose
 * installed DuckDB is older than the version this Duckle build pins keep working
 * on the old engine (so we never block them with the install modal), but this
 * prompts a one-click in-place upgrade: it re-downloads the pinned version,
 * overwrites the engine in place, and the backend drops the previous version's
 * cached cross-OS binaries from the app storage directory. Dismissible per
 * session. No-op in the browser build or when the engine is already current.
 */
export function EngineUpgradeBanner() {
    const [duck, setDuck] = useState<EngineStatus | null>(null);
    const [dismissed, setDismissed] = useState(false);
    const [progress, setProgress] = useState<InstallProgress | null>(null);
    const [upgrading, setUpgrading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [done, setDone] = useState(false);

    useEffect(() => {
        // engineStatus() returns [] in the browser build, so no extra guard is
        // needed - the banner simply never shows there.
        let cancelled = false;
        void engineStatus().then(list => {
            if (cancelled) return;
            const d = list.find(e => e.id === 'duckdb');
            if (d && d.outdated) setDuck(d);
        });
        return () => {
            cancelled = true;
        };
    }, []);

    if (!duck || dismissed) return null;

    const progressLabel = (p: InstallProgress): string => {
        switch (p.phase) {
            case 'downloading':
                return p.total
                    ? `Downloading ${Math.round((p.received / p.total) * 100)}%`
                    : `Downloading ${Math.round(p.received / 1_000_000)} MB`;
            case 'extracting':
                return 'Extracting...';
            case 'verifying':
                return 'Verifying...';
            case 'installing_extension':
                return `Installing extensions (${p.index}/${p.total})`;
            case 'downloading_model':
                return 'Downloading...';
            case 'done':
                return 'Done';
            case 'failed':
                return 'Failed';
        }
    };

    const runUpgrade = async () => {
        setUpgrading(true);
        setError(null);
        setProgress({ phase: 'downloading', received: 0 });
        try {
            await engineInstall('duckdb', p => setProgress(p));
            setDone(true);
        } catch (e) {
            const msg = e instanceof Error ? e.message : String(e);
            setError(msg || 'Upgrade failed.');
            setUpgrading(false);
        }
    };

    return (
        <div className="update-banner" role="status">
            <span className="update-banner-icon" aria-hidden="true">
                ⬆
            </span>
            <span className="update-banner-text">
                {done ? (
                    <>DuckDB engine upgraded to {duck.target_version}.</>
                ) : upgrading && progress ? (
                    progressLabel(progress)
                ) : error ? (
                    <>Engine upgrade failed: {error}</>
                ) : (
                    <>
                        DuckDB engine {duck.target_version} is available
                        {duck.version ? ` (you're on ${duck.version})` : ''}. Upgrade to keep the
                        engine current.
                    </>
                )}
            </span>
            {!upgrading && !done ? (
                <button
                    type="button"
                    className="update-banner-cta"
                    onClick={() => void runUpgrade()}
                >
                    Upgrade now
                </button>
            ) : null}
            {!upgrading || done ? (
                <button
                    type="button"
                    className="update-banner-dismiss"
                    aria-label="Dismiss"
                    title="Dismiss"
                    onClick={() => setDismissed(true)}
                >
                    ×
                </button>
            ) : null}
        </div>
    );
}
