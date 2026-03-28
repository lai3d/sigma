import { useEffect, useRef, useState } from 'react';
import { useForm } from 'react-hook-form';
import { Copy, Check } from 'lucide-react';
import { useCreateApiKey } from '@/hooks/useApiKeys';
import type { ApiKeyCreatedResponse } from '@/types/api';

interface Props {
  onClose: () => void;
}

interface FormData {
  name: string;
  role: string;
}

export default function ApiKeyFormDialog({ onClose }: Props) {
  const createMutation = useCreateApiKey();
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [createdKey, setCreatedKey] = useState<ApiKeyCreatedResponse | null>(null);
  const [copied, setCopied] = useState(false);

  const {
    register,
    handleSubmit,
    formState: { isSubmitting },
  } = useForm<FormData>({
    defaultValues: { name: '', role: 'readonly' },
  });

  useEffect(() => {
    dialogRef.current?.showModal();
  }, []);

  async function onSubmit(data: FormData) {
    const result = await createMutation.mutateAsync({
      name: data.name,
      role: data.role as 'admin' | 'operator' | 'readonly',
    });
    setCreatedKey(result);
  }

  async function copyKey() {
    if (createdKey) {
      await navigator.clipboard.writeText(createdKey.key);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  }

  // After key is created, show the key with copy button
  if (createdKey) {
    return (
      <dialog
        ref={dialogRef}
        onClose={onClose}
        className="rounded-lg shadow-xl p-0 backdrop:bg-black/40 w-full max-w-lg"
      >
        <div className="p-6 space-y-4">
          <h3 className="text-lg font-semibold text-gray-900">API Key Created</h3>
          <p className="text-sm text-gray-600">
            Copy this key now. You won't be able to see it again.
          </p>

          <div className="flex items-center gap-2 p-3 bg-gray-50 border rounded-md">
            <code className="flex-1 text-sm font-mono break-all text-gray-900">
              {createdKey.key}
            </code>
            <button
              onClick={copyKey}
              className="shrink-0 p-1.5 text-gray-500 hover:bg-gray-200 rounded"
              title="Copy"
            >
              {copied ? <Check size={16} className="text-green-600" /> : <Copy size={16} />}
            </button>
          </div>

          <div className="text-sm text-gray-500 space-y-1">
            <p><span className="font-medium">Name:</span> {createdKey.name}</p>
            <p><span className="font-medium">Role:</span> {createdKey.role}</p>
          </div>

          <div className="flex justify-end pt-2">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700"
            >
              Done
            </button>
          </div>
        </div>
      </dialog>
    );
  }

  return (
    <dialog
      ref={dialogRef}
      onClose={onClose}
      className="rounded-lg shadow-xl p-0 backdrop:bg-black/40 w-full max-w-lg"
    >
      <form onSubmit={handleSubmit(onSubmit)} className="p-6 space-y-4">
        <h3 className="text-lg font-semibold text-gray-900">Create API Key</h3>

        <div>
          <label className="block text-sm font-medium text-gray-700">Name *</label>
          <input
            {...register('name', { required: true })}
            className="input w-full mt-1"
            placeholder="e.g. vps-readonly-prod"
          />
        </div>

        <div>
          <label className="block text-sm font-medium text-gray-700">Role</label>
          <select {...register('role')} className="input w-full mt-1">
            <option value="readonly">Readonly</option>
            <option value="operator">Operator</option>
            <option value="agent">Agent</option>
            <option value="admin">Admin</option>
          </select>
        </div>

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
            {isSubmitting ? 'Creating...' : 'Create'}
          </button>
        </div>
      </form>
    </dialog>
  );
}
