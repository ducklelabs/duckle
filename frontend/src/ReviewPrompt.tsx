import { useEffect, useState } from 'react';
import { openExternal } from './tauri-io';

/**
 * After ~30s of use, a small bottom-right card invites the user to review Duckle
 * on SourceForge. Three choices:
 *   - Review          -> opens the SourceForge reviews page, never asks again
 *   - Remind me later -> hides for this session, asks again on the next launch
 *   - No thanks       -> never asks again
 *
 * Permanent decisions persist in localStorage; "remind me later" uses
 * sessionStorage so it only suppresses the current session (a fresh launch
 * starts a new session and the prompt returns). Shared by the desktop app and
 * the self-hosted web editor.
 *
 * Preview: set localStorage 'duckle.previewReviewPrompt' = '1' to force it on
 * immediately (for UI review without waiting / clearing state).
 */
const REVIEW_URL = 'https://sourceforge.net/projects/duckle/reviews/';
const STATE_KEY = 'duckle.reviewPrompt'; // localStorage: 'done' | 'dismissed'
const SNOOZE_KEY = 'duckle.reviewPrompt.snooze'; // sessionStorage: '1'
const DELAY_MS = 30_000;

export function ReviewPrompt() {
    const [show, setShow] = useState(
        typeof window !== 'undefined' &&
            window.localStorage?.getItem('duckle.previewReviewPrompt') === '1',
    );

    useEffect(() => {
        const settled =
            window.localStorage?.getItem(STATE_KEY) === 'done' ||
            window.localStorage?.getItem(STATE_KEY) === 'dismissed';
        const snoozed = window.sessionStorage?.getItem(SNOOZE_KEY) === '1';
        if (settled || snoozed) return;
        const timer = setTimeout(() => setShow(true), DELAY_MS);
        return () => clearTimeout(timer);
    }, []);

    if (!show) return null;

    const review = () => {
        void openExternal(REVIEW_URL);
        window.localStorage?.setItem(STATE_KEY, 'done');
        setShow(false);
    };
    const later = () => {
        window.sessionStorage?.setItem(SNOOZE_KEY, '1');
        setShow(false);
    };
    const never = () => {
        window.localStorage?.setItem(STATE_KEY, 'dismissed');
        setShow(false);
    };

    return (
        <div className="review-prompt" role="dialog" aria-label="Enjoying Duckle?">
            <button
                type="button"
                className="review-prompt-x"
                aria-label="Close"
                title="Close"
                onClick={later}
            >
                ×
            </button>
            <div className="review-prompt-title">Enjoying Duckle?</div>
            <div className="review-prompt-body">
                If Duckle's been useful, a quick review on SourceForge really helps others find it.
            </div>
            <div className="review-prompt-actions">
                <button type="button" className="review-prompt-cta" onClick={review}>
                    Review
                </button>
                <button type="button" className="review-prompt-link" onClick={later}>
                    Remind me later
                </button>
                <button type="button" className="review-prompt-link" onClick={never}>
                    No thanks
                </button>
            </div>
        </div>
    );
}
