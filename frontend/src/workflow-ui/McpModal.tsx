import { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import { Bot, Check, Clipboard, Loader2, X } from 'lucide-react';
import { copyText } from '../tauri-io';
import { mcpConnectionInfo, connectClaudeCode, type McpConnInfo } from '../tauri-bridge';

/**
 * Compact popup that connects Duckle to an MCP-capable AI (Claude Code,
 * Claude Desktop, Cursor, etc.). It surfaces the bundled duckle-mcp server
 * with the real resolved paths filled in: a one-click "Connect to Claude Code"
 * button, plus copyable command + config for any other client.
 */
export function McpModal({ onClose }: { onClose: () => void }) {
    const [info, setInfo] = useState<McpConnInfo | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [copied, setCopied] = useState<string | null>(null);
    const [connecting, setConnecting] = useState(false);
    const [connectMsg, setConnectMsg] = useState<{ ok: boolean; text: string } | null>(null);

    useEffect(() => {
        let alive = true;
        mcpConnectionInfo()
            .then(i => { if (alive) setInfo(i); })
            .catch(e => { if (alive) setError(String(e)); });
        return () => { alive = false; };
    }, []);

    const copy = async (key: string, text: string) => {
        if (await copyText(text)) {
            setCopied(key);
            setTimeout(() => setCopied(c => (c === key ? null : c)), 1500);
        }
    };

    const connect = async () => {
        setConnecting(true);
        setConnectMsg(null);
        try {
            const out = await connectClaudeCode();
            setConnectMsg({ ok: true, text: out || 'Connected. Restart Claude Code if it is open.' });
        } catch (e) {
            setConnectMsg({ ok: false, text: String(e) });
        } finally {
            setConnecting(false);
        }
    };

    const handleBackdrop = (e: React.MouseEvent) => {
        if (e.target === e.currentTarget) onClose();
    };

    return createPortal(
        <div className="modal-backdrop" onClick={handleBackdrop}>
            <div className="modal mcp-modal" role="dialog" aria-modal="true" aria-label="Connect to AI">
                <div className="modal-header">
                    <div className="modal-title">
                        <Bot size={16} style={{ verticalAlign: '-3px', marginRight: 6 }} />
                        Connect Duckle to your AI
                    </div>
                    <button type="button" className="modal-close" onClick={onClose} aria-label="Close">
                        <X size={16} />
                    </button>
                </div>

                <div className="modal-body">
                    {error ? (
                        <p className="mcp-warn">Could not load MCP details: {error}</p>
                    ) : !info ? (
                        <p className="mcp-muted"><Loader2 size={14} className="spin" /> Preparing the MCP server…</p>
                    ) : (
                        <>
                            <p className="mcp-muted">
                                Duckle ships a Model Context Protocol server, so an AI assistant can
                                generate, validate, run and build your pipelines for you - in this
                                workspace.
                            </p>

                            {!info.bundled && (
                                <p className="mcp-warn">
                                    This build does not bundle the MCP server. Build it with
                                    <code> cargo build -p duckle-mcp --release</code> and point your client at it.
                                </p>
                            )}
                            {info.bundled && !info.duckdbFound && (
                                <p className="mcp-warn">
                                    The DuckDB engine is not installed yet. The AI can still generate and
                                    validate pipelines; install the engine (setup screen) to run or build them.
                                </p>
                            )}

                            {/* Claude Code: one-click */}
                            <div className="mcp-section">
                                <div className="mcp-section-title">Claude Code</div>
                                <button
                                    type="button"
                                    className="btn btn-primary"
                                    onClick={() => void connect()}
                                    disabled={!info.bundled || connecting}
                                >
                                    {connecting ? <><Loader2 size={13} className="spin" /> Connecting…</> : 'Connect to Claude Code'}
                                </button>
                                {connectMsg && (
                                    <p className={connectMsg.ok ? 'mcp-ok' : 'mcp-warn'}>{connectMsg.text}</p>
                                )}
                                <div className="mcp-code-row">
                                    <code className="mcp-code">{info.claudeCommand}</code>
                                    <button type="button" className="btn mcp-copy" onClick={() => void copy('cmd', info.claudeCommand)}>
                                        {copied === 'cmd' ? <><Check size={12} /> Copied</> : <><Clipboard size={12} /> Copy</>}
                                    </button>
                                </div>
                                <p className="mcp-hint">Or paste that command in a terminal.</p>
                            </div>

                            {/* Other clients: config JSON */}
                            <div className="mcp-section">
                                <div className="mcp-section-title">Claude Desktop, Cursor, or any MCP client</div>
                                <p className="mcp-hint">Add this to the client's MCP servers config:</p>
                                <div className="mcp-code-row">
                                    <pre className="mcp-code mcp-pre">{info.configJson}</pre>
                                    <button type="button" className="btn mcp-copy" onClick={() => void copy('json', info.configJson)}>
                                        {copied === 'json' ? <><Check size={12} /> Copied</> : <><Clipboard size={12} /> Copy</>}
                                    </button>
                                </div>
                            </div>

                            <details className="mcp-paths">
                                <summary>Resolved paths</summary>
                                <div className="mcp-kv"><span>MCP server</span><code>{info.mcpPath || '(not bundled)'}</code></div>
                                <div className="mcp-kv"><span>DuckDB</span><code>{info.duckdbPath || '(not installed)'}</code></div>
                                <div className="mcp-kv"><span>Runner</span><code>{info.runnerPath || '(not bundled)'}</code></div>
                            </details>
                        </>
                    )}
                </div>

                <div className="modal-footer">
                    <button type="button" className="btn" onClick={onClose}>Done</button>
                </div>
            </div>
        </div>,
        document.body,
    );
}
