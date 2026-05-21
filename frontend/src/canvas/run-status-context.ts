import { createContext, useContext } from 'react';
import type { NodeRunStatus } from '../tauri-bridge';

export type RunStatusMap = Record<string, NodeRunStatus>;

export const RunStatusContext = createContext<RunStatusMap>({});

export function useRunStatus(nodeId: string): NodeRunStatus | undefined {
    return useContext(RunStatusContext)[nodeId];
}
