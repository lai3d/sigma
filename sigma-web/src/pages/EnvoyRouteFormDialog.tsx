import { useEffect, useRef } from 'react';
import { useForm } from 'react-hook-form';
import { useEnvoyRoute, useCreateEnvoyRoute, useUpdateEnvoyRoute } from '@/hooks/useEnvoy';
import type { EnvoyNode } from '@/types/api';

interface Props {
  id?: string;
  nodes: EnvoyNode[];
  onClose: () => void;
}

interface FormData {
  envoy_node_id: string;
  name: string;
  listen_port: string;
  backend_host: string;
  backend_port: string;
  cluster_type: string;
  connect_timeout_secs: string;
  proxy_protocol: string;
  status: string;
}

export default function EnvoyRouteFormDialog({ id, nodes, onClose }: Props) {
  const isEdit = !!id;
  const { data: existing } = useEnvoyRoute(id || '');
  const createMutation = useCreateEnvoyRoute();
  const updateMutation = useUpdateEnvoyRoute();

  const dialogRef = useRef<HTMLDialogElement>(null);

  const {
    register,
    handleSubmit,
    reset,
    formState: { isSubmitting },
  } = useForm<FormData>({
    defaultValues: {
      envoy_node_id: '',
      name: '',
      listen_port: '',
      backend_host: '',
      backend_port: '',
      cluster_type: 'logical_dns',
      connect_timeout_secs: '5',
      proxy_protocol: '1',
      status: 'active',
    },
  });

  useEffect(() => {
    dialogRef.current?.showModal();
  }, []);

  useEffect(() => {
    if (existing) {
      reset({
        envoy_node_id: existing.envoy_node_id,
        name: existing.name,
        listen_port: String(existing.listen_port),
        backend_host: existing.backend_host || '',
        backend_port: existing.backend_port !== null ? String(existing.backend_port) : '',
        cluster_type: existing.cluster_type,
        connect_timeout_secs: String(existing.connect_timeout_secs),
        proxy_protocol: String(existing.proxy_protocol),
        status: existing.status,
      });
    }
  }, [existing, reset]);

  async function onSubmit(data: FormData) {
    if (isEdit && id) {
      await updateMutation.mutateAsync({
        id,
        data: {
          name: data.name,
          listen_port: Number(data.listen_port),
          backend_host: data.backend_host || null,
          backend_port: data.backend_port ? Number(data.backend_port) : null,
          cluster_type: data.cluster_type,
          connect_timeout_secs: Number(data.connect_timeout_secs),
          proxy_protocol: Number(data.proxy_protocol),
          status: data.status,
        },
      });
    } else {
      await createMutation.mutateAsync({
        envoy_node_id: data.envoy_node_id,
        name: data.name,
        listen_port: Number(data.listen_port),
        backend_host: data.backend_host || undefined,
        backend_port: data.backend_port ? Number(data.backend_port) : undefined,
        cluster_type: data.cluster_type,
        connect_timeout_secs: Number(data.connect_timeout_secs),
        proxy_protocol: Number(data.proxy_protocol),
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
          {isEdit ? 'Edit Envoy Route' : 'Add Envoy Route'}
        </h3>

        <div>
          <label className="block text-sm font-medium text-gray-700">Envoy Node *</label>
          <select
            {...register('envoy_node_id', { required: true })}
            className="input w-full mt-1"
            disabled={isEdit}
          >
            <option value="">Select node...</option>
            {nodes.map((n) => (
              <option key={n.id} value={n.id}>
                {n.node_id}
              </option>
            ))}
          </select>
        </div>

        <div>
          <label className="block text-sm font-medium text-gray-700">Name *</label>
          <input
            {...register('name', { required: true })}
            className="input w-full mt-1"
            placeholder="tcp-proxy-hk01"
          />
        </div>

        <div className="grid grid-cols-3 gap-4">
          <div>
            <label className="block text-sm font-medium text-gray-700">Listen Port *</label>
            <input
              {...register('listen_port', { required: true })}
              type="number"
              className="input w-full mt-1"
              placeholder="30008"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700">Backend Host</label>
            <input
              {...register('backend_host')}
              className="input w-full mt-1"
              placeholder="backend.example.com"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700">Backend Port</label>
            <input
              {...register('backend_port')}
              type="number"
              className="input w-full mt-1"
              placeholder="30008"
            />
          </div>
        </div>

        <div className="grid grid-cols-3 gap-4">
          <div>
            <label className="block text-sm font-medium text-gray-700">Cluster Type</label>
            <select {...register('cluster_type')} className="input w-full mt-1">
              <option value="logical_dns">logical_dns</option>
              <option value="static">static</option>
              <option value="strict_dns">strict_dns</option>
            </select>
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700">Timeout (s)</label>
            <input
              {...register('connect_timeout_secs')}
              type="number"
              min="1"
              className="input w-full mt-1"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700">Proxy Protocol</label>
            <select {...register('proxy_protocol')} className="input w-full mt-1">
              <option value="0">None</option>
              <option value="1">v1</option>
              <option value="2">v2</option>
            </select>
          </div>
        </div>

        {isEdit && (
          <div>
            <label className="block text-sm font-medium text-gray-700">Status</label>
            <select {...register('status')} className="input w-full mt-1">
              <option value="active">Active</option>
              <option value="placeholder">Placeholder</option>
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
