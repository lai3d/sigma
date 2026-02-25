import { useMemo, useEffect } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  useNodesState,
  useEdgesState,
  type Node,
  type Edge,
  type NodeTypes,
  Position,
  MarkerType,
  Handle,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import dagre from 'dagre';
import { useEnvoyTopology } from '@/hooks/useEnvoy';
import { useVpsPurposes } from '@/hooks/useVpsPurposes';
import { buildPurposeColorMap, buildPurposeLabelMap, getPurposeColor } from '@/lib/purposeColors';
import type { TopologyNode, TopologyEdge, TopologyRouteInfo } from '@/types/api';

const NODE_WIDTH = 200;
const NODE_HEIGHT = 80;

// ─── Custom Node Components ──────────────────────────────

interface VpsNodeData {
  hostname: string;
  alias: string;
  country: string;
  purpose: string;
  purposeLabel?: string;
  [key: string]: unknown;
}

function VpsNode({ data }: { data: VpsNodeData }) {
  const colorMap = data._colorMap as Record<string, ReturnType<typeof getPurposeColor>> | undefined;
  const colors = colorMap?.[data.purpose] ?? getPurposeColor('gray');
  return (
    <>
      <Handle type="target" position={Position.Left} />
      <div className={`px-4 py-3 rounded-lg border-2 shadow-sm ${colors.bg} ${colors.border} min-w-[180px]`}>
        <div className="flex items-center justify-between">
          <span className="font-bold text-sm text-gray-900 truncate">
            {data.hostname}
          </span>
          <span className="text-xs font-medium text-gray-500 ml-2 uppercase">{data.country}</span>
        </div>
        {data.alias && (
          <div className="text-xs text-gray-500 truncate">{data.alias}</div>
        )}
        <span className={`inline-block mt-1 px-1.5 py-0.5 rounded text-xs font-medium ${colors.badge}`}>
          {data.purposeLabel || data.purpose || 'unknown'}
        </span>
      </div>
      <Handle type="source" position={Position.Right} />
    </>
  );
}

function ExternalNode({ data }: { data: { label: string; [key: string]: unknown } }) {
  return (
    <>
      <Handle type="target" position={Position.Left} />
      <div className="px-4 py-3 rounded-lg border-2 border-dashed border-red-300 bg-red-50 shadow-sm min-w-[180px]">
        <div className="font-mono text-sm text-red-700 truncate">{data.label}</div>
        <span className="inline-block mt-1 px-1.5 py-0.5 rounded text-xs font-medium bg-red-100 text-red-600">
          external
        </span>
      </div>
      <Handle type="source" position={Position.Right} />
    </>
  );
}

const nodeTypes: NodeTypes = {
  vpsNode: VpsNode,
  externalNode: ExternalNode,
};

// ─── Dagre Layout ────────────────────────────────────────

function getLayoutedElements(nodes: Node[], edges: Edge[]): { nodes: Node[]; edges: Edge[] } {
  const g = new dagre.graphlib.Graph();
  g.setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: 'LR', nodesep: 60, ranksep: 150 });

  nodes.forEach((node) => {
    g.setNode(node.id, { width: NODE_WIDTH, height: NODE_HEIGHT });
  });
  edges.forEach((edge) => {
    g.setEdge(edge.source, edge.target);
  });

  dagre.layout(g);

  const layoutedNodes = nodes.map((node) => {
    const pos = g.node(node.id);
    return {
      ...node,
      position: { x: pos.x - NODE_WIDTH / 2, y: pos.y - NODE_HEIGHT / 2 },
    };
  });

  return { nodes: layoutedNodes, edges };
}

// ─── Edge Label ──────────────────────────────────────────

function formatEdgeLabel(routes: TopologyRouteInfo[]): string {
  const hasStatic = routes.some((r) => r.source === 'static');
  const hasDynamic = routes.some((r) => r.source === 'dynamic');
  const sourceTag = hasStatic && hasDynamic ? ' [mixed]' : hasStatic ? ' [static]' : '';

  if (routes.length === 1) {
    const r = routes[0];
    return `:${r.listen_port} \u2192 :${r.backend_port ?? '?'}${sourceTag}`;
  }
  const ports = routes.map((r) => r.listen_port).sort((a, b) => a - b);
  return `${routes.length} routes (:${ports[0]}\u2013:${ports[ports.length - 1]})${sourceTag}`;
}

function getEdgeColor(routes: TopologyRouteInfo[]): string {
  const allStatic = routes.every((r) => r.source === 'static');
  if (allStatic) return '#9ca3af'; // gray for static
  return '#6b7280'; // default
}

// ─── Transform API → React Flow ──────────────────────────

