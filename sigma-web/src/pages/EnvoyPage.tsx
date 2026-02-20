import { useState, useMemo } from 'react';
import { Plus, Pencil, Trash2 } from 'lucide-react';
import { useEnvoyNodes, useDeleteEnvoyNode, useEnvoyRoutes, useDeleteEnvoyRoute } from '@/hooks/useEnvoy';
import { useVpsList } from '@/hooks/useVps';
import ConfirmDialog from '@/components/ConfirmDialog';
import Pagination from '@/components/Pagination';
import EnvoyNodeFormDialog from './EnvoyNodeFormDialog';
import EnvoyRouteFormDialog from './EnvoyRouteFormDialog';

type Tab = 'nodes' | 'routes';

const STATUS_COLORS: Record<string, string> = {
  active: 'bg-green-100 text-green-800',
  disabled: 'bg-gray-100 text-gray-600',
  placeholder: 'bg-yellow-100 text-yellow-800',
};

export default function EnvoyPage() {
  const [tab, setTab] = useState<Tab>('nodes');

  return (
    <div>
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold text-gray-900">Envoy</h2>
      </div>

      {/* Tab toggle */}
      <div className="mt-4 flex gap-1 bg-gray-100 rounded-lg p-1 w-fit">
        {(['nodes', 'routes'] as Tab[]).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`px-4 py-1.5 text-sm font-medium rounded-md transition-colors ${
              tab === t
                ? 'bg-white text-gray-900 shadow-sm'
                : 'text-gray-500 hover:text-gray-700'
            }`}
          >
            {t === 'nodes' ? 'Nodes' : 'Routes'}
          </button>
        ))}
      </div>

      {tab === 'nodes' ? <NodesTab /> : <RoutesTab />}
    </div>
  );
}

// ─── Nodes Tab ───────────────────────────────────────────

