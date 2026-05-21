import { Handle, Position, type Node, type NodeProps } from '@xyflow/react';

export type TransformNodeData = {
    label: string;
    subtitle?: string;
};

export type TransformNodeType = Node<TransformNodeData, 'transform'>;

export default function TransformNode({ data }: NodeProps<TransformNodeType>) {
    return (
        <div className="node node-transform">
            <div className="node-kind">transform</div>
            <div className="node-label">{data.label}</div>
            {data.subtitle ? <div className="node-subtitle">{data.subtitle}</div> : null}
            <Handle type="target" position={Position.Left} />
            <Handle type="source" position={Position.Right} />
        </div>
    );
}
