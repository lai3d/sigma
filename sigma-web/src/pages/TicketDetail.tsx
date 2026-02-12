import { useState } from 'react';
import { useParams, useNavigate, Link } from 'react-router-dom';
import { Pencil, Trash2 } from 'lucide-react';
import { useTicket, useUpdateTicket, useDeleteTicket, useTicketComments, useAddComment } from '@/hooks/useTickets';
import { useVps } from '@/hooks/useVps';
import { useProvider } from '@/hooks/useProviders';
import { useAuth } from '@/contexts/AuthContext';
import ConfirmDialog from '@/components/ConfirmDialog';
import { formatDate, timeAgo } from '@/lib/utils';
import type { TicketStatus } from '@/types/api';

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

export default function TicketDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { user } = useAuth();
  const canMutate = user?.role === 'admin' || user?.role === 'operator';
  const isAdmin = user?.role === 'admin';

  const { data: ticket, isLoading } = useTicket(id || '');
  const { data: comments } = useTicketComments(id || '');
  const { data: linkedVps } = useVps(ticket?.vps_id || '');
  const { data: linkedProvider } = useProvider(ticket?.provider_id || '');
  const updateMutation = useUpdateTicket();
  const deleteMutation = useDeleteTicket();
  const addCommentMutation = useAddComment();

  const [confirmDelete, setConfirmDelete] = useState(false);
  const [commentBody, setCommentBody] = useState('');

  if (isLoading) return <div className="p-8 text-center text-gray-500">Loading...</div>;
  if (!ticket) return <div className="p-8 text-center text-gray-400">Ticket not found</div>;

  const handleStatusChange = (status: TicketStatus) => {
    updateMutation.mutate({ id: ticket.id, data: { status } });
  };

  const handleAddComment = (e: React.FormEvent) => {
    e.preventDefault();
    if (!commentBody.trim()) return;
    addCommentMutation.mutate(
      { ticketId: ticket.id, body: commentBody },
      { onSuccess: () => setCommentBody('') },
    );
  };

  const handleDelete = () => {
    deleteMutation.mutate(ticket.id, { onSuccess: () => navigate('/tickets') });
  };

  return (
    <div>
      {/* Header */}
      <div className="flex items-start justify-between">
        <div>
          <h2 className="text-2xl font-bold text-gray-900">{ticket.title}</h2>
          <div className="mt-2 flex items-center gap-2">
            <span className={`inline-block px-2 py-0.5 text-xs font-medium rounded ${STATUS_COLORS[ticket.status] || ''}`}>
              {ticket.status}
            </span>
            <span className={`inline-block px-2 py-0.5 text-xs font-medium rounded ${PRIORITY_COLORS[ticket.priority] || ''}`}>
              {ticket.priority}
            </span>
          </div>
        </div>
        {canMutate && (
          <div className="flex items-center gap-2">
            <Link
              to={`/tickets/${ticket.id}/edit`}
              className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm border rounded-md hover:bg-gray-50"
            >
              <Pencil size={14} /> Edit
            </Link>
            {isAdmin && (
              <button
                onClick={() => setConfirmDelete(true)}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm text-red-600 border border-red-200 rounded-md hover:bg-red-50"
              >
                <Trash2 size={14} /> Delete
              </button>
            )}
          </div>
        )}
      </div>

      {/* Info panel */}
      <div className="mt-6 grid grid-cols-1 md:grid-cols-2 gap-6">
        <div className="bg-white rounded-lg border p-5">
          <h3 className="text-sm font-medium text-gray-500 mb-3">Details</h3>
          {ticket.description && (
            <p className="text-sm text-gray-700 whitespace-pre-wrap mb-4">{ticket.description}</p>
          )}
          <dl className="space-y-2 text-sm">
            {ticket.vps_id && (
              <div className="flex justify-between">
                <dt className="text-gray-500">VPS</dt>
                <dd>
                  <Link to={`/vps/${ticket.vps_id}`} className="text-blue-600 hover:underline">
                    {linkedVps?.hostname || ticket.vps_id.slice(0, 8) + '...'}
                  </Link>
                </dd>
              </div>
            )}
            {ticket.provider_id && (
              <div className="flex justify-between">
                <dt className="text-gray-500">Provider</dt>
                <dd className="text-gray-700">
                  {linkedProvider?.name || ticket.provider_id.slice(0, 8) + '...'}
                </dd>
              </div>
            )}
            <div className="flex justify-between">
              <dt className="text-gray-500">Created</dt>
              <dd className="text-gray-700">{formatDate(ticket.created_at)}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-gray-500">Updated</dt>
              <dd className="text-gray-700">{formatDate(ticket.updated_at)}</dd>
            </div>
          </dl>
        </div>

        {/* Quick status buttons */}
        {canMutate && (
          <div className="bg-white rounded-lg border p-5">
            <h3 className="text-sm font-medium text-gray-500 mb-3">Actions</h3>
            <div className="flex flex-wrap gap-2">
              {ticket.status === 'open' && (
                <button
                  onClick={() => handleStatusChange('in-progress')}
                  className="px-3 py-1.5 text-sm font-medium text-yellow-700 bg-yellow-50 border border-yellow-200 rounded-md hover:bg-yellow-100"
                >
                  Start Work
                </button>
              )}
              {(ticket.status === 'open' || ticket.status === 'in-progress') && (
                <button
                  onClick={() => handleStatusChange('resolved')}
                  className="px-3 py-1.5 text-sm font-medium text-green-700 bg-green-50 border border-green-200 rounded-md hover:bg-green-100"
                >
                  Resolve
                </button>
              )}
              {ticket.status !== 'closed' && (
                <button
                  onClick={() => handleStatusChange('closed')}
                  className="px-3 py-1.5 text-sm font-medium text-gray-700 bg-gray-50 border border-gray-200 rounded-md hover:bg-gray-100"
                >
                  Close
                </button>
              )}
              {(ticket.status === 'resolved' || ticket.status === 'closed') && (
                <button
                  onClick={() => handleStatusChange('open')}
                  className="px-3 py-1.5 text-sm font-medium text-blue-700 bg-blue-50 border border-blue-200 rounded-md hover:bg-blue-100"
                >
                  Reopen
                </button>
              )}
            </div>
          </div>
        )}
      </div>

      {/* Comments */}
      <div className="mt-6">
        <h3 className="text-lg font-semibold text-gray-900 mb-4">Comments</h3>

        {comments && comments.length > 0 ? (
          <div className="space-y-3">
            {comments.map((c) => (
              <div key={c.id} className="bg-white rounded-lg border p-4">
                <div className="flex items-center justify-between mb-2">
                  <span className="text-sm font-medium text-gray-700">{c.user_email}</span>
                  <span className="text-xs text-gray-400" title={formatDate(c.created_at)}>
                    {timeAgo(c.created_at)}
                  </span>
                </div>
                <p className="text-sm text-gray-600 whitespace-pre-wrap">{c.body}</p>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-sm text-gray-400">No comments yet.</p>
        )}

        {canMutate && (
          <form onSubmit={handleAddComment} className="mt-4">
            <textarea
              value={commentBody}
              onChange={(e) => setCommentBody(e.target.value)}
              placeholder="Add a comment..."
              rows={3}
              className="w-full border rounded-md px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
            <div className="mt-2 flex justify-end">
              <button
                type="submit"
                disabled={!commentBody.trim() || addCommentMutation.isPending}
                className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50"
              >
                {addCommentMutation.isPending ? 'Posting...' : 'Add Comment'}
              </button>
            </div>
          </form>
        )}
      </div>

      <ConfirmDialog
        open={confirmDelete}
        title="Delete Ticket"
        message="This will permanently delete this ticket and all its comments."
        confirmLabel="Delete"
        variant="danger"
        onConfirm={handleDelete}
        onCancel={() => setConfirmDelete(false)}
      />
    </div>
  );
}
