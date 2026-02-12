import { useState } from 'react';
import { Link } from 'react-router-dom';
import { Plus } from 'lucide-react';
import { useTickets, useDeleteTicket } from '@/hooks/useTickets';
import { useAuth } from '@/contexts/AuthContext';
import ConfirmDialog from '@/components/ConfirmDialog';
import Pagination from '@/components/Pagination';
import { formatDate } from '@/lib/utils';
import type { TicketListQuery } from '@/types/api';

const STATUS_COLORS: Record<string, string> = {
  open: 'bg-blue-100 text-blue-800',
  'in-progress': 'bg-yellow-100 text-yellow-800',
  resolved: 'bg-green-100 text-green-800',
  closed: 'bg-gray-100 text-gray-600',
};

const PRIORITY_COLORS: Record<string, string> = {
  low: 'bg-gray-100 text-gray-600',
  medium: 'bg-blue-100 text-blue-700',
  high: 'bg-orange-100 text-orange-700',
  critical: 'bg-red-100 text-red-700',
};

export default function TicketList() {
  const { user } = useAuth();
  const canMutate = user?.role === 'admin' || user?.role === 'operator';

  const [filters, setFilters] = useState<TicketListQuery>({});
  const [page, setPage] = useState(1);
  const { data: result, isLoading } = useTickets({ ...filters, page, per_page: 25 });
  const deleteMutation = useDeleteTicket();
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);

  const tickets = result?.data;

  const handleFilterChange = (newFilters: TicketListQuery) => {
    setFilters(newFilters);
    setPage(1);
  };

  return (
    <div>
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold text-gray-900">Tickets</h2>
        {canMutate && (
          <Link
            to="/tickets/new"
            className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700"
          >
            <Plus size={16} /> New Ticket
          </Link>
        )}
      </div>

      {/* Filters */}
      <div className="mt-4 flex flex-wrap gap-3">
        <select
          value={filters.status || ''}
          onChange={(e) => handleFilterChange({ ...filters, status: e.target.value || undefined })}
          className="border rounded-md px-3 py-1.5 text-sm bg-white"
        >
          <option value="">All Statuses</option>
          <option value="open">Open</option>
          <option value="in-progress">In Progress</option>
          <option value="resolved">Resolved</option>
          <option value="closed">Closed</option>
        </select>

        <select
          value={filters.priority || ''}
          onChange={(e) => handleFilterChange({ ...filters, priority: e.target.value || undefined })}
          className="border rounded-md px-3 py-1.5 text-sm bg-white"
        >
          <option value="">All Priorities</option>
          <option value="critical">Critical</option>
          <option value="high">High</option>
          <option value="medium">Medium</option>
          <option value="low">Low</option>
        </select>
      </div>

      {/* Table */}
      <div className="mt-4 bg-white rounded-lg border overflow-x-auto">
        {isLoading ? (
          <div className="p-8 text-center text-gray-500">Loading...</div>
        ) : !tickets?.length ? (
          <div className="p-8 text-center text-gray-400">No tickets found</div>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-gray-500 border-b bg-gray-50">
                <th className="px-4 py-3 font-medium">Priority</th>
                <th className="px-4 py-3 font-medium">Title</th>
                <th className="px-4 py-3 font-medium">Status</th>
                <th className="px-4 py-3 font-medium">Created</th>
                <th className="px-4 py-3 font-medium">Updated</th>
              </tr>
            </thead>
            <tbody>
              {tickets.map((t) => (
                <tr key={t.id} className="border-b last:border-0 hover:bg-gray-50">
                  <td className="px-4 py-3">
                    <span className={`inline-block px-2 py-0.5 text-xs font-medium rounded ${PRIORITY_COLORS[t.priority] || ''}`}>
                      {t.priority}
                    </span>
                  </td>
                  <td className="px-4 py-3">
                    <Link
                      to={`/tickets/${t.id}`}
                      className="text-blue-600 hover:underline font-medium"
                    >
                      {t.title}
                    </Link>
                  </td>
                  <td className="px-4 py-3">
                    <span className={`inline-block px-2 py-0.5 text-xs font-medium rounded ${STATUS_COLORS[t.status] || ''}`}>
                      {t.status}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-gray-500">{formatDate(t.created_at)}</td>
                  <td className="px-4 py-3 text-gray-500">{formatDate(t.updated_at)}</td>
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

      <ConfirmDialog
        open={!!confirmDelete}
        title="Delete Ticket"
        message="This will permanently delete this ticket and all its comments."
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
