import { useEffect } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { useForm } from 'react-hook-form';
import { useTicket, useCreateTicket, useUpdateTicket } from '@/hooks/useTickets';
import { useProviders } from '@/hooks/useProviders';
import { useUsers } from '@/hooks/useUsers';

interface FormData {
  title: string;
  description: string;
  priority: string;
  status: string;
  vps_id: string;
  provider_id: string;
  assigned_to: string;
}

export default function TicketForm() {
  const { id } = useParams<{ id: string }>();
  const isEdit = !!id;
  const navigate = useNavigate();

  const { data: existing } = useTicket(id || '');
  const { data: providersResult } = useProviders({ per_page: 100 });
  const { data: usersResult } = useUsers({ per_page: 100 });
  const providers = providersResult?.data;
  const users = usersResult?.data;
  const createMutation = useCreateTicket();
  const updateMutation = useUpdateTicket();

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<FormData>({
    defaultValues: {
      title: '',
      description: '',
      priority: 'medium',
      status: 'open',
      vps_id: '',
      provider_id: '',
      assigned_to: '',
    },
  });

  useEffect(() => {
    if (existing) {
      reset({
        title: existing.title,
        description: existing.description,
        priority: existing.priority,
        status: existing.status,
        vps_id: existing.vps_id || '',
        provider_id: existing.provider_id || '',
        assigned_to: existing.assigned_to || '',
      });
    }
  }, [existing, reset]);

  const onSubmit = async (data: FormData) => {
    const payload = {
      title: data.title,
      description: data.description || undefined,
      priority: data.priority as 'low' | 'medium' | 'high' | 'critical',
      vps_id: data.vps_id || null,
      provider_id: data.provider_id || null,
      assigned_to: data.assigned_to || null,
      ...(isEdit ? { status: data.status as 'open' | 'in-progress' | 'resolved' | 'closed' } : {}),
    };

    if (isEdit && id) {
      await updateMutation.mutateAsync({ id, data: payload });
      navigate(`/tickets/${id}`);
    } else {
      const created = await createMutation.mutateAsync(payload);
      navigate(`/tickets/${created.id}`);
    }
  };

  return (
    <div>
      <h2 className="text-2xl font-bold text-gray-900">
        {isEdit ? 'Edit Ticket' : 'New Ticket'}
      </h2>

      <form onSubmit={handleSubmit(onSubmit)} className="mt-6 max-w-2xl space-y-5">
        {/* Title */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">Title *</label>
          <input
            {...register('title', { required: 'Title is required' })}
            className="w-full border rounded-md px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
          {errors.title && <p className="mt-1 text-xs text-red-500">{errors.title.message}</p>}
        </div>

        {/* Description */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">Description</label>
          <textarea
            {...register('description')}
            rows={4}
            className="w-full border rounded-md px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        {/* Priority */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">Priority</label>
          <select
            {...register('priority')}
            className="border rounded-md px-3 py-2 text-sm bg-white"
          >
            <option value="low">Low</option>
            <option value="medium">Medium</option>
            <option value="high">High</option>
            <option value="critical">Critical</option>
          </select>
        </div>

        {/* Status (edit only) */}
        {isEdit && (
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Status</label>
            <select
              {...register('status')}
              className="border rounded-md px-3 py-2 text-sm bg-white"
            >
              <option value="open">Open</option>
              <option value="in-progress">In Progress</option>
              <option value="resolved">Resolved</option>
              <option value="closed">Closed</option>
            </select>
          </div>
        )}

        {/* Provider */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">Provider (optional)</label>
          <select
            {...register('provider_id')}
            className="border rounded-md px-3 py-2 text-sm bg-white w-full"
          >
            <option value="">None</option>
            {providers?.map((p) => (
              <option key={p.id} value={p.id}>{p.name}</option>
            ))}
          </select>
        </div>

        {/* Assigned To */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">Assigned To (optional)</label>
          <select
            {...register('assigned_to')}
            className="border rounded-md px-3 py-2 text-sm bg-white w-full"
          >
            <option value="">Unassigned</option>
            {users?.map((u) => (
              <option key={u.id} value={u.id}>{u.name || u.email}</option>
            ))}
          </select>
        </div>

        {/* Buttons */}
        <div className="flex items-center gap-3 pt-2">
          <button
            type="submit"
            disabled={isSubmitting}
            className="px-5 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50"
          >
            {isSubmitting ? 'Saving...' : isEdit ? 'Update Ticket' : 'Create Ticket'}
          </button>
          <button
            type="button"
            onClick={() => navigate(isEdit && id ? `/tickets/${id}` : '/tickets')}
            className="px-5 py-2 text-sm font-medium text-gray-700 border rounded-md hover:bg-gray-50"
          >
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}
