import { useCallback, useEffect, useLayoutEffect, useState } from 'react';

// First-run guided tour: a spotlight walkthrough of the core surfaces. Anchors
// to [data-tour="..."] markers; if a marker is missing the step degrades to a
// centered card, so the tour never breaks. Dismissal persists to localStorage;
// re-launch by dispatching window event "duckle:start-tour".

// Bumped to v2 when the tour gained the Dives + column-lineage steps and the
// broader palette copy, so users who dismissed v1 see the new feature spotlights.
const SEEN_KEY = 'duckle.tour.v2.done';

type Placement = 'top' | 'bottom' | 'left' | 'right' | 'center';
interface Step {
    sel: string | null;
    title: string;
    body: string;
    placement?: Placement;
}

const STEPS: Step[] = [
    {
        sel: null,
        title: 'Welcome to Duckle',
        body: 'Your local-first DuckDB studio for building data pipelines - no servers, no JVM. This quick tour points out the essentials. You can skip it anytime.',
        placement: 'center',
    },
    {
        sel: '[data-tour="palette"]',
        title: 'Palette & project tree',
        body: 'Two tabs in this left panel. The component palette has 350+ blocks - databases, files, cloud + object stores, vector DBs (incl. LanceDB), data-quality & governance, AI, and code UDFs (Python, JavaScript) - drag any onto the canvas. The project tree browses your pipelines, connections and bundled examples. Tip: start typing on the canvas to quick-add a component.',
        placement: 'right',
    },
    {
        sel: '[data-tour="canvas"]',
        title: 'Pipeline canvas',
        body: 'Wire components together - connect a source to a transform to a sink. This visual graph is your pipeline.',
        placement: 'bottom',
    },
    {
        sel: '[data-tour="properties"]',
        title: 'Properties',
        body: 'Select any node to configure it here: connection details, columns, write modes (overwrite / append / upsert) and more.',
        placement: 'left',
    },
    {
        sel: '[data-tour="run"]',
        title: 'Run your pipeline',
        body: 'Execute the pipeline locally. The data preview, the compiled execution plan, and run logs appear in the panel below the canvas.',
        placement: 'bottom',
    },
    {
        sel: '[data-tour="dashboard"]',
        title: 'Web dashboard',
        body: 'Open the management console in your browser: run and monitor every pipeline grouped by job, with schedules and full run history.',
        placement: 'bottom',
    },
    {
        sel: '[data-tour="dives"]',
        title: 'Dives - live data views & dashboards',
        body: 'Explore your data with live, auto-charting views, then pin them into shareable dashboards - all local-first. A fast way to inspect results without leaving Duckle. (Hide this button from Settings if you prefer.)',
        placement: 'bottom',
    },
    {
        sel: '[data-tour="lineage"]',
        title: 'Column lineage',
        body: 'Trace every output column back through the pipeline to the source columns it came from - handy for audits and impact analysis before you change a transform.',
        placement: 'bottom',
    },
    {
        sel: '[data-tour="topbar"]',
        title: 'AI, Git, context & settings',
        body: 'Connect an AI assistant over MCP, manage version control, switch your context / environment, change language, and tune settings - all from the top bar.',
        placement: 'bottom',
    },
    {
        sel: null,
        title: "You're all set",
        body: 'Build your first pipeline, or open an example from the project tree. You can replay this tour anytime from Settings.',
        placement: 'center',
    },
];

interface Box {
    top: number;
    left: number;
    width: number;
    height: number;
}

