import { useEffect, useRef } from 'react';
import { useForm } from 'react-hook-form';
import {
  useCreateCloudAccount,
  useUpdateCloudAccount,
} from '@/hooks/useCloudAccounts';
import type { CloudProviderType } from '@/types/api';

interface Props {
  account?: { id: string; name: string; provider_type: CloudProviderType } | null;
  onClose: () => void;
}

interface FormData {
  name: string;
  provider_type: CloudProviderType;
  // AWS
  aws_access_key_id: string;
  aws_secret_access_key: string;
  aws_regions: string;
  // Alibaba
  ali_access_key_id: string;
  ali_access_key_secret: string;
  ali_regions: string;
  // DigitalOcean
  do_api_token: string;
  // Linode
  linode_api_token: string;
  // Volcengine
  volc_access_key_id: string;
  volc_secret_access_key: string;
  volc_regions: string;
}

const PROVIDER_LABELS: Record<CloudProviderType, string> = {
  aws: 'AWS EC2',
  alibaba: 'Alibaba Cloud ECS',
  digitalocean: 'DigitalOcean',
  linode: 'Linode (Akamai)',
  volcengine: 'Volcengine (火山引擎)',
};

function buildConfig(data: FormData): Record<string, unknown> {
  switch (data.provider_type) {
    case 'aws':
      return {
        access_key_id: data.aws_access_key_id,
        secret_access_key: data.aws_secret_access_key,
        regions: data.aws_regions
          .split(',')
          .map((r) => r.trim())
          .filter(Boolean),
      };
    case 'alibaba':
      return {
        access_key_id: data.ali_access_key_id,
        access_key_secret: data.ali_access_key_secret,
        regions: data.ali_regions
          .split(',')
          .map((r) => r.trim())
          .filter(Boolean),
      };
    case 'digitalocean':
      return { api_token: data.do_api_token };
    case 'linode':
      return { api_token: data.linode_api_token };
    case 'volcengine':
      return {
        access_key_id: data.volc_access_key_id,
        secret_access_key: data.volc_secret_access_key,
        regions: data.volc_regions
          .split(',')
          .map((r) => r.trim())
          .filter(Boolean),
      };
  }
}

