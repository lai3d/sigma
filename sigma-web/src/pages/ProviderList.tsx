import { useState } from 'react';
import { Plus, Pencil, Trash2, ExternalLink } from 'lucide-react';
import { useProviders, useDeleteProvider, useImportProviders } from '@/hooks/useProviders';
import ConfirmDialog from '@/components/ConfirmDialog';
import ImportExportButtons from '@/components/ImportExportButtons';
import { exportProviders } from '@/api/providers';
import { formatDate } from '@/lib/utils';
import ProviderFormDialog from './ProviderFormDialog';

export default function ProviderList() {
  const { data: providers, isLoading } = useProviders();
  const deleteMutation = useDeleteProvider();
  const importMutation = useImportProviders();

  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [editId, setEditId] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);

  return (
    <div>
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold text-gray-900">Providers</h2>
        <div className="flex items-center gap-2">
          <ImportExportButtons
            entityName="providers"
            onExport={(format) => exportProviders(format)}
            onImport={(format, data) => importMutation.mutateAsync({ format, data })}
          />
          <button
            onClick={() => setShowCreate(true)}
            className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700"
          >
            <Plus size={16} /> Add Provider
          </button>
        </div>
      </div>

      <div className="mt-4 bg-white rounded-lg border overflow-x-auto">
        {isLoading ? (
          <div className="p-8 text-center text-gray-500">Loading...</div>
        ) : !providers?.length ? (
          <div className="p-8 text-center text-gray-400">No providers yet</div>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-gray-500 border-b bg-gray-50">
                <th className="px-4 py-3 font-medium">Name</th>
                <th className="px-4 py-3 font-medium">Country</th>
                <th className="px-4 py-3 font-medium">Website</th>
                <th className="px-4 py-3 font-medium">Panel</th>
                <th className="px-4 py-3 font-medium">API</th>
                <th className="px-4 py-3 font-medium">Rating</th>
                <th className="px-4 py-3 font-medium">Added</th>
                <th className="px-4 py-3 font-medium w-24">Actions</th>
              </tr>
            </thead>
            <tbody>
              {providers.map((p) => (
                <tr key={p.id} className="border-b last:border-0 hover:bg-gray-50">
                  <td className="px-4 py-3 font-medium text-gray-900">{p.name}</td>
                  <td className="px-4 py-3">{p.country || '-'}</td>
                  <td className="px-4 py-3">
                    {p.website ? (
                      <a
                        href={p.website}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-blue-600 hover:underline inline-flex items-center gap-1"
                      >
                        Link <ExternalLink size={12} />
                      </a>
                    ) : (
                      '-'
                    )}
                  </td>
                  <td className="px-4 py-3">
                    {p.panel_url ? (
                      <a
                        href={p.panel_url}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-blue-600 hover:underline inline-flex items-center gap-1"
                      >
                        Panel <ExternalLink size={12} />
                      </a>
                    ) : (
                      '-'
                    )}
                  </td>
                  <td className="px-4 py-3">
                    <span
                      className={
                        p.api_supported
                          ? 'text-green-600 font-medium'
                          : 'text-gray-400'
                      }
                    >
                      {p.api_supported ? 'Yes' : 'No'}
                    </span>
                  </td>
                  <td className="px-4 py-3">
                    {p.rating !== null ? `${p.rating}/10` : '-'}
                  </td>
                  <td className="px-4 py-3 text-gray-500">{formatDate(p.created_at)}</td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-1">
                      <button
                        title="Edit"
                        onClick={() => setEditId(p.id)}
                        className="p-1 text-gray-500 hover:bg-gray-100 rounded"
                      >
                        <Pencil size={15} />
                      </button>
                      <button
                        title="Delete"
                        onClick={() => setConfirmDelete(p.id)}
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

      {showCreate && (
        <ProviderFormDialog onClose={() => setShowCreate(false)} />
      )}

      {editId && (
        <ProviderFormDialog id={editId} onClose={() => setEditId(null)} />
      )}

      <ConfirmDialog
        open={!!confirmDelete}
        title="Delete Provider"
        message="This will permanently delete this provider. VPS records referencing it may be affected."
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
