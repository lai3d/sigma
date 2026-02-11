import { useEffect, useRef } from 'react';
import { useForm } from 'react-hook-form';
import { useUser, useCreateUser, useUpdateUser } from '@/hooks/useUsers';

interface Props {
  id?: string;
  onClose: () => void;
}

interface FormData {
  email: string;
  name: string;
  role: string;
  password: string;
  force_password_change: boolean;
}

export default function UserFormDialog({ id, onClose }: Props) {
  const isEdit = !!id;
  const { data: existing } = useUser(id || '');
  const createMutation = useCreateUser();
  const updateMutation = useUpdateUser();

  const dialogRef = useRef<HTMLDialogElement>(null);

  const {
    register,
    handleSubmit,
    reset,
    formState: { isSubmitting },
  } = useForm<FormData>({
    defaultValues: {
      email: '',
      name: '',
      role: 'readonly',
      password: '',
      force_password_change: false,
    },
  });

  useEffect(() => {
    dialogRef.current?.showModal();
  }, []);

  useEffect(() => {
    if (existing) {
      reset({
        email: existing.email,
        name: existing.name,
        role: existing.role,
        password: '',
        force_password_change: existing.force_password_change,
      });
    }
  }, [existing, reset]);

  async function onSubmit(data: FormData) {
    if (isEdit && id) {
      await updateMutation.mutateAsync({
        id,
        data: {
          email: data.email || undefined,
          name: data.name,
          role: data.role as 'admin' | 'operator' | 'readonly',
          password: data.password || undefined,
          force_password_change: data.force_password_change,
        },
      });
    } else {
      await createMutation.mutateAsync({
        email: data.email,
        password: data.password,
        name: data.name || undefined,
        role: data.role as 'admin' | 'operator' | 'readonly',
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
          {isEdit ? 'Edit User' : 'Add User'}
        </h3>

        <div>
          <label className="block text-sm font-medium text-gray-700">Email *</label>
          <input
            {...register('email', { required: true })}
            type="email"
            className="input w-full mt-1"
            placeholder="user@example.com"
          />
        </div>

        <div>
          <label className="block text-sm font-medium text-gray-700">Name</label>
          <input {...register('name')} className="input w-full mt-1" placeholder="John Doe" />
        </div>

        <div>
          <label className="block text-sm font-medium text-gray-700">Role</label>
          <select {...register('role')} className="input w-full mt-1">
            <option value="readonly">Readonly</option>
            <option value="operator">Operator</option>
            <option value="admin">Admin</option>
          </select>
        </div>

        <div>
          <label className="block text-sm font-medium text-gray-700">
            {isEdit ? 'New Password (leave blank to keep)' : 'Password *'}
          </label>
          <input
            {...register('password', { required: !isEdit })}
            type="password"
            className="input w-full mt-1"
            placeholder={isEdit ? 'Leave blank to keep current' : 'Min 6 characters'}
            minLength={isEdit ? 0 : 6}
          />
        </div>

        {isEdit && (
          <label className="flex items-center gap-2 text-sm">
            <input type="checkbox" {...register('force_password_change')} className="rounded" />
            Force password change on next login
          </label>
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
