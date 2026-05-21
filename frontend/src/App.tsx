import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import Canvas from './canvas/Canvas';

type RuntimeState = 'connecting' | 'ready' | 'offline';

export default function App() {
    const [runtime, setRuntime] = useState<RuntimeState>('connecting');

    useEffect(() => {
        let cancelled = false;
        invoke<string>('ping')
            .then(reply => {
                if (!cancelled) setRuntime(reply === 'pong' ? 'ready' : 'offline');
            })
            .catch(() => {
                if (!cancelled) setRuntime('offline');
            });
        return () => {
            cancelled = true;
        };
    }, []);

    return (
        <div className="app">
            <header className="topbar">
                <div className="brand">
                    <span className="brand-mark">◇</span> Duckle
                </div>
                <div className="topbar-spacer" />
                <div className="status" data-state={runtime}>
                    <span className="status-dot" /> runtime: {runtime}
                </div>
            </header>

            <main className="workspace">
                <aside className="sidebar">
                    <div className="sidebar-section">
                        <div className="sidebar-title">Sources</div>
                        <div className="palette-item">CSV</div>
                        <div className="palette-item">Parquet</div>
                        <div className="palette-item">SQLite</div>
                    </div>
                    <div className="sidebar-section">
                        <div className="sidebar-title">Transforms</div>
                        <div className="palette-item">Filter</div>
                        <div className="palette-item">Project</div>
                        <div className="palette-item">Join</div>
                        <div className="palette-item">Aggregate</div>
                    </div>
                    <div className="sidebar-section">
                        <div className="sidebar-title">Sinks</div>
                        <div className="palette-item">CSV</div>
                        <div className="palette-item">Parquet</div>
                        <div className="palette-item">SQLite</div>
                    </div>
                </aside>

                <section className="canvas-shell">
                    <Canvas />
                </section>
            </main>
        </div>
    );
}
