import { useCallback } from 'react';
import {
    ReactFlow,
    ReactFlowProvider,
    Background,
    Controls,
    MiniMap,
    addEdge,
    useEdgesState,
    useNodesState,
    type Connection,
    type Edge,
    type Node,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import SourceNode from './nodes/SourceNode';
import TransformNode from './nodes/TransformNode';
import SinkNode from './nodes/SinkNode';

const nodeTypes = {
    source: SourceNode,
    transform: TransformNode,
    sink: SinkNode,
};

const initialNodes: Node[] = [
    {
        id: 's1',
        type: 'source',
        position: { x: 60, y: 140 },
        data: { label: 'CSV', subtitle: 'orders.csv' },
    },
    {
        id: 't1',
        type: 'transform',
        position: { x: 340, y: 140 },
        data: { label: 'Filter', subtitle: 'status = "paid"' },
    },
    {
        id: 'k1',
        type: 'sink',
        position: { x: 620, y: 140 },
        data: { label: 'Parquet', subtitle: 'orders_paid.parquet' },
    },
];

const initialEdges: Edge[] = [
    { id: 'e1', source: 's1', target: 't1' },
    { id: 'e2', source: 't1', target: 'k1' },
];

function CanvasInner() {
    const [nodes, , onNodesChange] = useNodesState(initialNodes);
    const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges);

    const onConnect = useCallback(
        (connection: Connection) => setEdges(eds => addEdge(connection, eds)),
        [setEdges],
    );

    return (
        <ReactFlow
            nodes={nodes}
            edges={edges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            nodeTypes={nodeTypes}
            fitView
            colorMode="dark"
        >
            <Background gap={16} />
            <MiniMap pannable zoomable />
            <Controls />
        </ReactFlow>
    );
}

export default function Canvas() {
    return (
        <ReactFlowProvider>
            <CanvasInner />
        </ReactFlowProvider>
    );
}
