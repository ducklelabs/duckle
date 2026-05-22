import { createContext } from 'react';
import type { Column } from '../../pipeline-types';
import type { ConnectionPayload, RepoItem, RoutinePayload } from '../../repo-types';

export type ActiveContext = {
    id: string;
    name: string;
    variables: { key: string; value: string; secret?: boolean }[];
};

export type FieldContextValue = {
    upstreamSchema: Column[];
    nodeSchema: Column[];
    repoItems: RepoItem[];
    /** The context whose variables fields can bind to (if any). */
    activeContext?: ActiveContext;
    onPickConnection?: (payload: ConnectionPayload) => void;
    onPickRoutine?: (payload: RoutinePayload) => void;
};

export const FieldContext = createContext<FieldContextValue>({
    upstreamSchema: [],
    nodeSchema: [],
    repoItems: [],
});
