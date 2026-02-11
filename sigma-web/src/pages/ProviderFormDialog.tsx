import { useEffect, useRef } from 'react';
import { useForm } from 'react-hook-form';
import { useProvider, useCreateProvider, useUpdateProvider } from '@/hooks/useProviders';

interface Props {
  id?: string;
  onClose: () => void;
}

interface FormData {
  name: string;
  country: string;
  website: string;
  panel_url: string;
  api_supported: boolean;
  rating: string;
  notes: string;
}

export default function ProviderFormDialog({ id, onClose }: Props) {
  const isEdit = !!id;
  const { data: existing } = useProvider(id || '');
  const createMutation = useCreateProvider();
  const updateMutation = useUpdateProvider();

  const dialogRef = useRef<HTMLDialogElement>(null);

  const {
    register,
    handleSubmit,
    reset,
    formState: { isSubmitting },
  } = useForm<FormData>({
    defaultValues: {
      name: '',
      country: '',
      website: '',
      panel_url: '',
      api_supported: false,
      rating: '',
      notes: '',
    },
  });

  useEffect(() => {
    dialogRef.current?.showModal();
  }, []);

  useEffect(() => {
    if (existing) {
      reset({
        name: existing.name,
        country: existing.country,
        website: existing.website,
        panel_url: existing.panel_url,
        api_supported: existing.api_supported,
        rating: existing.rating !== null ? String(existing.rating) : '',
        notes: existing.notes,
      });
    }
  }, [existing, reset]);

  async function onSubmit(data: FormData) {
    const payload = {
      name: data.name,
      country: data.country || undefined,
      website: data.website || undefined,
      panel_url: data.panel_url || undefined,
      api_supported: data.api_supported,
      rating: data.rating ? Number(data.rating) : null,
      notes: data.notes || undefined,
    };

    if (isEdit && id) {
      await updateMutation.mutateAsync({ id, data: payload });
    } else {
      await createMutation.mutateAsync(payload);
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
          {isEdit ? 'Edit Provider' : 'Add Provider'}
        </h3>

        <div>
          <label className="block text-sm font-medium text-gray-700">Name *</label>
          <input
            {...register('name', { required: true })}
            className="input w-full mt-1"
            placeholder="BandwagonHost"
          />
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="block text-sm font-medium text-gray-700">Country</label>
            <input {...register('country')} className="input w-full mt-1" placeholder="US" />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700">Rating (1-10)</label>
            <input
              {...register('rating')}
              type="number"
              min="1"
              max="10"
              className="input w-full mt-1"
            />
          </div>
        </div>

        <div>
          <label className="block text-sm font-medium text-gray-700">Website</label>
          <input
            {...register('website')}
            className="input w-full mt-1"
            placeholder="https://bandwagonhost.com"
          />
        </div>

        <div>
          <label className="block text-sm font-medium text-gray-700">Panel URL</label>
          <input
            {...register('panel_url')}
            className="input w-full mt-1"
            placeholder="https://kiwivm.64clouds.com"
          />
        </div>

        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" {...register('api_supported')} className="rounded" />
          API Supported
        </label>

        <div>
          <label className="block text-sm font-medium text-gray-700">Notes</label>
          <textarea {...register('notes')} rows={2} className="input w-full mt-1" />
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
