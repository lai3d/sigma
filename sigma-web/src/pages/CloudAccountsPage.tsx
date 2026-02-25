import { useState } from 'react';
import { Plus, RefreshCw, Pencil, Trash2 } from 'lucide-react';
import {
  useCloudAccounts,
  useDeleteCloudAccount,
  useSyncCloudAccount,
} from '@/hooks/useCloudAccounts';
import Pagination from '@/components/Pagination';
import ConfirmDialog from '@/components/ConfirmDialog';
import CloudAccountDialog from './CloudAccountDialog';
import type { CloudSyncResult, CloudProviderType } from '@/types/api';

const PROVIDER_LABELS: Record<CloudProviderType, string> = {
  aws: 'AWS EC2',
  alibaba: 'Alibaba Cloud',
  digitalocean: 'DigitalOcean',
  linode: 'Linode',
  volcengine: 'Volcengine',
};

const PROVIDER_COLORS: Record<CloudProviderType, string> = {
  aws: 'bg-yellow-100 text-yellow-800',
  alibaba: 'bg-orange-100 text-orange-800',
  digitalocean: 'bg-blue-100 text-blue-800',
  linode: 'bg-green-100 text-green-800',
  volcengine: 'bg-indigo-100 text-indigo-800',
};

export default function CloudAccountsPage() {
  const [page, setPage] = useState(1);
  const { data: result, isLoading } = useCloudAccounts({ page, per_page: 25 });
  const deleteMutation = useDeleteCloudAccount();
  const syncMutation = useSyncCloudAccount();

  const [showCreate, setShowCreate] = useState(false);
  const [editAccount, setEditAccount] = useState<{
    id: string;
    name: string;
    provider_type: CloudProviderType;
  } | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [syncingId, setSyncingId] = useState<string | null>(null);
  const [syncResult, setSyncResult] = useState<CloudSyncResult | null>(null);

  async function handleSync(id: string) {
    setSyncingId(id);
    setSyncResult(null);
    try {
      const result = await syncMutation.mutateAsync(id);
      setSyncResult(result);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Sync failed';
      alert(msg);
    } finally {
      setSyncingId(null);
    }
  }

  function maskedConfigSummary(masked: Record<string, string>): string {
    const values = Object.values(masked).filter(
      (v) => typeof v === 'string' && v !== '****'
    );
    return values[0] || '****';
  }

  return (
    <div>
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold text-gray-900">Cloud Accounts</h2>
        <button
          onClick={() => setShowCreate(true)}
          className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700"
        >
          <Plus size={16} /> Add Account
        </button>
      </div>

      {syncResult && (
        <div className="mt-3 p-3 bg-green-50 border border-green-200 rounded-md text-sm text-green-800">
          Sync complete: {syncResult.instances_found} instances found,{' '}
          {syncResult.created} created, {syncResult.updated} updated,{' '}
          {syncResult.retired} retired
          <button
            onClick={() => setSyncResult(null)}
            className="ml-3 text-green-600 hover:text-green-800 font-medium"
          >
            Dismiss
          </button>
        </div>
      )}

      <div className="mt-4 bg-white rounded-lg border overflow-x-auto">
        {isLoading ? (
          <div className="p-8 text-center text-gray-500">Loading...</div>
        ) : !result?.data.length ? (
          <div className="p-8 text-center text-gray-400">
            No cloud accounts configured
          </div>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-gray-500 border-b bg-gray-50">
                <th className="px-4 py-3 font-medium">Name</th>
                <th className="px-4 py-3 font-medium">Type</th>
                <th className="px-4 py-3 font-medium">Credentials</th>
                <th className="px-4 py-3 font-medium text-right">VPS Count</th>
                <th className="px-4 py-3 font-medium">Last Synced</th>
                <th className="px-4 py-3 font-medium w-36">Actions</th>
              </tr>
            </thead>
            <tbody>
              {result.data.map((acc) => (
                <tr
                  key={acc.id}
                  className="border-b last:border-0 hover:bg-gray-50"
                >
                  <td className="px-4 py-3 font-medium text-gray-900">
                    {acc.name}
                  </td>
                  <td className="px-4 py-3">
                    <span
                      className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${
                        PROVIDER_COLORS[acc.provider_type] ||
                        'bg-gray-100 text-gray-600'
                      }`}
                    >
                      {PROVIDER_LABELS[acc.provider_type] || acc.provider_type}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-gray-500 font-mono text-xs">
                    {maskedConfigSummary(acc.masked_config)}
                  </td>
                  <td className="px-4 py-3 text-right">{acc.vps_count}</td>
                  <td className="px-4 py-3 text-gray-500">
                    {acc.last_synced_at
                      ? new Date(acc.last_synced_at).toLocaleString()
                      : 'Never'}
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-1">
                      <button
                        onClick={() => handleSync(acc.id)}
                        disabled={syncingId === acc.id}
                        className="p-1.5 text-blue-600 hover:text-blue-800 disabled:opacity-50"
                        title="Sync"
                      >
                        <RefreshCw
                          size={15}
                          className={
                            syncingId === acc.id ? 'animate-spin' : ''
                          }
                        />
                      </button>
                      <button
                        onClick={() =>
                          setEditAccount({
                            id: acc.id,
                            name: acc.name,
                            provider_type: acc.provider_type,
                          })
                        }
                        className="p-1.5 text-gray-500 hover:text-gray-700"
                        title="Edit"
                      >
                        <Pencil size={15} />
                      </button>
                      <button
                        onClick={() => setConfirmDelete(acc.id)}
                        className="p-1.5 text-red-500 hover:text-red-700"
                        title="Delete"
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
        <CloudAccountDialog onClose={() => setShowCreate(false)} />
      )}
      {editAccount && (
        <CloudAccountDialog
          account={editAccount}
          onClose={() => setEditAccount(null)}
        />
      )}

      <ConfirmDialog
        open={!!confirmDelete}
        title="Delete Cloud Account"
        message="This will delete this account. Linked VPS records will be kept but unlinked."
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
