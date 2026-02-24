import { useEffect, useRef } from 'react';
import { useForm } from 'react-hook-form';
import {
  useCreateCloudflareAccount,
  useUpdateCloudflareAccount,
} from '@/hooks/useCloudflare';

interface Props {
  account?: { id: string; name: string } | null;
  onClose: () => void;
}

interface FormData {
  name: string;
  api_token: string;
}

export default function CloudflareAccountDialog({ account, onClose }: Props) {
  const isEdit = !!account;
  const createMutation = useCreateCloudflareAccount();
  const updateMutation = useUpdateCloudflareAccount();
  const dialogRef = useRef<HTMLDialogElement>(null);

  const {
    register,
    handleSubmit,
    reset,
    formState: { isSubmitting },
  } = useForm<FormData>({
    defaultValues: {
      name: '',
      api_token: '',
    },
  });

  useEffect(() => {
    dialogRef.current?.showModal();
  }, []);

  useEffect(() => {
    if (account) {
      reset({ name: account.name, api_token: '' });
    }
  }, [account, reset]);

  async function onSubmit(data: FormData) {
    try {
      if (isEdit && account) {
        await updateMutation.mutateAsync({
          id: account.id,
          data: {
            name: data.name || undefined,
            api_token: data.api_token || undefined,
          },
        });
      } else {
        await createMutation.mutateAsync({
          name: data.name,
          api_token: data.api_token,
        });
      }
      onClose();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Failed to save account';
      alert(msg);
    }
  }

  return (
    <dialog
      ref={dialogRef}
      onClose={onClose}
      className="rounded-lg shadow-xl p-0 backdrop:bg-black/40 w-full max-w-lg"
    >
      <form onSubmit={handleSubmit(onSubmit)} className="p-6 space-y-4">
        <h3 className="text-lg font-semibold text-gray-900">
          {isEdit ? 'Edit Cloudflare Account' : 'Add Cloudflare Account'}
        </h3>

        <div>
          <label className="block text-sm font-medium text-gray-700">Name *</label>
          <input
            {...register('name', { required: true })}
            className="input w-full mt-1"
            placeholder="My Cloudflare Account"
          />
        </div>

        <div>
          <label className="block text-sm font-medium text-gray-700">
            API Token {isEdit ? '(leave blank to keep current)' : '*'}
          </label>
          <input
            {...register('api_token', { required: !isEdit })}
            type="password"
            className="input w-full mt-1"
            placeholder={isEdit ? '••••••••' : 'CF API Token'}
          />
          <p className="mt-1 text-xs text-gray-500">
            Token needs Zone:Read, DNS:Read, and SSL:Read permissions
          </p>
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
            {isSubmitting ? 'Saving...' : isEdit ? 'Update' : 'Create'}
          </button>
        </div>
      </form>
    </dialog>
  );
}
