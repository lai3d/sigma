import { useState } from 'react';
import { Plus, Pencil, Trash2 } from 'lucide-react';
import { useUsers, useDeleteUser } from '@/hooks/useUsers';
import ConfirmDialog from '@/components/ConfirmDialog';
import Pagination from '@/components/Pagination';
import { formatDate } from '@/lib/utils';
import UserFormDialog from './UserFormDialog';

const ROLE_BADGE: Record<string, string> = {
  admin: 'bg-red-50 text-red-700',
  operator: 'bg-blue-50 text-blue-700',
  readonly: 'bg-gray-100 text-gray-600',
};

export default function UserList() {
  const [page, setPage] = useState(1);
  const { data: result, isLoading } = useUsers({ page, per_page: 25 });
  const deleteMutation = useDeleteUser();

  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [editId, setEditId] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);

  const users = result?.data;

  return (
    <div>
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold text-gray-900">Users</h2>
        <button
          onClick={() => setShowCreate(true)}
          className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700"
        >
          <Plus size={16} /> Add User
        </button>
      </div>

      <div className="mt-4 bg-white rounded-lg border overflow-x-auto">
        {isLoading ? (
          <div className="p-8 text-center text-gray-500">Loading...</div>
        ) : !users?.length ? (
          <div className="p-8 text-center text-gray-400">No users yet</div>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-gray-500 border-b bg-gray-50">
                <th className="px-4 py-3 font-medium">Email</th>
                <th className="px-4 py-3 font-medium">Name</th>
                <th className="px-4 py-3 font-medium">Role</th>
                <th className="px-4 py-3 font-medium">Created</th>
                <th className="px-4 py-3 font-medium w-24">Actions</th>
              </tr>
            </thead>
            <tbody>
              {users.map((u) => (
                <tr key={u.id} className="border-b last:border-0 hover:bg-gray-50">
                  <td className="px-4 py-3 font-medium text-gray-900">{u.email}</td>
                  <td className="px-4 py-3">{u.name || '-'}</td>
                  <td className="px-4 py-3">
                    <span className={`inline-block px-2 py-0.5 rounded text-xs font-medium ${ROLE_BADGE[u.role] || ''}`}>
                      {u.role}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-gray-500">{formatDate(u.created_at)}</td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-1">
                      <button
                        title="Edit"
                        onClick={() => setEditId(u.id)}
                        className="p-1 text-gray-500 hover:bg-gray-100 rounded"
                      >
                        <Pencil size={15} />
                      </button>
                      <button
                        title="Delete"
                        onClick={() => setConfirmDelete(u.id)}
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
        <UserFormDialog onClose={() => setShowCreate(false)} />
      )}

      {editId && (
        <UserFormDialog id={editId} onClose={() => setEditId(null)} />
      )}

      <ConfirmDialog
        open={!!confirmDelete}
        title="Delete User"
        message="This will permanently delete this user account."
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