export function GuidedTour() {
    const [active, setActive] = useState(false);
    const [i, setI] = useState(0);
    const [box, setBox] = useState<Box | null>(null);

    // Open on first run - but only once the workspace UI is actually mounted
    // (poll for the canvas anchor), so brand-new users still on the engine-setup
    // screen don't see a tour pointing at elements that don't exist yet.
    useEffect(() => {
        if (localStorage.getItem(SEEN_KEY)) return;
        let tries = 0;
        const iv = setInterval(() => {
            tries += 1;
            if (document.querySelector('[data-tour="canvas"]')) {
                clearInterval(iv);
                setActive(true);
            } else if (tries > 40) {
                clearInterval(iv);
            }
        }, 600);
        return () => clearInterval(iv);
    }, []);
    useEffect(() => {
        const start = () => {
            setI(0);
            setActive(true);
        };
        window.addEventListener('duckle:start-tour', start);
        return () => window.removeEventListener('duckle:start-tour', start);
    }, []);

    const measure = useCallback(() => {
        const step = STEPS[i];
        if (!step?.sel) {
            setBox(null);
            return;
        }
        const el = document.querySelector(step.sel) as HTMLElement | null;
        if (!el) {
            setBox(null);
            return;
        }
        const r = el.getBoundingClientRect();
        if (r.width === 0 && r.height === 0) {
            setBox(null);
            return;
        }
        setBox({ top: r.top, left: r.left, width: r.width, height: r.height });
    }, [i]);

    useLayoutEffect(() => {
        if (!active) return;
        measure();
        window.addEventListener('resize', measure);
        window.addEventListener('scroll', measure, true);
        return () => {
            window.removeEventListener('resize', measure);
            window.removeEventListener('scroll', measure, true);
        };
    }, [active, measure]);

    if (!active) return null;

    const step = STEPS[i];
    const last = i === STEPS.length - 1;
    const close = () => {
        localStorage.setItem(SEEN_KEY, '1');
        setActive(false);
    };
    const next = () => (last ? close() : setI((n) => n + 1));
    const back = () => setI((n) => Math.max(0, n - 1));

    // Tooltip position: anchored beside the spotlight, then ALWAYS clamped into
    // the viewport so the card (and its Skip/Back/Next buttons) is reachable even
    // when the target fills the screen (e.g. the canvas). Very large targets get
    // a centered card since "beside" has no room.
    const PAD = 10;
    const TIP_W = 340;
    const TIP_H = 280; // generous estimate used only for clamping
    const vh = window.innerHeight;
    const vw = window.innerWidth;
    let tipStyle: React.CSSProperties;
    const big = !!box && box.height > vh * 0.7 && box.width > vw * 0.45;
    if (!box || big) {
        tipStyle = { top: '50%', left: '50%', transform: 'translate(-50%,-50%)' };
    } else {
        const place = step.placement ?? 'bottom';
        let top: number;
        let left: number;
        if (place === 'right' && box.left + box.width + TIP_W + 24 < vw) {
            left = box.left + box.width + PAD;
            top = box.top;
        } else if (place === 'left' && box.left - TIP_W - 24 > 0) {
            left = box.left - TIP_W - PAD;
            top = box.top;
        } else if (place === 'top' && box.top - TIP_H - PAD > 0) {
            top = box.top - TIP_H - PAD;
            left = box.left;
        } else {
            // bottom (default); if it would overflow, flip above the target
            top = box.top + box.height + PAD;
            left = box.left;
            if (top + TIP_H + 12 > vh && box.top - TIP_H - PAD > 0) {
                top = box.top - TIP_H - PAD;
            }
        }
        // Final guard: keep the whole card on screen.
        top = Math.max(12, Math.min(top, vh - TIP_H - 12));
        left = Math.max(12, Math.min(left, vw - TIP_W - 12));
        tipStyle = { top, left };
    }

    return (
        <div className="tour-root" role="dialog" aria-modal="true" aria-label="Duckle guided tour">
            {/* Spotlight: a transparent box with a huge shadow dims everything else. */}
            {box ? (
                <div
                    className="tour-spotlight"
                    style={{
                        top: box.top - PAD,
                        left: box.left - PAD,
                        width: box.width + PAD * 2,
                        height: box.height + PAD * 2,
                    }}
                />
            ) : (
                <div className="tour-dim" onClick={close} />
            )}
            <div className="tour-tip" style={{ ...tipStyle, width: TIP_W }}>
                <div className="tour-progress">
                    Step {i + 1} of {STEPS.length}
                </div>
                <h3 className="tour-title">{step.title}</h3>
                <p className="tour-body">{step.body}</p>
                <div className="tour-dots">
                    {STEPS.map((_, n) => (
                        <span key={n} className={n === i ? 'tour-dot on' : 'tour-dot'} />
                    ))}
                </div>
                <div className="tour-actions">
                    <button type="button" className="tour-skip" onClick={close}>
                        Skip tour
                    </button>
                    <div className="tour-nav">
                        {i > 0 ? (
                            <button type="button" className="tour-btn" onClick={back}>
                                Back
                            </button>
                        ) : null}
                        <button type="button" className="tour-btn primary" onClick={next}>
                            {last ? 'Get started' : 'Next'}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
}
