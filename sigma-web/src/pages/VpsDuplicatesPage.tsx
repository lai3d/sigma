import { useState } from 'react';
import { CheckCircle2, Loader2 } from 'lucide-react';
import { useVpsDuplicates, useMergeVps } from '@/hooks/useVpsDuplicates';
import ConfirmDialog from '@/components/ConfirmDialog';
import type { DuplicateGroup, Vps } from '@/types/api';

function IpList({ ips }: { ips: { ip: string; label: string }[] }) {
  return (
    <div className="flex flex-wrap gap-1">
      {ips.map((e) => (
        <span
          key={e.ip}
          className="inline-flex items-center px-1.5 py-0.5 text-xs rounded bg-gray-100 text-gray-700 font-mono"
        >
          {e.ip}
          {e.label && (
            <span className="ml-1 text-gray-400">{e.label}</span>
          )}
        </span>
      ))}
    </div>
  );
}

function TagList({ tags }: { tags: string[] }) {
  if (!tags.length) return <span className="text-gray-400">—</span>;
  return (
    <div className="flex flex-wrap gap-1">
      {tags.map((t) => (
        <span key={t} className="px-1.5 py-0.5 text-xs rounded bg-blue-50 text-blue-700">
          {t}
        </span>
      ))}
    </div>
  );
}

const COMPARE_FIELDS: { key: keyof Vps; label: string; render?: (v: Vps) => React.ReactNode }[] = [
  { key: 'hostname', label: 'Hostname' },
  { key: 'source', label: 'Source' },
  { key: 'status', label: 'Status' },
  { key: 'country', label: 'Country' },
  {
    key: 'ip_addresses',
    label: 'IPs',
    render: (v) => <IpList ips={v.ip_addresses} />,
  },
  { key: 'provider_id', label: 'Provider ID', render: (v) => v.provider_id || <span className="text-gray-400">—</span> },
  { key: 'purpose', label: 'Purpose', render: (v) => v.purpose || <span className="text-gray-400">—</span> },
  { key: 'cloud_account_id', label: 'Cloud Account', render: (v) => v.cloud_account_id || <span className="text-gray-400">—</span> },
  {
    key: 'tags',
    label: 'Tags',
    render: (v) => <TagList tags={v.tags} />,
  },
];

function GroupCard({
  group,
  onMerge,
  merging,
}: {
  group: DuplicateGroup;
  onMerge: (targetId: string, sourceId: string) => void;
  merging: boolean;
}) {
  return (
    <div className="border border-amber-200 rounded-lg bg-white overflow-hidden">
      <div className="bg-amber-50 px-4 py-2 border-b border-amber-200 flex items-center justify-between">
        <span className="text-sm font-medium text-amber-800">
          Shared IPs: {group.shared_ips.join(', ')}
        </span>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b bg-gray-50">
              <th className="text-left px-4 py-2 font-medium text-gray-600 w-32">Field</th>
              <th className="text-left px-4 py-2 font-medium text-gray-600">VPS A</th>
              <th className="text-left px-4 py-2 font-medium text-gray-600">VPS B</th>
            </tr>
          </thead>
          <tbody>
            {COMPARE_FIELDS.map(({ key, label, render }) => (
              <tr key={key} className="border-b last:border-0">
                <td className="px-4 py-2 font-medium text-gray-500 whitespace-nowrap">{label}</td>
                <td className="px-4 py-2 break-all">
                  {render ? render(group.vps_a) : String(group.vps_a[key] ?? '—')}
                </td>
                <td className="px-4 py-2 break-all">
                  {render ? render(group.vps_b) : String(group.vps_b[key] ?? '—')}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="px-4 py-3 bg-gray-50 border-t flex gap-3">
        <button
          disabled={merging}
          onClick={() => onMerge(group.vps_a.id, group.vps_b.id)}
          className="px-3 py-1.5 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50"
        >
          Keep A
        </button>
        <button
          disabled={merging}
          onClick={() => onMerge(group.vps_b.id, group.vps_a.id)}
          className="px-3 py-1.5 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50"
        >
          Keep B
        </button>
      </div>
    </div>
  );
}

export default function VpsDuplicatesPage() {
  const { data, isLoading } = useVpsDuplicates();
  const mergeMutation = useMergeVps();
  const [confirmMerge, setConfirmMerge] = useState<{
    targetId: string;
    sourceId: string;
  } | null>(null);

  function handleMerge(targetId: string, sourceId: string) {
    setConfirmMerge({ targetId, sourceId });
  }

  async function executeMerge() {
    if (!confirmMerge) return;
    try {
      await mergeMutation.mutateAsync({
        target_id: confirmMerge.targetId,
        source_id: confirmMerge.sourceId,
      });
    } finally {
      setConfirmMerge(null);
    }
  }

  const groups = data?.groups ?? [];

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-gray-900">VPS Duplicates</h1>
          <p className="text-sm text-gray-500 mt-1">
            Detect and merge duplicate VPS records sharing public IPs
          </p>
        </div>
        {groups.length > 0 && (
          <span className="inline-flex items-center px-2.5 py-1 rounded-full text-sm font-medium bg-amber-100 text-amber-800">
            {groups.length} group{groups.length !== 1 ? 's' : ''}
          </span>
        )}
      </div>

      {isLoading && (
        <div className="flex items-center justify-center py-20">
          <Loader2 className="animate-spin text-gray-400" size={32} />
        </div>
      )}

      {!isLoading && groups.length === 0 && (
        <div className="flex flex-col items-center justify-center py-20 text-center">
          <CheckCircle2 className="text-green-500 mb-3" size={48} />
          <h2 className="text-lg font-semibold text-gray-700">No duplicates found</h2>
          <p className="text-sm text-gray-500 mt-1">
            All VPS records have unique public IPs.
          </p>
        </div>
      )}

      {!isLoading && groups.length > 0 && (
        <div className="space-y-6">
          {groups.map((group) => (
            <GroupCard
              key={`${group.vps_a.id}-${group.vps_b.id}`}
              group={group}
              onMerge={handleMerge}
              merging={mergeMutation.isPending}
            />
          ))}
        </div>
      )}

      <ConfirmDialog
        open={!!confirmMerge}
        title="Merge VPS Records"
        message="The source VPS will be deleted and its data (IPs, tags, linked records) will be merged into the target. This cannot be undone."
        confirmLabel="Merge"
        variant="danger"
        onConfirm={executeMerge}
        onCancel={() => setConfirmMerge(null)}
      />
    </div>
  );
}
