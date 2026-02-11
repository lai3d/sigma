import { useState } from 'react';
import { Link } from 'react-router-dom';
import { Plus, Trash2, Power } from 'lucide-react';
import { useVpsList, useDeleteVps, useRetireVps } from '@/hooks/useVps';
import { useProviders } from '@/hooks/useProviders';
import StatusBadge from '@/components/StatusBadge';
import ConfirmDialog from '@/components/ConfirmDialog';
import { formatDate, daysUntil, ipLabelColor, ipLabelShort } from '@/lib/utils';
import type { VpsListQuery } from '@/types/api';

export default function VpsList() {
  const [filters, setFilters] = useState<VpsListQuery>({});
  const { data: vpsList, isLoading } = useVpsList(filters);
  const { data: providers } = useProviders();
  const deleteMutation = useDeleteVps();
  const retireMutation = useRetireVps();

  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [confirmRetire, setConfirmRetire] = useState<string | null>(null);

  const providerMap = new Map(providers?.map((p) => [p.id, p.name]) || []);

  return (
    <div>
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold text-gray-900">VPS Instances</h2>
        <Link
          to="/vps/new"
          className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700"
        >
          <Plus size={16} /> Add VPS
        </Link>
      </div>

      {/* Filters */}
      <div className="mt-4 flex flex-wrap gap-3">
        <select
          value={filters.status || ''}
          onChange={(e) => setFilters({ ...filters, status: e.target.value || undefined })}
          className="border rounded-md px-3 py-1.5 text-sm bg-white"
        >
          <option value="">All Statuses</option>
          <option value="provisioning">Provisioning</option>
          <option value="active">Active</option>
          <option value="retiring">Retiring</option>
          <option value="retired">Retired</option>
        </select>

        <select
          value={filters.purpose || ''}
          onChange={(e) => setFilters({ ...filters, purpose: e.target.value || undefined })}
          className="border rounded-md px-3 py-1.5 text-sm bg-white"
        >
          <option value="">All Purposes</option>
          <option value="vpn-exit">VPN Exit</option>
          <option value="vpn-relay">VPN Relay</option>
          <option value="vpn-entry">VPN Entry</option>
          <option value="monitor">Monitor</option>
          <option value="management">Management</option>
        </select>

        <select
          value={filters.provider_id || ''}
          onChange={(e) =>
            setFilters({ ...filters, provider_id: e.target.value || undefined })
          }
          className="border rounded-md px-3 py-1.5 text-sm bg-white"
        >
          <option value="">All Providers</option>
          {providers?.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>

        <input
          type="text"
          placeholder="Country (e.g. US)"
          value={filters.country || ''}
          onChange={(e) =>
            setFilters({ ...filters, country: e.target.value || undefined })
          }
          className="border rounded-md px-3 py-1.5 text-sm w-36"
        />

        <input
          type="text"
          placeholder="Tag"
          value={filters.tag || ''}
          onChange={(e) =>
            setFilters({ ...filters, tag: e.target.value || undefined })
          }
          className="border rounded-md px-3 py-1.5 text-sm w-32"
        />
      </div>

      {/* Table */}
      <div className="mt-4 bg-white rounded-lg border overflow-x-auto">
        {isLoading ? (
          <div className="p-8 text-center text-gray-500">Loading...</div>
        ) : !vpsList?.length ? (
          <div className="p-8 text-center text-gray-400">No VPS instances found</div>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-gray-500 border-b bg-gray-50">
                <th className="px-4 py-3 font-medium">Hostname</th>
                <th className="px-4 py-3 font-medium">IP</th>
                <th className="px-4 py-3 font-medium">Provider</th>
                <th className="px-4 py-3 font-medium">Country</th>
                <th className="px-4 py-3 font-medium">Purpose</th>
                <th className="px-4 py-3 font-medium">Status</th>
                <th className="px-4 py-3 font-medium">Expires</th>
                <th className="px-4 py-3 font-medium">Tags</th>
                <th className="px-4 py-3 font-medium w-24">Actions</th>
              </tr>
            </thead>
            <tbody>
              {vpsList.map((vps) => {
                const days = daysUntil(vps.expire_date);
                return (
                  <tr key={vps.id} className="border-b last:border-0 hover:bg-gray-50">
                    <td className="px-4 py-3">
                      <Link
                        to={`/vps/${vps.id}`}
                        className="font-mono text-blue-600 hover:underline"
                      >
                        {vps.hostname}
                      </Link>
                      {vps.alias && (
                        <span className="ml-2 text-xs text-gray-400">{vps.alias}</span>
                      )}
                    </td>
                    <td className="px-4 py-3">
                      <div className="space-y-0.5">
                        {vps.ip_addresses.map((entry, i) => (
                          <div key={i} className="flex items-center gap-1">
                            <span className="font-mono text-xs">{entry.ip}</span>
                            {entry.label && (
                              <span className={`px-1 py-0.5 text-[10px] rounded ${ipLabelColor(entry.label)}`}>
                                {ipLabelShort(entry.label)}
                              </span>
                            )}
                          </div>
                        ))}
                      </div>
                    </td>
                    <td className="px-4 py-3">{providerMap.get(vps.provider_id) || '-'}</td>
                    <td className="px-4 py-3">{vps.country}</td>
                    <td className="px-4 py-3">{vps.purpose || '-'}</td>
                    <td className="px-4 py-3">
                      <StatusBadge status={vps.status} />
                    </td>
                    <td className="px-4 py-3">
                      <span className={days !== null && days <= 7 ? 'text-red-600 font-medium' : ''}>
                        {formatDate(vps.expire_date)}
                      </span>
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex flex-wrap gap-1">
                        {vps.tags.map((t) => (
                          <span
                            key={t}
                            className="px-1.5 py-0.5 bg-blue-50 text-blue-700 text-xs rounded"
                          >
                            {t}
                          </span>
                        ))}
                      </div>
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex items-center gap-1">
                        {vps.status !== 'retired' && (
                          <button
                            title="Retire"
                            onClick={() => setConfirmRetire(vps.id)}
                            className="p-1 text-orange-500 hover:bg-orange-50 rounded"
                          >
                            <Power size={15} />
                          </button>
                        )}
                        <button
                          title="Delete"
                          onClick={() => setConfirmDelete(vps.id)}
                          className="p-1 text-red-500 hover:bg-red-50 rounded"
                        >
                          <Trash2 size={15} />
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      <ConfirmDialog
        open={!!confirmRetire}
        title="Retire VPS"
        message="This will mark the VPS as retired and disable monitoring. Continue?"
        confirmLabel="Retire"
        variant="danger"
        onConfirm={() => {
          if (confirmRetire) retireMutation.mutate(confirmRetire);
          setConfirmRetire(null);
        }}
        onCancel={() => setConfirmRetire(null)}
      />

      <ConfirmDialog
        open={!!confirmDelete}
        title="Delete VPS"
        message="This will permanently delete this VPS record. This cannot be undone."
        confirmLabel="Delete"
        variant="danger"
        onConfirm={() => {
          if (confirmDelete) deleteMutation.mutate(confirmDelete);
          setConfirmDelete(null);
        }}
        onCancel={() => setConfirmDelete(null)}
      />
    </div>
  );
}
