import { useState } from 'react';
import { useParams, useNavigate, Link } from 'react-router-dom';
import { Pencil, Trash2, Power, BarChart3 } from 'lucide-react';
import { useVps, useDeleteVps, useRetireVps } from '@/hooks/useVps';
import { useProvider } from '@/hooks/useProviders';
import { useTickets } from '@/hooks/useTickets';
import { useAuth } from '@/contexts/AuthContext';
import StatusBadge from '@/components/StatusBadge';
import ConfirmDialog from '@/components/ConfirmDialog';
import { formatDate, daysUntil, timeAgo, formatUptime, ipLabelColor, ipLabelShort } from '@/lib/utils';

const PRIORITY_COLORS: Record<string, string> = {
  low: 'bg-gray-100 text-gray-600',
  medium: 'bg-blue-100 text-blue-700',
  high: 'bg-orange-100 text-orange-700',
  critical: 'bg-red-100 text-red-700',
};

const TICKET_STATUS_COLORS: Record<string, string> = {
  open: 'bg-blue-100 text-blue-800',
  'in-progress': 'bg-yellow-100 text-yellow-800',
  resolved: 'bg-green-100 text-green-800',
  closed: 'bg-gray-100 text-gray-600',
};

export default function VpsDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { user } = useAuth();
  const canMutate = user?.role === 'admin' || user?.role === 'operator';
  const isAdmin = user?.role === 'admin';

  const { data: vps, isLoading } = useVps(id || '');
  const { data: provider } = useProvider(vps?.provider_id || '');
  const { data: ticketsResult } = useTickets({ vps_id: id, per_page: 10 });
  const deleteMutation = useDeleteVps();
  const retireMutation = useRetireVps();

  const [confirmDelete, setConfirmDelete] = useState(false);
  const [confirmRetire, setConfirmRetire] = useState(false);

  if (isLoading) return <div className="p-8 text-center text-gray-500">Loading...</div>;
  if (!vps) return <div className="p-8 text-center text-gray-400">VPS not found</div>;

  const handleRetire = () => {
    retireMutation.mutate(vps.id, { onSuccess: () => setConfirmRetire(false) });
  };

  const handleDelete = () => {
    deleteMutation.mutate(vps.id, { onSuccess: () => navigate('/vps') });
  };

  const grafanaBaseUrl = localStorage.getItem('sigma_grafana_url') || '';
  const firstIp = vps.ip_addresses?.[0]?.ip;
  const grafanaLink = grafanaBaseUrl && firstIp
    ? `${grafanaBaseUrl}${grafanaBaseUrl.includes('?') ? '&' : '?'}var-target=${encodeURIComponent(firstIp)}`
    : '';

  const days = daysUntil(vps.expire_date);
  const tickets = ticketsResult?.data;

  // Agent info
  const hb = vps.extra?.last_heartbeat as string | undefined;
  const agentOnline = hb ? Date.now() - new Date(hb).getTime() < 3 * 60 * 1000 : false;
  const si = vps.extra?.system_info as { cpu_cores?: number; ram_mb?: number; disk_gb?: number; uptime_seconds?: number; load_avg?: number[] } | undefined;

  return (
    <div>
      {/* Header */}
      <div className="flex items-start justify-between">
        <div>
          <div className="flex items-center gap-3">
            <h2 className="text-2xl font-bold text-gray-900 font-mono">{vps.hostname}</h2>
            <StatusBadge status={vps.status} />
          </div>
          {vps.alias && (
            <p className="mt-1 text-sm text-gray-500">{vps.alias}</p>
          )}
        </div>
        <div className="flex items-center gap-2">
          {grafanaLink && (
            <a
              href={grafanaLink}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium text-orange-700 bg-orange-50 border border-orange-200 rounded-md hover:bg-orange-100"
            >
              <BarChart3 size={14} /> Grafana
            </a>
          )}
          {canMutate && (
            <>
              <Link
                to={`/vps/${vps.id}/edit`}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium border rounded-md hover:bg-gray-50"
              >
                <Pencil size={14} /> Edit
              </Link>
              {vps.status !== 'retired' && (
                <button
                  onClick={() => setConfirmRetire(true)}
                  className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium text-orange-600 border border-orange-200 rounded-md hover:bg-orange-50"
                >
                  <Power size={14} /> Retire
                </button>
              )}
              {isAdmin && (
                <button
                  onClick={() => setConfirmDelete(true)}
                  className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium text-red-600 border border-red-200 rounded-md hover:bg-red-50"
                >
                  <Trash2 size={14} /> Delete
                </button>
              )}
            </>
          )}
        </div>
      </div>

      {/* Agent Info */}
      {hb && (
        <div className="mt-6 bg-gray-50 rounded-lg border p-5">
          <h3 className="text-sm font-semibold text-gray-700 mb-3">Agent Info</h3>
          <div className="grid grid-cols-2 md:grid-cols-3 gap-x-6 gap-y-2 text-sm">
            <div>
              <span className="text-gray-500">Last Heartbeat</span>
              <div className="flex items-center gap-1.5 mt-0.5 font-medium">
                <span className={`inline-block w-2 h-2 rounded-full ${agentOnline ? 'bg-green-500' : 'bg-red-500'}`} />
                {timeAgo(hb)}
              </div>
            </div>
            {si?.cpu_cores != null && (
              <div>
                <span className="text-gray-500">CPU Cores</span>
                <div className="mt-0.5 font-medium">{si.cpu_cores}</div>
              </div>
            )}
            {si?.ram_mb != null && (
              <div>
                <span className="text-gray-500">RAM</span>
                <div className="mt-0.5 font-medium">{si.ram_mb >= 1024 ? `${(si.ram_mb / 1024).toFixed(1)} GB` : `${si.ram_mb} MB`}</div>
              </div>
            )}
            {si?.disk_gb != null && (
              <div>
                <span className="text-gray-500">Disk</span>
                <div className="mt-0.5 font-medium">{si.disk_gb} GB</div>
              </div>
            )}
            {si?.uptime_seconds != null && (
              <div>
                <span className="text-gray-500">Uptime</span>
                <div className="mt-0.5 font-medium">{formatUptime(si.uptime_seconds)}</div>
              </div>
            )}
            {si?.load_avg && (
              <div>
                <span className="text-gray-500">Load Average</span>
                <div className="mt-0.5 font-medium">{si.load_avg.map(v => v.toFixed(2)).join(' / ')}</div>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Info Panels */}
      <div className="mt-6 grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* Basic Info */}
        <div className="bg-white rounded-lg border p-5">
          <h3 className="text-sm font-medium text-gray-500 mb-3">Basic Info</h3>
          <dl className="space-y-2 text-sm">
            <div className="flex justify-between">
              <dt className="text-gray-500">Provider</dt>
              <dd className="text-gray-700">{provider?.name || '-'}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-gray-500">Country</dt>
              <dd className="text-gray-700">{vps.country || '-'}</dd>
            </div>
            {vps.city && (
              <div className="flex justify-between">
                <dt className="text-gray-500">City</dt>
                <dd className="text-gray-700">{vps.city}</dd>
              </div>
            )}
            {vps.dc_name && (
              <div className="flex justify-between">
                <dt className="text-gray-500">Data Center</dt>
                <dd className="text-gray-700">{vps.dc_name}</dd>
              </div>
            )}
            <div className="flex justify-between">
              <dt className="text-gray-500">Purpose</dt>
              <dd className="text-gray-700">{vps.purpose || '-'}</dd>
            </div>
            {vps.vpn_protocol && (
              <div className="flex justify-between">
                <dt className="text-gray-500">VPN Protocol</dt>
                <dd className="text-gray-700">{vps.vpn_protocol}</dd>
              </div>
            )}
            {vps.tags.length > 0 && (
              <div className="flex justify-between items-start">
                <dt className="text-gray-500">Tags</dt>
                <dd className="flex flex-wrap gap-1 justify-end">
                  {vps.tags.map((t) => (
                    <span key={t} className="px-1.5 py-0.5 bg-blue-50 text-blue-700 text-xs rounded">
                      {t}
                    </span>
                  ))}
                </dd>
              </div>
            )}
          </dl>
        </div>

        {/* Network */}
        <div className="bg-white rounded-lg border p-5">
          <h3 className="text-sm font-medium text-gray-500 mb-3">Network</h3>
          <dl className="space-y-2 text-sm">
            <div>
              <dt className="text-gray-500 mb-1">IP Addresses</dt>
              <dd>
                {vps.ip_addresses.length > 0 ? (
                  <div className="space-y-1">
                    {vps.ip_addresses.map((entry, i) => (
                      <div key={i} className="flex items-center gap-1.5">
                        <span className="font-mono text-xs">{entry.ip}</span>
                        {entry.label && (
                          <span className={`px-1 py-0.5 text-[10px] rounded ${ipLabelColor(entry.label)}`}>
                            {ipLabelShort(entry.label)}
                          </span>
                        )}
                      </div>
                    ))}
                  </div>
                ) : (
                  <span className="text-gray-400">-</span>
                )}
              </dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-gray-500">SSH Port</dt>
              <dd className="text-gray-700 font-mono">{vps.ssh_port}</dd>
            </div>
          </dl>
        </div>

        {/* Specs */}
        <div className="bg-white rounded-lg border p-5">
          <h3 className="text-sm font-medium text-gray-500 mb-3">Specs</h3>
          <dl className="space-y-2 text-sm">
            {vps.cpu_cores != null && (
              <div className="flex justify-between">
                <dt className="text-gray-500">CPU Cores</dt>
                <dd className="text-gray-700">{vps.cpu_cores}</dd>
              </div>
            )}
            {vps.ram_mb != null && (
              <div className="flex justify-between">
                <dt className="text-gray-500">RAM</dt>
                <dd className="text-gray-700">{vps.ram_mb >= 1024 ? `${(vps.ram_mb / 1024).toFixed(1)} GB` : `${vps.ram_mb} MB`}</dd>
              </div>
            )}
            {vps.disk_gb != null && (
              <div className="flex justify-between">
                <dt className="text-gray-500">Disk</dt>
                <dd className="text-gray-700">{vps.disk_gb} GB</dd>
              </div>
            )}
            {vps.bandwidth_tb && (
              <div className="flex justify-between">
                <dt className="text-gray-500">Bandwidth</dt>
                <dd className="text-gray-700">{vps.bandwidth_tb} TB</dd>
              </div>
            )}
          </dl>
        </div>

        {/* Cost & Dates */}
        <div className="bg-white rounded-lg border p-5">
          <h3 className="text-sm font-medium text-gray-500 mb-3">Cost & Dates</h3>
          <dl className="space-y-2 text-sm">
            {vps.cost_monthly && (
              <div className="flex justify-between">
                <dt className="text-gray-500">Monthly Cost</dt>
                <dd className="text-gray-700">{vps.cost_monthly} {vps.currency}</dd>
              </div>
            )}
            <div className="flex justify-between">
              <dt className="text-gray-500">Purchase Date</dt>
              <dd className="text-gray-700">{formatDate(vps.purchase_date)}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-gray-500">Expire Date</dt>
              <dd className={days !== null && days <= 7 ? 'text-red-600 font-medium' : 'text-gray-700'}>
                {formatDate(vps.expire_date)}
                {days !== null && (
                  <span className="ml-1 text-xs">({days}d)</span>
                )}
              </dd>
            </div>
          </dl>
        </div>

        {/* Monitoring */}
        <div className="bg-white rounded-lg border p-5">
          <h3 className="text-sm font-medium text-gray-500 mb-3">Monitoring</h3>
          <dl className="space-y-2 text-sm">
            <div className="flex justify-between">
              <dt className="text-gray-500">Monitoring Enabled</dt>
              <dd className="text-gray-700">{vps.monitoring_enabled ? 'Yes' : 'No'}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-gray-500">Node Exporter Port</dt>
              <dd className="text-gray-700 font-mono">{vps.node_exporter_port}</dd>
            </div>
          </dl>
        </div>

        {/* Notes */}
        {vps.notes && (
          <div className="bg-white rounded-lg border p-5">
            <h3 className="text-sm font-medium text-gray-500 mb-3">Notes</h3>
            <p className="text-sm text-gray-700 whitespace-pre-wrap">{vps.notes}</p>
          </div>
        )}
      </div>

      {/* Related Tickets */}
      <div className="mt-6">
        <h3 className="text-lg font-semibold text-gray-900 mb-4">Related Tickets</h3>
        {tickets && tickets.length > 0 ? (
          <div className="bg-white rounded-lg border overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-gray-500 border-b bg-gray-50">
                  <th className="px-4 py-3 font-medium">Title</th>
                  <th className="px-4 py-3 font-medium">Status</th>
                  <th className="px-4 py-3 font-medium">Priority</th>
                  <th className="px-4 py-3 font-medium">Created</th>
                </tr>
              </thead>
              <tbody>
                {tickets.map((t) => (
                  <tr key={t.id} className="border-b last:border-0 hover:bg-gray-50">
                    <td className="px-4 py-3">
                      <Link to={`/tickets/${t.id}`} className="text-blue-600 hover:underline">
                        {t.title}
                      </Link>
                    </td>
                    <td className="px-4 py-3">
                      <span className={`inline-block px-2 py-0.5 text-xs font-medium rounded ${TICKET_STATUS_COLORS[t.status] || ''}`}>
                        {t.status}
                      </span>
                    </td>
                    <td className="px-4 py-3">
                      <span className={`inline-block px-2 py-0.5 text-xs font-medium rounded ${PRIORITY_COLORS[t.priority] || ''}`}>
                        {t.priority}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-gray-500">{formatDate(t.created_at)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <p className="text-sm text-gray-400">No tickets linked to this VPS.</p>
        )}
      </div>

      {/* Metadata footer */}
      <div className="mt-6 pt-4 border-t text-xs text-gray-400 flex gap-4">
        <span>Created: {formatDate(vps.created_at)}</span>
        <span>Updated: {formatDate(vps.updated_at)}</span>
      </div>

      <ConfirmDialog
        open={confirmRetire}
        title="Retire VPS"
        message="This will mark the VPS as retired and disable monitoring. Continue?"
        confirmLabel="Retire"
        variant="danger"
        onConfirm={handleRetire}
        onCancel={() => setConfirmRetire(false)}
      />

      <ConfirmDialog
        open={confirmDelete}
        title="Delete VPS"
        message="This will permanently delete this VPS record. This cannot be undone."
        confirmLabel="Delete"
        variant="danger"
        onConfirm={handleDelete}
        onCancel={() => setConfirmDelete(false)}
      />
    </div>
  );
}
