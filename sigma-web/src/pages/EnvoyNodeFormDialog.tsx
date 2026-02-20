import { useEffect, useRef } from 'react';
import { useForm } from 'react-hook-form';
import { useEnvoyNode, useCreateEnvoyNode, useUpdateEnvoyNode } from '@/hooks/useEnvoy';
import { useVpsList } from '@/hooks/useVps';

interface Props {
  id?: string;
  onClose: () => void;
}

interface FormData {
  vps_id: string;
  node_id: string;
  admin_port: string;
  description: string;
  status: string;
}

export default function EnvoyNodeFormDialog({ id, onClose }: Props) {
  const isEdit = !!id;
  const { data: existing } = useEnvoyNode(id || '');
  const createMutation = useCreateEnvoyNode();
  const updateMutation = useUpdateEnvoyNode();
  const { data: vpsResult } = useVpsList({ per_page: 100, status: 'active' });
  const vpsList = vpsResult?.data ?? [];

  const dialogRef = useRef<HTMLDialogElement>(null);

  const {
    register,
    handleSubmit,
    reset,
    formState: { isSubmitting },
  } = useForm<FormData>({
    defaultValues: {
      vps_id: '',
      node_id: '',
      admin_port: '',
      description: '',
      status: 'active',
    },
  });

  useEffect(() => {
    dialogRef.current?.showModal();
  }, []);

  useEffect(() => {
    if (existing) {
      reset({
        vps_id: existing.vps_id,
        node_id: existing.node_id,
        admin_port: existing.admin_port !== null ? String(existing.admin_port) : '',
        description: existing.description,
        status: existing.status,
      });
    }
  }, [existing, reset]);

  async function onSubmit(data: FormData) {
    if (isEdit && id) {
      await updateMutation.mutateAsync({
        id,
        data: {
          node_id: data.node_id,
          admin_port: data.admin_port ? Number(data.admin_port) : null,
          description: data.description,
          status: data.status,
        },
      });
    } else {
      await createMutation.mutateAsync({
        vps_id: data.vps_id,
        node_id: data.node_id,
        admin_port: data.admin_port ? Number(data.admin_port) : undefined,
        description: data.description || undefined,
      });
    }
    onClose();
  }

  return (
    <dialog
      ref={dialogRef}
      onClose={onClose}
      className="rounded-lg shadow-xl p-0 backdrop:bg-black/40 w-full max-w-lg"
    >
      <form onSubmit={handleSubmit(onSubmit)} className="p-6 space-y-4">
        <h3 className="text-lg font-semibold text-gray-900">
          {isEdit ? 'Edit Envoy Node' : 'Add Envoy Node'}
        </h3>

        <div>
          <label className="block text-sm font-medium text-gray-700">VPS *</label>
          <select
            {...register('vps_id', { required: true })}
            className="input w-full mt-1"
            disabled={isEdit}
          >
            <option value="">Select VPS...</option>
            {vpsList.map((v) => (
              <option key={v.id} value={v.id}>
                {v.hostname} ({v.ip_addresses?.[0]?.ip || v.country || v.id.slice(0, 8)})
              </option>
            ))}
          </select>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="block text-sm font-medium text-gray-700">Node ID *</label>
            <input
              {...register('node_id', { required: true })}
              className="input w-full mt-1"
              placeholder="layer4-01"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700">Admin Port</label>
            <input
              {...register('admin_port')}
              type="number"
              className="input w-full mt-1"
              placeholder="9911"
            />
          </div>
        </div>

        <div>
          <label className="block text-sm font-medium text-gray-700">Description</label>
          <input
            {...register('description')}
            className="input w-full mt-1"
            placeholder="Layer 4 proxy for HK region"
          />
        </div>

        {isEdit && (
          <div>
            <label className="block text-sm font-medium text-gray-700">Status</label>
            <select {...register('status')} className="input w-full mt-1">
              <option value="active">Active</option>
              <option value="disabled">Disabled</option>
            </select>
          </div>
        )}

        <div className="flex justify-end gap-3 pt-2">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-md hover:bg-gray-50"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={isSubmitting}
            className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50"
          >
            {isSubmitting ? 'Saving...' : isEdit ? 'Update' : 'Create'}
          </button>
        </div>
      </form>
    </dialog>
  );
}