function NodesTab() {
  const [page, setPage] = useState(1);
  const [statusFilter, setStatusFilter] = useState('');
  const { data: result, isLoading } = useEnvoyNodes({
    page,
    per_page: 25,
    status: statusFilter || undefined,
  });
  const deleteMutation = useDeleteEnvoyNode();
  const { data: vpsResult } = useVpsList({ per_page: 100 });

  const vpsMap = useMemo(() => {
    const map = new Map<string, string>();
    vpsResult?.data?.forEach((v) => map.set(v.id, v.hostname));
    return map;
  }, [vpsResult]);

  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [editId, setEditId] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);

  const nodes = result?.data;

  return (
    <>
      <div className="mt-4 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <select
            value={statusFilter}
            onChange={(e) => { setStatusFilter(e.target.value); setPage(1); }}
            className="input text-sm"
          >
            <option value="">All Statuses</option>
            <option value="active">Active</option>
            <option value="disabled">Disabled</option>
          </select>
        </div>
        <button
          onClick={() => setShowCreate(true)}
          className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700"
        >
          <Plus size={16} /> Add Node
        </button>
      </div>

      <div className="mt-4 bg-white rounded-lg border overflow-x-auto">
        {isLoading ? (
          <div className="p-8 text-center text-gray-500">Loading...</div>
        ) : !nodes?.length ? (
          <div className="p-8 text-center text-gray-400">No envoy nodes yet</div>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-gray-500 border-b bg-gray-50">
                <th className="px-4 py-3 font-medium">Node ID</th>
                <th className="px-4 py-3 font-medium">VPS</th>
                <th className="px-4 py-3 font-medium">Admin Port</th>
                <th className="px-4 py-3 font-medium">Status</th>
                <th className="px-4 py-3 font-medium">Config Version</th>
                <th className="px-4 py-3 font-medium">Description</th>
                <th className="px-4 py-3 font-medium w-24">Actions</th>
              </tr>
            </thead>
            <tbody>
              {nodes.map((n) => (
                <tr key={n.id} className="border-b last:border-0 hover:bg-gray-50">
                  <td className="px-4 py-3 font-medium text-gray-900 font-mono">{n.node_id}</td>
                  <td className="px-4 py-3">{vpsMap.get(n.vps_id) || n.vps_id.slice(0, 8)}</td>
                  <td className="px-4 py-3 font-mono">{n.admin_port ?? '-'}</td>
                  <td className="px-4 py-3">
                    <span className={`inline-block px-2 py-0.5 rounded-full text-xs font-medium ${STATUS_COLORS[n.status] || 'bg-gray-100 text-gray-600'}`}>
                      {n.status}
                    </span>
                  </td>
                  <td className="px-4 py-3 font-mono text-gray-500">{n.config_version}</td>
                  <td className="px-4 py-3 text-gray-500 truncate max-w-48">{n.description || '-'}</td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-1">
                      <button
                        title="Edit"
                        onClick={() => setEditId(n.id)}
                        className="p-1 text-gray-500 hover:bg-gray-100 rounded"
                      >
                        <Pencil size={15} />
                      </button>
                      <button
                        title="Delete"
                        onClick={() => setConfirmDelete(n.id)}
                        className="p-1 text-red-500 hover:bg-red-50 rounded"
                      >
                        <Trash2 size={15} />
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {result && (
        <Pagination
          page={result.page}
          perPage={result.per_page}
          total={result.total}
          onPageChange={setPage}
        />
      )}

      {showCreate && (
        <EnvoyNodeFormDialog onClose={() => setShowCreate(false)} />
      )}

      {editId && (
        <EnvoyNodeFormDialog id={editId} onClose={() => setEditId(null)} />
      )}

      <ConfirmDialog
        open={!!confirmDelete}
        title="Delete Envoy Node"
        message="This will permanently delete this node and all its routes."
        confirmLabel="Delete"
        variant="danger"
        onConfirm={() => {
          if (confirmDelete) deleteMutation.mutate(confirmDelete);
          setConfirmDelete(null);
        }}
        onCancel={() => setConfirmDelete(null)}
      />
    </>
  );
}

// ─── Routes Tab ──────────────────────────────────────────

function RoutesTab() {
  const [page, setPage] = useState(1);
  const [statusFilter, setStatusFilter] = useState('');
  const [nodeFilter, setNodeFilter] = useState('');

  const { data: nodesResult } = useEnvoyNodes({ per_page: 100 });
  const allNodes = nodesResult?.data ?? [];

  const nodeMap = useMemo(() => {
    const map = new Map<string, string>();
    allNodes.forEach((n) => map.set(n.id, n.node_id));
    return map;
  }, [allNodes]);

  const { data: result, isLoading } = useEnvoyRoutes({
    page,
    per_page: 25,
    envoy_node_id: nodeFilter || undefined,
    status: statusFilter || undefined,
  });
  const deleteMutation = useDeleteEnvoyRoute();

  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [editId, setEditId] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);

  const routes = result?.data;

  const ppLabel = (v: number) => v === 0 ? 'none' : v === 1 ? 'v1' : 'v2';

  return (
    <>
      <div className="mt-4 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <select
            value={nodeFilter}
            onChange={(e) => { setNodeFilter(e.target.value); setPage(1); }}
            className="input text-sm"
          >
            <option value="">All Nodes</option>
            {allNodes.map((n) => (
              <option key={n.id} value={n.id}>{n.node_id}</option>
            ))}
          </select>
          <select
            value={statusFilter}
            onChange={(e) => { setStatusFilter(e.target.value); setPage(1); }}
            className="input text-sm"
          >
            <option value="">All Statuses</option>
            <option value="active">Active</option>
            <option value="placeholder">Placeholder</option>
            <option value="disabled">Disabled</option>
          </select>
        </div>
        <button
          onClick={() => setShowCreate(true)}
          className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700"
        >
          <Plus size={16} /> Add Route
        </button>
      </div>

      <div className="mt-4 bg-white rounded-lg border overflow-x-auto">
        {isLoading ? (
          <div className="p-8 text-center text-gray-500">Loading...</div>
        ) : !routes?.length ? (
          <div className="p-8 text-center text-gray-400">No envoy routes yet</div>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-gray-500 border-b bg-gray-50">
                <th className="px-4 py-3 font-medium">Name</th>
                <th className="px-4 py-3 font-medium">Node</th>
                <th className="px-4 py-3 font-medium">Listen Port</th>
                <th className="px-4 py-3 font-medium">Backend</th>
                <th className="px-4 py-3 font-medium">Cluster</th>
                <th className="px-4 py-3 font-medium">PP</th>
                <th className="px-4 py-3 font-medium">Status</th>
                <th className="px-4 py-3 font-medium w-24">Actions</th>
              </tr>
            </thead>
            <tbody>
              {routes.map((r) => (
                <tr key={r.id} className="border-b last:border-0 hover:bg-gray-50">
                  <td className="px-4 py-3 font-medium text-gray-900">{r.name}</td>
                  <td className="px-4 py-3 font-mono text-sm">{nodeMap.get(r.envoy_node_id) || r.envoy_node_id.slice(0, 8)}</td>
                  <td className="px-4 py-3 font-mono">{r.listen_port}</td>
                  <td className="px-4 py-3 font-mono text-sm">
                    {r.backend_host
                      ? `${r.backend_host}:${r.backend_port ?? '?'}`
                      : <span className="text-gray-400">-</span>}
                  </td>
                  <td className="px-4 py-3 text-gray-500">{r.cluster_type}</td>
                  <td className="px-4 py-3 text-gray-500">{ppLabel(r.proxy_protocol)}</td>
                  <td className="px-4 py-3">
                    <span className={`inline-block px-2 py-0.5 rounded-full text-xs font-medium ${STATUS_COLORS[r.status] || 'bg-gray-100 text-gray-600'}`}>
                      {r.status}
                    </span>
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-1">
                      <button
                        title="Edit"
                        onClick={() => setEditId(r.id)}
                        className="p-1 text-gray-500 hover:bg-gray-100 rounded"
                      >
                        <Pencil size={15} />
                      </button>
                      <button
                        title="Delete"
                        onClick={() => setConfirmDelete(r.id)}
                        className="p-1 text-red-500 hover:bg-red-50 rounded"
                      >
                        <Trash2 size={15} />
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {result && (
        <Pagination
          page={result.page}
          perPage={result.per_page}
          total={result.total}
          onPageChange={setPage}
        />
      )}

      {showCreate && (
        <EnvoyRouteFormDialog nodes={allNodes} onClose={() => setShowCreate(false)} />
      )}

      {editId && (
        <EnvoyRouteFormDialog id={editId} nodes={allNodes} onClose={() => setEditId(null)} />
      )}

      <ConfirmDialog
        open={!!confirmDelete}
        title="Delete Envoy Route"
        message="This will permanently delete this route. The parent node's config version will be bumped."
        confirmLabel="Delete"
        variant="danger"
        onConfirm={() => {
          if (confirmDelete) deleteMutation.mutate(confirmDelete);
          setConfirmDelete(null);
        }}
        onCancel={() => setConfirmDelete(null)}
      />
    </>
  );
}