export default function CloudAccountDialog({ account, onClose }: Props) {
  const isEdit = !!account;
  const createMutation = useCreateCloudAccount();
  const updateMutation = useUpdateCloudAccount();
  const dialogRef = useRef<HTMLDialogElement>(null);

  const {
    register,
    handleSubmit,
    watch,
    reset,
    formState: { isSubmitting },
  } = useForm<FormData>({
    defaultValues: {
      name: '',
      provider_type: 'aws',
      aws_access_key_id: '',
      aws_secret_access_key: '',
      aws_regions: 'us-east-1',
      ali_access_key_id: '',
      ali_access_key_secret: '',
      ali_regions: 'cn-hangzhou',
      do_api_token: '',
      linode_api_token: '',
      volc_access_key_id: '',
      volc_secret_access_key: '',
      volc_regions: 'cn-beijing',
    },
  });

  const providerType = watch('provider_type');

  useEffect(() => {
    dialogRef.current?.showModal();
  }, []);

  useEffect(() => {
    if (account) {
      reset({
        name: account.name,
        provider_type: account.provider_type,
        aws_access_key_id: '',
        aws_secret_access_key: '',
        aws_regions: 'us-east-1',
        ali_access_key_id: '',
        ali_access_key_secret: '',
        ali_regions: 'cn-hangzhou',
        do_api_token: '',
        linode_api_token: '',
        volc_access_key_id: '',
        volc_secret_access_key: '',
        volc_regions: 'cn-beijing',
      });
    }
  }, [account, reset]);

  async function onSubmit(data: FormData) {
    try {
      if (isEdit && account) {
        const config = buildConfig(data);
        // Only include config if any credential value was provided
        const hasConfig =
          data.provider_type === 'aws'
            ? data.aws_access_key_id !== '' || data.aws_secret_access_key !== ''
            : data.provider_type === 'alibaba'
              ? data.ali_access_key_id !== '' || data.ali_access_key_secret !== ''
              : data.provider_type === 'digitalocean'
                ? data.do_api_token !== ''
                : data.provider_type === 'linode'
                  ? data.linode_api_token !== ''
                  : data.volc_access_key_id !== '' || data.volc_secret_access_key !== '';
        await updateMutation.mutateAsync({
          id: account.id,
          data: {
            name: data.name || undefined,
            config: hasConfig ? config : undefined,
          },
        });
      } else {
        await createMutation.mutateAsync({
          name: data.name,
          provider_type: data.provider_type,
          config: buildConfig(data),
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
          {isEdit ? 'Edit Cloud Account' : 'Add Cloud Account'}
        </h3>

        <div>
          <label className="block text-sm font-medium text-gray-700">Name *</label>
          <input
            {...register('name', { required: true })}
            className="input w-full mt-1"
            placeholder="My AWS Account"
          />
        </div>

        {!isEdit && (
          <div>
            <label className="block text-sm font-medium text-gray-700">Provider *</label>
            <select
              {...register('provider_type')}
              className="input w-full mt-1"
            >
              {(Object.keys(PROVIDER_LABELS) as CloudProviderType[]).map((pt) => (
                <option key={pt} value={pt}>{PROVIDER_LABELS[pt]}</option>
              ))}
            </select>
          </div>
        )}

        {isEdit && (
          <div>
            <label className="block text-sm font-medium text-gray-700">Provider</label>
            <p className="mt-1 text-sm text-gray-600">
              {PROVIDER_LABELS[account!.provider_type]}
            </p>
          </div>
        )}

        {/* AWS fields */}
        {providerType === 'aws' && (
          <>
            <div>
              <label className="block text-sm font-medium text-gray-700">
                Access Key ID {isEdit ? '(leave blank to keep current)' : '*'}
              </label>
              <input
                {...register('aws_access_key_id', { required: !isEdit })}
                type="password"
                className="input w-full mt-1"
                placeholder={isEdit ? '••••••••' : 'AKIA...'}
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700">
                Secret Access Key {isEdit ? '(leave blank to keep current)' : '*'}
              </label>
              <input
                {...register('aws_secret_access_key', { required: !isEdit })}
                type="password"
                className="input w-full mt-1"
                placeholder={isEdit ? '••••••••' : 'Secret Access Key'}
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700">Regions</label>
              <input
                {...register('aws_regions')}
                className="input w-full mt-1"
                placeholder="us-east-1, ap-southeast-1"
              />
              <p className="mt-1 text-xs text-gray-500">
                Comma-separated AWS region IDs
              </p>
            </div>
          </>
        )}

        {/* Alibaba fields */}
        {providerType === 'alibaba' && (
          <>
            <div>
              <label className="block text-sm font-medium text-gray-700">
                Access Key ID {isEdit ? '(leave blank to keep current)' : '*'}
              </label>
              <input
                {...register('ali_access_key_id', { required: !isEdit })}
                type="password"
                className="input w-full mt-1"
                placeholder={isEdit ? '••••••••' : 'LTAI...'}
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700">
                Access Key Secret {isEdit ? '(leave blank to keep current)' : '*'}
              </label>
              <input
                {...register('ali_access_key_secret', { required: !isEdit })}
                type="password"
                className="input w-full mt-1"
                placeholder={isEdit ? '••••••••' : 'Access Key Secret'}
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700">Regions</label>
              <input
                {...register('ali_regions')}
                className="input w-full mt-1"
                placeholder="cn-hangzhou, cn-beijing"
              />
              <p className="mt-1 text-xs text-gray-500">
                Comma-separated Alibaba Cloud region IDs
              </p>
            </div>
          </>
        )}

        {/* DigitalOcean fields */}
        {providerType === 'digitalocean' && (
          <div>
            <label className="block text-sm font-medium text-gray-700">
              API Token {isEdit ? '(leave blank to keep current)' : '*'}
            </label>
            <input
              {...register('do_api_token', { required: !isEdit })}
              type="password"
              className="input w-full mt-1"
              placeholder={isEdit ? '••••••••' : 'dop_v1_...'}
            />
          </div>
        )}

        {/* Linode fields */}
        {providerType === 'linode' && (
          <div>
            <label className="block text-sm font-medium text-gray-700">
              API Token {isEdit ? '(leave blank to keep current)' : '*'}
            </label>
            <input
              {...register('linode_api_token', { required: !isEdit })}
              type="password"
              className="input w-full mt-1"
              placeholder={isEdit ? '••••••••' : 'Linode Personal Access Token'}
            />
          </div>
        )}

        {/* Volcengine fields */}
        {providerType === 'volcengine' && (
          <>
            <div>
              <label className="block text-sm font-medium text-gray-700">
                Access Key ID {isEdit ? '(leave blank to keep current)' : '*'}
              </label>
              <input
                {...register('volc_access_key_id', { required: !isEdit })}
                type="password"
                className="input w-full mt-1"
                placeholder={isEdit ? '••••••••' : 'AK...'}
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700">
                Secret Access Key {isEdit ? '(leave blank to keep current)' : '*'}
              </label>
              <input
                {...register('volc_secret_access_key', { required: !isEdit })}
                type="password"
                className="input w-full mt-1"
                placeholder={isEdit ? '••••••••' : 'Secret Access Key'}
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700">Regions</label>
              <input
                {...register('volc_regions')}
                className="input w-full mt-1"
                placeholder="cn-beijing, cn-shanghai"
              />
              <p className="mt-1 text-xs text-gray-500">
                Comma-separated region IDs
              </p>
            </div>
          </>
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
