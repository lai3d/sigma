import { useState } from 'react';
import { Plus, Trash2, Key } from 'lucide-react';
import { useApiKeys, useDeleteApiKey } from '@/hooks/useApiKeys';
import ConfirmDialog from '@/components/ConfirmDialog';
import Pagination from '@/components/Pagination';
import { formatDate } from '@/lib/utils';
import ApiKeyFormDialog from './ApiKeyFormDialog';

const ROLE_BADGE: Record<string, string> = {
  admin: 'bg-red-50 text-red-700',
  operator: 'bg-blue-50 text-blue-700',
  readonly: 'bg-gray-100 text-gray-600',
  agent: 'bg-amber-50 text-amber-700',
};

export default function ApiKeyList() {
  const [page, setPage] = useState(1);
  const { data: result, isLoading } = useApiKeys({ page, per_page: 25 });
  const deleteMutation = useDeleteApiKey();

  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);

  const keys = result?.data;

  return (
    <div>
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold text-gray-900">API Keys</h2>
        <button
          onClick={() => setShowCreate(true)}
          className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700"
        >
          <Plus size={16} /> Create Key
        </button>
      </div>

      <div className="mt-4 bg-white rounded-lg border overflow-x-auto">
        {isLoading ? (
          <div className="p-8 text-center text-gray-500">Loading...</div>
        ) : !keys?.length ? (
          <div className="p-8 text-center text-gray-400">
            <Key size={32} className="mx-auto mb-2 opacity-40" />
            No API keys yet
          </div>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-gray-500 border-b bg-gray-50">
                <th className="px-4 py-3 font-medium">Name</th>
                <th className="px-4 py-3 font-medium">Prefix</th>
                <th className="px-4 py-3 font-medium">Role</th>
                <th className="px-4 py-3 font-medium">Last Used</th>
                <th className="px-4 py-3 font-medium">Created</th>
                <th className="px-4 py-3 font-medium w-20">Actions</th>
              </tr>
            </thead>
            <tbody>
              {keys.map((k) => (
                <tr key={k.id} className="border-b last:border-0 hover:bg-gray-50">
                  <td className="px-4 py-3 font-medium text-gray-900">{k.name}</td>
                  <td className="px-4 py-3 font-mono text-xs text-gray-500">{k.key_prefix}...</td>
                  <td className="px-4 py-3">
                    <span className={`inline-block px-2 py-0.5 rounded text-xs font-medium ${ROLE_BADGE[k.role] || ''}`}>
                      {k.role}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-gray-500">
                    {k.last_used_at ? formatDate(k.last_used_at) : <span className="text-gray-400">Never</span>}
                  </td>
                  <td className="px-4 py-3 text-gray-500">{formatDate(k.created_at)}</td>
                  <td className="px-4 py-3">
                    <button
                      title="Delete"
                      onClick={() => setConfirmDelete(k.id)}
                      className="p-1 text-red-500 hover:bg-red-50 rounded"
                    >
                      <Trash2 size={15} />
                    </button>
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
        <ApiKeyFormDialog onClose={() => setShowCreate(false)} />
      )}

      <ConfirmDialog
        open={!!confirmDelete}
        title="Delete API Key"
        message="This will permanently delete this API key. Applications using it will stop working immediately."
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
