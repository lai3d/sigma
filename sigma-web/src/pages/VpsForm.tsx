import { useEffect, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { useForm } from 'react-hook-form';
import { BarChart3 } from 'lucide-react';
import { useVps, useCreateVps, useUpdateVps } from '@/hooks/useVps';
import { useProviders } from '@/hooks/useProviders';
import { useVpsPurposes } from '@/hooks/useVpsPurposes';
import { ipLabelColor, timeAgo, formatUptime } from '@/lib/utils';
import type { IpEntry } from '@/types/api';
import { COUNTRIES } from '@/lib/countries';
import { PROJECTS } from '@/lib/projects';

const IP_LABELS = [
  { value: '', display: '-' },
  { value: 'china-telecom', display: '电信' },
  { value: 'china-unicom', display: '联通' },
  { value: 'china-mobile', display: '移动' },
  { value: 'china-cernet', display: '教育网' },
  { value: 'overseas', display: '海外' },
  { value: 'internal', display: '内网' },
  { value: 'anycast', display: 'Anycast' },
] as const;

interface FormData {
  hostname: string;
  alias: string;
  provider_id: string;
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
  const { data: providersResult } = useProviders({ per_page: 100 });
  const providers = providersResult?.data;
  const { data: purposesResult } = useVpsPurposes({ per_page: 100 });
  const purposes = purposesResult?.data;
  const createMutation = useCreateVps();
  const updateMutation = useUpdateVps();

  // Dynamic IP list with labels
  const [ipList, setIpList] = useState<IpEntry[]>([{ ip: '', label: '' }]);
  const [ipError, setIpError] = useState('');

  // Project selection (stored as p:xxx tags)
  const [selectedProjects, setSelectedProjects] = useState<Set<string>>(new Set());

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
      const ips =
        existing.ip_addresses.length > 0
          ? existing.ip_addresses
          : [{ ip: '', label: '' }];
      setIpList(ips.map((e) => ({ ...e })));
      // Split project tags (p:xxx) from regular tags
      const projectTags = new Set(
        existing.tags.filter((t) => t.startsWith('p:')).map((t) => t.slice(2)),
      );
      const regularTags = existing.tags.filter((t) => !t.startsWith('p:'));
      setSelectedProjects(projectTags);
      reset({
        hostname: existing.hostname,
        alias: existing.alias,
        provider_id: existing.provider_id,
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
        tags_raw: regularTags.join(', '),
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

  function updateIp(index: number, field: keyof IpEntry, value: string) {
    setIpList((prev) =>
      prev.map((entry, i) => (i === index ? { ...entry, [field]: value } : entry)),
    );
    setIpError('');
  }

  function addIp() {
    setIpList((prev) => [...prev, { ip: '', label: '' }]);
  }

  function removeIp(index: number) {
    setIpList((prev) => {
      const next = prev.filter((_, i) => i !== index);
      return next.length === 0 ? [{ ip: '', label: '' }] : next;
    });
  }

  function pasteIps(e: React.ClipboardEvent<HTMLInputElement>, index: number) {
    const text = e.clipboardData.getData('text');
    const parts = text
      .split(/[,\n\r]+/)
      .map((s) => s.trim())
      .filter(Boolean);
    if (parts.length > 1) {
      e.preventDefault();
      setIpList((prev) => {
        const next = [...prev];
        const newEntries: IpEntry[] = parts.map((ip) => ({ ip, label: '' }));
        next.splice(index, 1, ...newEntries);
        return next;
      });
    }
  }

  async function onSubmit(data: FormData) {
    if (!data.hostname.trim()) return;

    const ip_addresses = ipList
      .filter((e) => e.ip.trim())
      .map((e) => ({ ip: e.ip.trim(), label: e.label }));

    // Validate IP format
    const ipRegex = /^[\d.:a-fA-F]+$/;
    const badEntry = ip_addresses.find((e) => !ipRegex.test(e.ip));
    if (badEntry) {
      setIpError(`Invalid IP address: ${badEntry.ip}`);
      return;
    }

    const regularTags = data.tags_raw
      ? data.tags_raw.split(/[,\s]+/).filter(Boolean)
      : [];
    const projectTags = [...selectedProjects].map((p) => `p:${p}`);
    const tags = [...regularTags, ...projectTags];

    const payload = {
      hostname: data.hostname.trim(),
      alias: data.alias || '',
      provider_id: data.provider_id || undefined,
      ip_addresses,
      ssh_port: Number(data.ssh_port) || 22,
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
      node_exporter_port: Number(data.node_exporter_port) || 9100,
      notes: data.notes || '',
    };

    try {
      if (isEdit && id) {
        await updateMutation.mutateAsync({ id, data: payload });
      } else {
        await createMutation.mutateAsync(payload);
      }
      navigate(isEdit && id ? `/vps/${id}` : '/vps');
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Failed to save VPS';
      alert(msg);
    }
  }

  const grafanaBaseUrl = localStorage.getItem('sigma_grafana_url') || '';
  const firstIp = existing?.ip_addresses?.[0]?.ip;
  const grafanaLink = grafanaBaseUrl && firstIp
    ? `${grafanaBaseUrl}${grafanaBaseUrl.includes('?') ? '&' : '?'}var-target=${encodeURIComponent(firstIp)}`
    : '';

  return (
    <div>
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold text-gray-900">
          {isEdit ? 'Edit VPS' : 'Add VPS'}
        </h2>
        {isEdit && grafanaLink && (
          <a
            href={grafanaLink}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium text-orange-700 bg-orange-50 border border-orange-200 rounded-md hover:bg-orange-100"
          >
            <BarChart3 size={14} /> Grafana
          </a>
        )}
      </div>

      <form onSubmit={handleSubmit(onSubmit)} className="mt-6 max-w-3xl space-y-8">
        {/* Agent Info (read-only, edit mode only) */}
        {isEdit && existing?.extra?.last_heartbeat ? (() => {
          const hb = existing.extra.last_heartbeat as string;
          const online = Date.now() - new Date(hb).getTime() < 3 * 60 * 1000;
          const si = existing.extra.system_info as { cpu_cores?: number; ram_mb?: number; disk_gb?: number; disk_used_gb?: number; uptime_seconds?: number; load_avg?: number[] } | undefined;
          return (
            <fieldset className="bg-gray-50 rounded-lg border p-5 space-y-3">
              <legend className="text-sm font-semibold text-gray-700 px-2">Agent Info</legend>
              <div className="grid grid-cols-2 md:grid-cols-3 gap-x-6 gap-y-2 text-sm">
                <div>
                  <span className="text-gray-500">Last Heartbeat</span>
                  <div className="flex items-center gap-1.5 mt-0.5 font-medium">
                    <span className={`inline-block w-2 h-2 rounded-full ${online ? 'bg-green-500' : 'bg-red-500'}`} />
                    {timeAgo(hb)}
                  </div>
                </div>
                {si?.cpu_cores != null && (
                  <div>
                    <span className="text-gray-500">CPU Cores</span>
                    <div className="mt-0.5 font-medium">{si.cpu_cores}</div>
                  </div>
                )}
                {si?.ram_mb != null && (
                  <div>
                    <span className="text-gray-500">RAM</span>
                    <div className="mt-0.5 font-medium">{si.ram_mb >= 1024 ? `${(si.ram_mb / 1024).toFixed(1)} GB` : `${si.ram_mb} MB`}</div>
                  </div>
                )}
                {si?.disk_gb != null && (
                  <div>
                    <span className="text-gray-500">Disk (Used / Total)</span>
                    <div className="mt-0.5 font-medium">{si.disk_used_gb != null ? `${si.disk_used_gb} / ${si.disk_gb} GB` : `${si.disk_gb} GB`}</div>
                  </div>
                )}
                {si?.uptime_seconds != null && (
                  <div>
                    <span className="text-gray-500">Uptime</span>
                    <div className="mt-0.5 font-medium">{formatUptime(si.uptime_seconds)}</div>
                  </div>
                )}
                {si?.load_avg && (
                  <div>
                    <span className="text-gray-500">Load Average</span>
                    <div className="mt-0.5 font-medium">{si.load_avg.map(v => v.toFixed(2)).join(' / ')}</div>
                  </div>
                )}
              </div>
            </fieldset>
          );
        })() : null}

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
            <Field label="Provider" error={errors.provider_id?.message}>
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
          <div className="space-y-3">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">
                IP Addresses
                <span className="text-gray-400 font-normal ml-1">(paste multiple to auto-split)</span>
              </label>
              <div className="space-y-2">
                {ipList.map((entry, i) => (
                  <div key={i} className="flex gap-2 items-center">
                    <input
                      value={entry.ip}
                      onChange={(e) => updateIp(i, 'ip', e.target.value)}
                      onPaste={(e) => pasteIps(e, i)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') {
                          e.preventDefault();
                          addIp();
                        }
                      }}
                      className="input flex-1 min-w-0"
                      placeholder="103.1.2.3"
                    />
                    <select
                      value={entry.label}
                      onChange={(e) => updateIp(i, 'label', e.target.value)}
                      className={`shrink-0 rounded-md border px-2 py-1.5 text-sm font-medium outline-none cursor-pointer ${
                        entry.label ? ipLabelColor(entry.label) + ' border-transparent' : 'border-gray-300 text-gray-400'
                      }`}
                    >
                      {IP_LABELS.map((l) => (
                        <option key={l.value} value={l.value}>
                          {l.display}
                        </option>
                      ))}
                    </select>
                    <button
                      type="button"
                      onClick={() => removeIp(i)}
                      className="p-1.5 text-gray-400 hover:text-red-500 shrink-0"
                      title="Remove"
                    >
                      <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                      </svg>
                    </button>
                  </div>
                ))}
              </div>
              {ipError && <p className="mt-1 text-xs text-red-600">{ipError}</p>}
              <button
                type="button"
                onClick={addIp}
                className="mt-2 inline-flex items-center gap-1 text-sm text-blue-600 hover:text-blue-700"
              >
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
                </svg>
                Add IP
              </button>
            </div>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <Field label="SSH Port">
                <input {...register('ssh_port')} type="number" className="input" />
              </Field>
            </div>
          </div>
        </fieldset>

        {/* Location */}
        <fieldset className="bg-white rounded-lg border p-5 space-y-4">
          <legend className="text-sm font-semibold text-gray-700 px-2">Location</legend>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <Field label="Country">
              <select {...register('country')} className="input">
                <option value="">Select country...</option>
                {COUNTRIES.map((c) => (
                  <option key={c.code} value={c.code}>
                    {c.code} - {c.name}
                  </option>
                ))}
              </select>
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
                {purposes?.map((p) => (
                  <option key={p.id} value={p.name}>{p.label}</option>
                ))}
              </select>
            </Field>
            <Field label="VPN Protocol">
              <input
                {...register('vpn_protocol')}
                className="input"
                placeholder="e.g. wireguard, nginx, ..."
              />
            </Field>
            <div className="md:col-span-2">
              <label className="block text-sm font-medium text-gray-700 mb-1.5">Projects</label>
              <div className="flex flex-wrap gap-2">
                {PROJECTS.map((proj) => {
                  const checked = selectedProjects.has(proj.id);
                  return (
                    <button
                      key={proj.id}
                      type="button"
                      onClick={() => {
                        setSelectedProjects((prev) => {
                          const next = new Set(prev);
                          if (next.has(proj.id)) next.delete(proj.id);
                          else next.add(proj.id);
                          return next;
                        });
                      }}
                      className={`px-3 py-1 text-sm rounded-full border transition-colors ${
                        checked
                          ? 'bg-purple-50 text-purple-700 border-purple-300'
                          : 'bg-white text-gray-500 border-gray-300 hover:border-gray-400'
                      }`}
                    >
                      {proj.name}
                    </button>
                  );
                })}
              </div>
            </div>
            <div className="md:col-span-2">
              <Field label="Tags" hint="Comma-separated">
                <input
                  {...register('tags_raw')}
                  className="input"
                  placeholder="optimized, premium, gpu"
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
            onClick={() => navigate(isEdit && id ? `/vps/${id}` : '/vps')}
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
