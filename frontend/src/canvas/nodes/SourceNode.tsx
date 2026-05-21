import { Handle, Position, type Node, type NodeProps } from '@xyflow/react';

export type SourceNodeData = {
    label: string;
    subtitle?: string;
};

export type SourceNodeType = Node<SourceNodeData, 'source'>;

export default function SourceNode({ data }: NodeProps<SourceNodeType>) {
    return (
        <div className="node node-source">
            <div className="node-kind">source</div>
            <div className="node-label">{data.label}</div>
            {data.subtitle ? <div className="node-subtitle">{data.subtitle}</div> : null}
            <Handle type="source" position={Position.Right} />
        </div>
    );
}