function buildFlowElements(
  apiNodes: TopologyNode[],
  apiEdges: TopologyEdge[],
  colorMap: Record<string, { bg: string; border: string; badge: string; minimap: string }>,
  labelMap: Record<string, string>,
) {
  const flowNodes: Node[] = [];
  const flowEdges: Edge[] = [];

  for (const n of apiNodes) {
    flowNodes.push({
      id: n.id,
      type: 'vpsNode',
      position: { x: 0, y: 0 },
      data: {
        hostname: n.hostname,
        alias: n.alias,
        country: n.country,
        purpose: n.purpose,
        purposeLabel: labelMap[n.purpose],
        _colorMap: colorMap,
      },
    });
  }

  const externalNodes = new Set<string>();

  for (let i = 0; i < apiEdges.length; i++) {
    const e = apiEdges[i];
    let targetId: string;

    if (e.target_vps_id) {
      targetId = e.target_vps_id;
    } else {
      const ext = e.target_external || 'unknown';
      targetId = `ext-${ext}`;
      if (!externalNodes.has(ext)) {
        externalNodes.add(ext);
        flowNodes.push({
          id: targetId,
          type: 'externalNode',
          position: { x: 0, y: 0 },
          data: { label: ext },
        });
      }
    }

    const edgeColor = getEdgeColor(e.routes);
    const allStatic = e.routes.every((r) => r.source === 'static');
    flowEdges.push({
      id: `e-${i}`,
      source: e.source_vps_id,
      target: targetId,
      label: formatEdgeLabel(e.routes),
      markerEnd: { type: MarkerType.ArrowClosed },
      style: { stroke: edgeColor, strokeWidth: 1.5, strokeDasharray: allStatic ? '5 3' : undefined },
      labelStyle: { fontSize: 11, fill: allStatic ? '#9ca3af' : '#374151' },
    });
  }

  return getLayoutedElements(flowNodes, flowEdges);
}

// ─── Page ────────────────────────────────────────────────

export default function TopologyPage() {
  const { data, isLoading, error } = useEnvoyTopology();
  const { data: purposesResult } = useVpsPurposes({ per_page: 100 });
  const purposes = purposesResult?.data ?? [];

  const colorMap = useMemo(() => buildPurposeColorMap(purposes), [purposes]);
  const labelMap = useMemo(() => buildPurposeLabelMap(purposes), [purposes]);

  const { layoutNodes, layoutEdges } = useMemo(() => {
    if (!data) return { layoutNodes: [] as Node[], layoutEdges: [] as Edge[] };
    const { nodes, edges } = buildFlowElements(data.nodes, data.edges, colorMap, labelMap);
    return { layoutNodes: nodes, layoutEdges: edges };
  }, [data, colorMap, labelMap]);

  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);

  useEffect(() => {
    setNodes(layoutNodes);
    setEdges(layoutEdges);
  }, [layoutNodes, layoutEdges, setNodes, setEdges]);

  if (isLoading) {
    return (
      <div>
        <h2 className="text-2xl font-bold text-gray-900">Network Topology</h2>
        <p className="mt-4 text-gray-500">Loading topology...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div>
        <h2 className="text-2xl font-bold text-gray-900">Network Topology</h2>
        <p className="mt-4 text-red-500">Failed to load topology</p>
      </div>
    );
  }

  if (!data || data.nodes.length === 0) {
    return (
      <div>
        <h2 className="text-2xl font-bold text-gray-900">Network Topology</h2>
        <p className="mt-8 text-center text-gray-400">
          No active Envoy routes to visualize. Create routes on the Envoy page to see the network topology.
        </p>
      </div>
    );
  }

  return (
    <div>
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold text-gray-900">Network Topology</h2>
        <div className="flex items-center gap-3 text-xs text-gray-500 flex-wrap">
          {purposes.map((p) => {
            const c = getPurposeColor(p.color);
            return (
              <span key={p.id} className="flex items-center gap-1">
                <span className={`w-3 h-3 rounded ${c.bg} border ${c.border}`} /> {p.label}
              </span>
            );
          })}
          <span className="flex items-center gap-1">
            <span className="w-3 h-3 rounded border-2 border-dashed border-red-300 bg-red-100" /> External
          </span>
          <span className="flex items-center gap-1">
            <span className="w-6 border-t-2 border-dashed border-gray-400" /> Static
          </span>
        </div>
      </div>

      <div className="mt-4 bg-white rounded-lg border" style={{ height: 'calc(100vh - 160px)' }}>
        <ReactFlow
          nodes={nodes}
          edges={edges}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          nodeTypes={nodeTypes}
          fitView
          fitViewOptions={{ padding: 0.2 }}
          minZoom={0.3}
          maxZoom={2}
          proOptions={{ hideAttribution: true }}
        >
          <Background />
          <Controls />
          <MiniMap
            nodeColor={(node) => {
              if (node.type === 'externalNode') return '#fecaca';
              const purpose = (node.data as VpsNodeData)?.purpose;
              return colorMap[purpose]?.minimap ?? '#d1d5db';
            }}
          />
        </ReactFlow>
      </div>
    </div>
  );
}
