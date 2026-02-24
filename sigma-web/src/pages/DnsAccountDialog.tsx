import { useEffect, useRef } from 'react';
import { useForm } from 'react-hook-form';
import {
  useCreateDnsAccount,
  useUpdateDnsAccount,
} from '@/hooks/useDns';
import type { DnsProviderType } from '@/types/api';

interface Props {
  account?: { id: string; name: string; provider_type: DnsProviderType } | null;
  onClose: () => void;
}

interface FormData {
  name: string;
  provider_type: DnsProviderType;
  // Cloudflare
  cf_api_token: string;
  // Route 53
  r53_access_key_id: string;
  r53_secret_access_key: string;
  r53_region: string;
  // GoDaddy
  gd_api_key: string;
  gd_api_secret: string;
  // Name.com
  nc_username: string;
  nc_api_token: string;
}

const PROVIDER_LABELS: Record<DnsProviderType, string> = {
  cloudflare: 'Cloudflare',
  route53: 'AWS Route 53',
  godaddy: 'GoDaddy',
  namecom: 'Name.com',
};

function buildConfig(data: FormData): Record<string, string> {
  switch (data.provider_type) {
    case 'cloudflare':
      return { api_token: data.cf_api_token };
    case 'route53':
      return {
        access_key_id: data.r53_access_key_id,
        secret_access_key: data.r53_secret_access_key,
        region: data.r53_region || 'us-east-1',
      };
    case 'godaddy':
      return { api_key: data.gd_api_key, api_secret: data.gd_api_secret };
    case 'namecom':
      return { username: data.nc_username, api_token: data.nc_api_token };
  }
}

export default function DnsAccountDialog({ account, onClose }: Props) {
  const isEdit = !!account;
  const createMutation = useCreateDnsAccount();
  const updateMutation = useUpdateDnsAccount();
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
      provider_type: 'cloudflare',
      cf_api_token: '',
      r53_access_key_id: '',
      r53_secret_access_key: '',
      r53_region: 'us-east-1',
      gd_api_key: '',
      gd_api_secret: '',
      nc_username: '',
      nc_api_token: '',
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
        cf_api_token: '',
        r53_access_key_id: '',
        r53_secret_access_key: '',
        r53_region: 'us-east-1',
        gd_api_key: '',
        gd_api_secret: '',
        nc_username: '',
        nc_api_token: '',
      });
    }
  }, [account, reset]);

  async function onSubmit(data: FormData) {
    try {
      if (isEdit && account) {
        const config = buildConfig(data);
        // Only include config if any value was provided
        const hasConfig = Object.values(config).some((v) => v !== '' && v !== 'us-east-1');
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
          {isEdit ? 'Edit DNS Account' : 'Add DNS Account'}
        </h3>

        <div>
          <label className="block text-sm font-medium text-gray-700">Name *</label>
          <input
            {...register('name', { required: true })}
            className="input w-full mt-1"
            placeholder="My DNS Account"
          />
        </div>

        {!isEdit && (
          <div>
            <label className="block text-sm font-medium text-gray-700">Provider *</label>
            <select
              {...register('provider_type')}
              className="input w-full mt-1"
            >
              {(Object.keys(PROVIDER_LABELS) as DnsProviderType[]).map((pt) => (
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

        {/* Provider-specific fields */}
        {providerType === 'cloudflare' && (
          <div>
            <label className="block text-sm font-medium text-gray-700">
              API Token {isEdit ? '(leave blank to keep current)' : '*'}
            </label>
            <input
              {...register('cf_api_token', { required: !isEdit })}
              type="password"
              className="input w-full mt-1"
              placeholder={isEdit ? '••••••••' : 'Cloudflare API Token'}
            />
            <p className="mt-1 text-xs text-gray-500">
              Token needs Zone:Read, DNS:Read, and SSL:Read permissions
            </p>
          </div>
        )}

        {providerType === 'route53' && (
          <>
            <div>
              <label className="block text-sm font-medium text-gray-700">
                Access Key ID {isEdit ? '(leave blank to keep current)' : '*'}
              </label>
              <input
                {...register('r53_access_key_id', { required: !isEdit })}
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
                {...register('r53_secret_access_key', { required: !isEdit })}
                type="password"
                className="input w-full mt-1"
                placeholder={isEdit ? '••••••••' : 'Secret Access Key'}
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700">Region</label>
              <select
                {...register('r53_region')}
                className="input w-full mt-1"
              >
                {['us-east-1', 'us-west-2', 'eu-west-1', 'eu-central-1', 'ap-northeast-1', 'ap-southeast-1'].map((r) => (
                  <option key={r} value={r}>{r}</option>
                ))}
              </select>
            </div>
          </>
        )}

        {providerType === 'godaddy' && (
          <>
            <div>
              <label className="block text-sm font-medium text-gray-700">
                API Key {isEdit ? '(leave blank to keep current)' : '*'}
              </label>
              <input
                {...register('gd_api_key', { required: !isEdit })}
                type="password"
                className="input w-full mt-1"
                placeholder={isEdit ? '••••••••' : 'GoDaddy API Key'}
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700">
                API Secret {isEdit ? '(leave blank to keep current)' : '*'}
              </label>
              <input
                {...register('gd_api_secret', { required: !isEdit })}
                type="password"
                className="input w-full mt-1"
                placeholder={isEdit ? '••••••••' : 'GoDaddy API Secret'}
              />
            </div>
          </>
        )}

        {providerType === 'namecom' && (
          <>
            <div>
              <label className="block text-sm font-medium text-gray-700">
                Username {isEdit ? '(leave blank to keep current)' : '*'}
              </label>
              <input
                {...register('nc_username', { required: !isEdit })}
                className="input w-full mt-1"
                placeholder={isEdit ? '••••••••' : 'Name.com username'}
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700">
                API Token {isEdit ? '(leave blank to keep current)' : '*'}
              </label>
              <input
                {...register('nc_api_token', { required: !isEdit })}
                type="password"
                className="input w-full mt-1"
                placeholder={isEdit ? '••••••••' : 'Name.com API Token'}
              />
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
