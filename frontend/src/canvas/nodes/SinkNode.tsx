import { Handle, Position, type Node, type NodeProps } from '@xyflow/react';

export type SinkNodeData = {
    label: string;
    subtitle?: string;
};

export type SinkNodeType = Node<SinkNodeData, 'sink'>;

export default function SinkNode({ data }: NodeProps<SinkNodeType>) {
    return (
        <div className="node node-sink">
            <div className="node-kind">sink</div>
            <div className="node-label">{data.label}</div>
            {data.subtitle ? <div className="node-subtitle">{data.subtitle}</div> : null}
            <Handle type="target" position={Position.Left} />
        </div>
    );
}
