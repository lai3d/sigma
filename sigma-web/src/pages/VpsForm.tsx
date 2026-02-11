import { useEffect } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { useForm } from 'react-hook-form';
import { useVps, useCreateVps, useUpdateVps } from '@/hooks/useVps';
import { useProviders } from '@/hooks/useProviders';

interface FormData {
  hostname: string;
  alias: string;
  provider_id: string;
  ip_addresses_raw: string;
  ssh_port: number;
  country: string;
  city: string;
  dc_name: string;
  cpu_cores: string;
  ram_mb: string;
  disk_gb: string;
  bandwidth_tb: string;
  cost_monthly: string;
  currency: string;
  status: string;
  purchase_date: string;
  expire_date: string;
  purpose: string;
  vpn_protocol: string;
  tags_raw: string;
  monitoring_enabled: boolean;
  node_exporter_port: number;
  notes: string;
}

export default function VpsForm() {
  const { id } = useParams<{ id: string }>();
  const isEdit = !!id;
  const navigate = useNavigate();

  const { data: existing } = useVps(id || '');
  const { data: providers } = useProviders();
  const createMutation = useCreateVps();
  const updateMutation = useUpdateVps();

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<FormData>({
    defaultValues: {
      hostname: '',
      alias: '',
      provider_id: '',
      ip_addresses_raw: '',
      ssh_port: 22,
      country: '',
      city: '',
      dc_name: '',
      cpu_cores: '',
      ram_mb: '',
      disk_gb: '',
      bandwidth_tb: '',
      cost_monthly: '',
      currency: 'USD',
      status: 'provisioning',
      purchase_date: '',
      expire_date: '',
      purpose: '',
      vpn_protocol: '',
      tags_raw: '',
      monitoring_enabled: true,
      node_exporter_port: 9100,
      notes: '',
    },
  });

  useEffect(() => {
    if (existing) {
      reset({
        hostname: existing.hostname,
        alias: existing.alias,
        provider_id: existing.provider_id,
        ip_addresses_raw: existing.ip_addresses.map((ip) => ip.replace(/\/(32|128)$/, '')).join(', '),
        ssh_port: existing.ssh_port,
        country: existing.country,
        city: existing.city,
        dc_name: existing.dc_name,
        cpu_cores: existing.cpu_cores !== null ? String(existing.cpu_cores) : '',
        ram_mb: existing.ram_mb !== null ? String(existing.ram_mb) : '',
        disk_gb: existing.disk_gb !== null ? String(existing.disk_gb) : '',
        bandwidth_tb: existing.bandwidth_tb ? String(existing.bandwidth_tb) : '',
        cost_monthly: existing.cost_monthly ? String(existing.cost_monthly) : '',
        currency: existing.currency,
        status: existing.status,
        purchase_date: existing.purchase_date || '',
        expire_date: existing.expire_date || '',
        purpose: existing.purpose,
        vpn_protocol: existing.vpn_protocol,
        tags_raw: existing.tags.join(', '),
        monitoring_enabled: existing.monitoring_enabled,
        node_exporter_port: existing.node_exporter_port,
        notes: existing.notes,
      });
    }
  }, [existing, reset]);

  function toNum(val: string): number | null {
    if (!val) return null;
    const n = Number(val);
    return isNaN(n) ? null : n;
  }

  async function onSubmit(data: FormData) {
    if (!data.hostname.trim()) return;
    if (!data.provider_id) return;

    const ip_addresses = data.ip_addresses_raw
      ? data.ip_addresses_raw.split(/[,\s]+/).filter(Boolean)
      : [];
    const tags = data.tags_raw
      ? data.tags_raw.split(/[,\s]+/).filter(Boolean)
      : [];

    const payload = {
      hostname: data.hostname.trim(),
      alias: data.alias || '',
      provider_id: data.provider_id,
      ip_addresses,
      ssh_port: data.ssh_port || 22,
      country: data.country || '',
      city: data.city || '',
      dc_name: data.dc_name || '',
      cpu_cores: toNum(data.cpu_cores),
      ram_mb: toNum(data.ram_mb),
      disk_gb: toNum(data.disk_gb),
      bandwidth_tb: toNum(data.bandwidth_tb),
      cost_monthly: toNum(data.cost_monthly),
      currency: data.currency || 'USD',
      status: data.status || 'provisioning',
      purchase_date: data.purchase_date || null,
      expire_date: data.expire_date || null,
      purpose: data.purpose || '',
      vpn_protocol: data.vpn_protocol || '',
      tags,
      monitoring_enabled: data.monitoring_enabled ?? true,
      node_exporter_port: data.node_exporter_port || 9100,
      notes: data.notes || '',
    };

    if (isEdit && id) {
      await updateMutation.mutateAsync({ id, data: payload });
    } else {
      await createMutation.mutateAsync(payload);
    }
    navigate('/vps');
  }

  return (
    <div>
      <h2 className="text-2xl font-bold text-gray-900">
        {isEdit ? 'Edit VPS' : 'Add VPS'}
      </h2>

      <form onSubmit={handleSubmit(onSubmit)} className="mt-6 max-w-3xl space-y-8">
        {/* Basic Info */}
        <fieldset className="bg-white rounded-lg border p-5 space-y-4">
          <legend className="text-sm font-semibold text-gray-700 px-2">Basic Info</legend>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <Field label="Hostname *" error={errors.hostname?.message}>
              <input {...register('hostname')} className="input" placeholder="hk-exit-01" />
            </Field>
            <Field label="Alias">
              <input {...register('alias')} className="input" placeholder="Hong Kong Exit 1" />
            </Field>
            <Field label="Provider *" error={errors.provider_id?.message}>
              <select {...register('provider_id')} className="input">
                <option value="">Select provider...</option>
                {providers?.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Status">
              <select {...register('status')} className="input">
                <option value="provisioning">Provisioning</option>
                <option value="active">Active</option>
                <option value="retiring">Retiring</option>
                <option value="retired">Retired</option>
              </select>
            </Field>
          </div>
        </fieldset>

        {/* Network */}
        <fieldset className="bg-white rounded-lg border p-5 space-y-4">
          <legend className="text-sm font-semibold text-gray-700 px-2">Network</legend>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="md:col-span-2">
              <Field label="IP Addresses" hint="Comma-separated">
                <input
                  {...register('ip_addresses_raw')}
                  className="input"
                  placeholder="103.1.2.3, 2001:db8::1"
                />
              </Field>
            </div>
            <Field label="SSH Port">
              <input {...register('ssh_port')} type="number" className="input" />
            </Field>
          </div>
        </fieldset>

        {/* Location */}
        <fieldset className="bg-white rounded-lg border p-5 space-y-4">
          <legend className="text-sm font-semibold text-gray-700 px-2">Location</legend>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <Field label="Country">
              <input {...register('country')} className="input" placeholder="HK" />
            </Field>
            <Field label="City">
              <input {...register('city')} className="input" placeholder="Hong Kong" />
            </Field>
            <Field label="Data Center">
              <input {...register('dc_name')} className="input" placeholder="MEGA-i" />
            </Field>
          </div>
        </fieldset>

        {/* Specs */}
        <fieldset className="bg-white rounded-lg border p-5 space-y-4">
          <legend className="text-sm font-semibold text-gray-700 px-2">Specs</legend>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <Field label="CPU Cores">
              <input {...register('cpu_cores')} type="number" className="input" />
            </Field>
            <Field label="RAM (MB)">
              <input {...register('ram_mb')} type="number" className="input" />
            </Field>
            <Field label="Disk (GB)">
              <input {...register('disk_gb')} type="number" className="input" />
            </Field>
            <Field label="Bandwidth (TB)">
              <input {...register('bandwidth_tb')} type="number" step="0.1" className="input" />
            </Field>
          </div>
        </fieldset>

        {/* Cost & Dates */}
        <fieldset className="bg-white rounded-lg border p-5 space-y-4">
          <legend className="text-sm font-semibold text-gray-700 px-2">Cost & Dates</legend>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <Field label="Monthly Cost">
              <input {...register('cost_monthly')} type="number" step="0.01" className="input" />
            </Field>
            <Field label="Currency">
              <input {...register('currency')} className="input" />
            </Field>
            <Field label="Purchase Date">
              <input {...register('purchase_date')} type="date" className="input" />
            </Field>
            <Field label="Expire Date">
              <input {...register('expire_date')} type="date" className="input" />
            </Field>
          </div>
        </fieldset>

        {/* VPN Config */}
        <fieldset className="bg-white rounded-lg border p-5 space-y-4">
          <legend className="text-sm font-semibold text-gray-700 px-2">VPN & Monitoring</legend>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <Field label="Purpose">
              <select {...register('purpose')} className="input">
                <option value="">None</option>
                <option value="vpn-exit">VPN Exit</option>
                <option value="vpn-relay">VPN Relay</option>
                <option value="vpn-entry">VPN Entry</option>
                <option value="monitor">Monitor</option>
                <option value="management">Management</option>
              </select>
            </Field>
            <Field label="VPN Protocol">
              <input
                {...register('vpn_protocol')}
                className="input"
                placeholder="wireguard, xray, ..."
              />
            </Field>
            <div className="md:col-span-2">
              <Field label="Tags" hint="Comma-separated">
                <input
                  {...register('tags_raw')}
                  className="input"
                  placeholder="cn-optimized, iplc, cmhi"
                />
              </Field>
            </div>
            <label className="flex items-center gap-2 text-sm">
              <input type="checkbox" {...register('monitoring_enabled')} className="rounded" />
              Monitoring Enabled
            </label>
            <Field label="Node Exporter Port">
              <input {...register('node_exporter_port')} type="number" className="input" />
            </Field>
          </div>
        </fieldset>

        {/* Notes */}
        <fieldset className="bg-white rounded-lg border p-5 space-y-4">
          <legend className="text-sm font-semibold text-gray-700 px-2">Notes</legend>
          <textarea
            {...register('notes')}
            rows={3}
            className="input w-full"
            placeholder="Any additional notes..."
          />
        </fieldset>

        {/* Actions */}
        <div className="flex gap-3">
          <button
            type="submit"
            disabled={isSubmitting}
            className="px-6 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50"
          >
            {isSubmitting ? 'Saving...' : isEdit ? 'Update' : 'Create'}
          </button>
          <button
            type="button"
            onClick={() => navigate('/vps')}
            className="px-6 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-md hover:bg-gray-50"
          >
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}

function Field({
  label,
  hint,
  error,
  children,
}: {
  label: string;
  hint?: string;
  error?: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <label className="block text-sm font-medium text-gray-700">
        {label}
        {hint && <span className="text-gray-400 font-normal ml-1">({hint})</span>}
      </label>
      <div className="mt-1">{children}</div>
      {error && <p className="mt-1 text-xs text-red-600">{error}</p>}
    </div>
  );
}
