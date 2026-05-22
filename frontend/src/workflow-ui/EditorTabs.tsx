import { useState } from 'react';
import type {
    Connection,
    Edge,
    EdgeChange,
    Node,
    NodeChange,
    OnSelectionChangeParams,
} from '@xyflow/react';
import Canvas, { type DropPosition, type NodeAction, type PaneAction } from '../canvas/Canvas';
import PlanView from './PlanView';
import RunView from './RunView';
import type { EngineId } from './EngineSelector';
import type { DuckleNodeData } from '../pipeline-types';
import type { ComponentDef } from './palette-data';
import type { ConnectionType } from '../canvas/connection-types';

type TabId = 'canvas' | 'plan' | 'run';

const TABS: { id: TabId; label: string }[] = [
    { id: 'canvas', label: 'Canvas' },
    { id: 'plan', label: 'Plan' },
    { id: 'run', label: 'Run' },
];

type Props = {
    engine: EngineId;
    nodes: Node<DuckleNodeData>[];
    edges: Edge[];
    onNodesChange: (changes: NodeChange[]) => void;
    onEdgesChange: (changes: EdgeChange[]) => void;
    onConnectWithType: (connection: Connection, type: ConnectionType) => void;
    onSelectionChange: (params: OnSelectionChangeParams) => void;
    onDropComponent: (component: ComponentDef, position: DropPosition) => void;
    onSetActiveContext?: (id: string) => void;
    onNodeAction: (action: NodeAction, nodeId: string) => void;
    onPaneAction: (action: PaneAction) => void;
    onEdgeChangeType: (edgeId: string, newType: ConnectionType) => void;
    onEdgeDelete: (edgeId: string) => void;
    onEdgeEdit: (edgeId: string) => void;
    nodeAutodetectAvailable: (nodeId: string) => boolean;
};

export default function EditorTabs({
    engine,
    nodes,
    edges,
    onNodesChange,
    onEdgesChange,
    onConnectWithType,
    onSelectionChange,
    onDropComponent,
    onSetActiveContext,
    onNodeAction,
    onPaneAction,
    onEdgeChangeType,
    onEdgeDelete,
    onEdgeEdit,
    nodeAutodetectAvailable,
}: Props) {
    const [active, setActive] = useState<TabId>('canvas');

    return (
        <div className="editor">
            <div className="tabbar" role="tablist" aria-label="Editor views">
                {TABS.map(t => (
                    <button
                        key={t.id}
                        type="button"
                        role="tab"
                        aria-selected={active === t.id}
                        className="tab"
                        onClick={() => setActive(t.id)}
                    >
                        {t.label}
                    </button>
                ))}
            </div>
            <div className="tab-content">
                <div className={'tab-panel' + (active === 'canvas' ? ' tab-panel-active' : '')}>
                    <Canvas
                        nodes={nodes}
                        edges={edges}
                        onNodesChange={onNodesChange}
                        onEdgesChange={onEdgesChange}
                        onConnectWithType={onConnectWithType}
                        onSelectionChange={onSelectionChange}
                        onDropComponent={onDropComponent}
                        onSetActiveContext={onSetActiveContext}
                        onNodeAction={onNodeAction}
                        onPaneAction={onPaneAction}
                        onEdgeChangeType={onEdgeChangeType}
                        onEdgeDelete={onEdgeDelete}
                        onEdgeEdit={onEdgeEdit}
                        nodeAutodetectAvailable={nodeAutodetectAvailable}
                    />
                </div>
                <div className={'tab-panel' + (active === 'plan' ? ' tab-panel-active' : '')}>
                    <PlanView engine={engine} />
                </div>
                <div className={'tab-panel' + (active === 'run' ? ' tab-panel-active' : '')}>
                    <RunView engine={engine} />
                </div>
            </div>
        </div>
    );
}
