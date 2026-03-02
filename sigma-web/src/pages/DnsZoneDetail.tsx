import { Fragment, useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { ArrowLeft, RefreshCw, Cloud, ArrowRight, History, ChevronDown, ChevronRight } from 'lucide-react';
import { useDnsZone, useDnsRecords, useSyncDnsZone, useDnsAccounts, useDnsRecordHistory } from '@/hooks/useDns';
import Pagination from '@/components/Pagination';
import type { DnsSyncResult } from '@/types/api';

const RECORD_TYPE_COLORS: Record<string, string> = {
  A: 'bg-blue-100 text-blue-800',
  AAAA: 'bg-indigo-100 text-indigo-800',
  CNAME: 'bg-purple-100 text-purple-800',
  MX: 'bg-orange-100 text-orange-800',
  TXT: 'bg-gray-100 text-gray-700',
  NS: 'bg-green-100 text-green-800',
  SRV: 'bg-cyan-100 text-cyan-800',
};

const PROVIDER_LABELS: Record<string, string> = {
  cloudflare: 'Cloudflare',
  route53: 'Route 53',
  godaddy: 'GoDaddy',
  namecom: 'Name.com',
};

function ExpiryCell({ label, date }: { label: string; date: string | null }) {
  if (!date) {
    return (
      <div>
        <span className="text-xs text-gray-500">{label}</span>
        <div className="text-gray-400">-</div>
      </div>
    );
  }
  const now = new Date();
  const exp = new Date(date);
  const days = Math.ceil((exp.getTime() - now.getTime()) / (1000 * 60 * 60 * 24));

  let colorClass = 'text-green-700 bg-green-50';
  if (days < 7) colorClass = 'text-red-700 bg-red-50';
  else if (days < 30) colorClass = 'text-yellow-700 bg-yellow-50';

  return (
    <div>
      <span className="text-xs text-gray-500">{label}</span>
      <div>
        <span className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${colorClass}`}>
          {exp.toLocaleDateString()} ({days}d)
        </span>
      </div>
    </div>
  );
}

const ACTION_BADGE: Record<string, string> = {
  created: 'bg-green-100 text-green-800',
  updated: 'bg-yellow-100 text-yellow-800',
  deleted: 'bg-red-100 text-red-800',
};

function RecordHistory({ recordId }: { recordId: string }) {
  const [page, setPage] = useState(1);
  const { data, isLoading } = useDnsRecordHistory(recordId, { page, per_page: 10 });

  if (isLoading) return <div className="px-4 py-3 text-xs text-gray-500">Loading history...</div>;
  if (!data?.data.length) return <div className="px-4 py-3 text-xs text-gray-400">No history yet</div>;

  return (
    <div className="px-4 py-3 space-y-2">
      {data.data.map((h) => (
        <div key={h.id} className="flex items-start gap-3 text-xs">
          <span className={`inline-flex items-center px-1.5 py-0.5 rounded font-medium shrink-0 ${ACTION_BADGE[h.action] || 'bg-gray-100 text-gray-600'}`}>
            {h.action}
          </span>
          <div className="flex-1 min-w-0">
            {h.action === 'updated' && h.old_content !== h.new_content && (
              <div className="font-mono">
                <span className="text-red-600 line-through">{h.old_content}</span>
                <span className="text-gray-400 mx-1">&rarr;</span>
                <span className="text-green-700">{h.new_content}</span>
              </div>
            )}
            {h.action === 'updated' && h.old_content === h.new_content && (
              <div className="text-gray-500 italic">extra metadata changed</div>
            )}
            {h.action === 'created' && (
              <span className="font-mono text-green-700">{h.new_content}</span>
            )}
            {h.action === 'deleted' && (
              <span className="font-mono text-red-600 line-through">{h.old_content}</span>
            )}
            {h.actor_email && (
              <span className="text-gray-400">
                by {h.actor_email}{h.actor_ip ? ` (${h.actor_ip})` : ''}
              </span>
            )}
          </div>
          <span className="text-gray-400 shrink-0">{new Date(h.created_at).toLocaleString()}</span>
        </div>
      ))}
      {data.total > 10 && (
        <div className="flex items-center gap-2 pt-1">
          <button
            disabled={page <= 1}
            onClick={() => setPage((p) => p - 1)}
            className="text-xs text-blue-600 hover:text-blue-800 disabled:text-gray-300"
          >
            Prev
          </button>
          <span className="text-xs text-gray-400">{page} / {Math.ceil(data.total / 10)}</span>
          <button
            disabled={page * 10 >= data.total}
            onClick={() => setPage((p) => p + 1)}
            className="text-xs text-blue-600 hover:text-blue-800 disabled:text-gray-300"
          >
            Next
          </button>
        </div>
      )}
    </div>
  );
}

export default function DnsZoneDetail() {
  const { id } = useParams<{ id: string }>();
  const { data: zone, isLoading } = useDnsZone(id || '');
  const { data: accounts } = useDnsAccounts({ per_page: 100 });

  const [recordPage, setRecordPage] = useState(1);
  const [typeFilter, setTypeFilter] = useState('');
  const [expandedRecord, setExpandedRecord] = useState<string | null>(null);
  const { data: recordsResult, isLoading: recordsLoading } = useDnsRecords({
    zone_name: zone?.zone_name,
    record_type: typeFilter || undefined,
    page: recordPage,
    per_page: 25,
  });

  const syncMutation = useSyncDnsZone();
  const [syncing, setSyncing] = useState(false);
  const [syncResult, setSyncResult] = useState<DnsSyncResult | null>(null);

  async function handleSync() {
    if (!id) return;
    setSyncing(true);
    setSyncResult(null);
    try {
      const result = await syncMutation.mutateAsync(id);
      setSyncResult(result);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Sync failed';
      alert(msg);
    } finally {
      setSyncing(false);
    }
  }

  if (isLoading) return <div className="p-8 text-center text-gray-500">Loading...</div>;
  if (!zone) return <div className="p-8 text-center text-gray-400">Zone not found</div>;

  const account = accounts?.data.find((a) => a.id === zone.account_id);

  return (
    <div>
      {/* Header */}
      <div className="flex items-center gap-3 mb-6">
        <Link to="/domains" className="text-gray-400 hover:text-gray-600">
          <ArrowLeft size={20} />
        </Link>
        <div className="flex-1">
          <div className="flex items-center gap-3">
            <h2 className="text-2xl font-bold text-gray-900">{zone.zone_name}</h2>
            <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${
              zone.status === 'active'
                ? 'bg-green-100 text-green-800'
                : 'bg-gray-100 text-gray-600'
            }`}>
              {zone.status}
            </span>
          </div>
        </div>
        <button
          onClick={handleSync}
          disabled={syncing}
          className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50"
        >
          <RefreshCw size={16} className={syncing ? 'animate-spin' : ''} />
          Sync
        </button>
      </div>

      {syncResult && (
        <div className="mb-4 p-3 bg-green-50 border border-green-200 rounded-md text-sm text-green-800">
          Sync complete: {syncResult.records_count} records, {syncResult.records_linked} linked to VPS, {syncResult.records_deleted} deleted
          <button onClick={() => setSyncResult(null)} className="ml-3 text-green-600 hover:text-green-800 font-medium">
            Dismiss
          </button>
        </div>
      )}

      {/* Info panels */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
        <div className="bg-white rounded-lg border p-4">
          <h3 className="text-sm font-medium text-gray-500 mb-3">Zone Info</h3>
          <dl className="space-y-2 text-sm">
            <div className="flex justify-between">
              <dt className="text-gray-500">Provider</dt>
              <dd className="font-medium text-gray-900">
                {account ? PROVIDER_LABELS[account.provider_type] || account.provider_type : '-'}
              </dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-gray-500">Account</dt>
              <dd className="font-medium text-gray-900">{account?.name || '-'}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-gray-500">Zone ID</dt>
              <dd className="font-mono text-xs text-gray-600">{zone.zone_id}</dd>
            </div>
          </dl>
        </div>

        <div className="bg-white rounded-lg border p-4">
          <h3 className="text-sm font-medium text-gray-500 mb-3">Expiry & Dates</h3>
          <div className="grid grid-cols-2 gap-3">
            <ExpiryCell label="Domain Expires" date={zone.domain_expires_at} />
            <ExpiryCell label="Cert Expires" date={zone.cert_expires_at} />
            <div>
              <span className="text-xs text-gray-500">Last Synced</span>
              <div className="text-sm text-gray-900">{new Date(zone.synced_at).toLocaleString()}</div>
            </div>
            <div>
              <span className="text-xs text-gray-500">Created</span>
              <div className="text-sm text-gray-900">{new Date(zone.created_at).toLocaleString()}</div>
            </div>
          </div>
        </div>
      </div>

      {/* Records */}
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-lg font-semibold text-gray-900">
          DNS Records
          {recordsResult && <span className="text-sm font-normal text-gray-500 ml-2">({recordsResult.total})</span>}
        </h3>
        <select
          value={typeFilter}
          onChange={(e) => { setTypeFilter(e.target.value); setRecordPage(1); }}
          className="border rounded-md px-3 py-1.5 text-sm bg-white"
        >
          <option value="">All Types</option>
          {['A', 'AAAA', 'CNAME', 'MX', 'TXT', 'NS', 'SRV'].map((t) => (
            <option key={t} value={t}>{t}</option>
          ))}
        </select>
      </div>

      <div className="bg-white rounded-lg border overflow-x-auto">
        {recordsLoading ? (
          <div className="p-8 text-center text-gray-500">Loading...</div>
        ) : !recordsResult?.data.length ? (
          <div className="p-8 text-center text-gray-400">No DNS records found</div>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-gray-500 border-b bg-gray-50">
                <th className="px-4 py-3 font-medium w-8"></th>
                <th className="px-4 py-3 font-medium">Name</th>
                <th className="px-4 py-3 font-medium">Type</th>
                <th className="px-4 py-3 font-medium">Content</th>
                <th className="px-4 py-3 font-medium">TTL</th>
                <th className="px-4 py-3 font-medium">Proxied</th>
                <th className="px-4 py-3 font-medium">VPS</th>
              </tr>
            </thead>
            <tbody>
              {recordsResult.data.map((rec) => {
                const proxied = rec.extra?.proxied === true;
                const isExpanded = expandedRecord === rec.id;
                return (
                  <Fragment key={rec.id}>
                    <tr className="border-b last:border-0 hover:bg-gray-50">
                      <td className="px-2 py-3 text-center">
                        <button
                          onClick={() => setExpandedRecord(isExpanded ? null : rec.id)}
                          className="text-gray-400 hover:text-gray-600 p-0.5"
                          title="Toggle history"
                        >
                          {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                        </button>
                      </td>
                      <td className="px-4 py-3 font-medium text-gray-900 max-w-xs truncate">
                        {rec.name}
                      </td>
                      <td className="px-4 py-3">
                        <span className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${
                          RECORD_TYPE_COLORS[rec.record_type] || 'bg-gray-100 text-gray-600'
                        }`}>
                          {rec.record_type}
                        </span>
                      </td>
                      <td className="px-4 py-3 text-gray-600 font-mono text-xs max-w-xs truncate">
                        {rec.content}
                      </td>
                      <td className="px-4 py-3 text-gray-500">
                        {rec.ttl === 1 ? 'Auto' : rec.ttl}
                      </td>
                      <td className="px-4 py-3">
                        {proxied ? (
                          <Cloud size={16} className="text-orange-500" />
                        ) : (
                          <span className="text-gray-400 text-xs">-</span>
                        )}
                      </td>
                      <td className="px-4 py-3">
                        {rec.vps_id ? (
                          <Link
                            to={`/vps/${rec.vps_id}`}
                            className="inline-flex items-center gap-1 text-blue-600 hover:text-blue-800 text-xs"
                          >
                            {rec.vps_hostname || 'VPS'}
                            {rec.vps_country && (
                              <span className="text-gray-400">({rec.vps_country})</span>
                            )}
                            <ArrowRight size={12} />
                          </Link>
                        ) : (
                          <span className="text-gray-300">&mdash;</span>
                        )}
                      </td>
                    </tr>
                    {isExpanded && (
                      <tr className="border-b last:border-0">
                        <td colSpan={7} className="bg-gray-50/50">
                          <div className="flex items-center gap-1.5 px-4 pt-2 text-xs font-medium text-gray-500">
                            <History size={12} />
                            Change History
                          </div>
                          <RecordHistory recordId={rec.id} />
                        </td>
                      </tr>
                    )}
                  </Fragment>
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      {recordsResult && (
        <Pagination
          page={recordsResult.page}
          perPage={recordsResult.per_page}
          total={recordsResult.total}
          onPageChange={setRecordPage}
        />
      )}
    </div>
  );
}
