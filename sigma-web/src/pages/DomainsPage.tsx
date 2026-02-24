import { useState } from 'react';
import { Link } from 'react-router-dom';
import { Plus, RefreshCw, Pencil, Trash2, Cloud, ArrowRight } from 'lucide-react';
import {
  useDnsAccounts,
  useDeleteDnsAccount,
  useSyncDnsAccount,
  useDnsZones,
  useDnsRecords,
} from '@/hooks/useDns';
import Pagination from '@/components/Pagination';
import ConfirmDialog from '@/components/ConfirmDialog';
import DnsAccountDialog from './DnsAccountDialog';
import type { DnsSyncResult, DnsProviderType } from '@/types/api';

type Tab = 'accounts' | 'zones' | 'records';

const PROVIDER_LABELS: Record<DnsProviderType, string> = {
  cloudflare: 'Cloudflare',
  route53: 'Route 53',
  godaddy: 'GoDaddy',
  namecom: 'Name.com',
};

const PROVIDER_COLORS: Record<DnsProviderType, string> = {
  cloudflare: 'bg-orange-100 text-orange-800',
  route53: 'bg-yellow-100 text-yellow-800',
  godaddy: 'bg-green-100 text-green-800',
  namecom: 'bg-blue-100 text-blue-800',
};

export default function DomainsPage() {
  const [tab, setTab] = useState<Tab>('accounts');

  return (
    <div>
      <h2 className="text-2xl font-bold text-gray-900">Domains</h2>

      <div className="mt-4 flex gap-1 bg-gray-100 rounded-lg p-1 w-fit">
        {(['accounts', 'zones', 'records'] as Tab[]).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`px-4 py-1.5 text-sm font-medium rounded-md transition-colors ${
              tab === t
                ? 'bg-white text-gray-900 shadow-sm'
                : 'text-gray-500 hover:text-gray-700'
            }`}
          >
            {t === 'accounts' ? 'Accounts' : t === 'zones' ? 'Zones' : 'Records'}
          </button>
        ))}
      </div>

      {tab === 'accounts' && <AccountsTab />}
      {tab === 'zones' && <ZonesTab />}
      {tab === 'records' && <RecordsTab />}
    </div>
  );
}

// ─── Accounts Tab ─────────────────────────────────────────

function AccountsTab() {
  const [page, setPage] = useState(1);
  const { data: result, isLoading } = useDnsAccounts({ page, per_page: 25 });
  const deleteMutation = useDeleteDnsAccount();
  const syncMutation = useSyncDnsAccount();

  const [showCreate, setShowCreate] = useState(false);
  const [editAccount, setEditAccount] = useState<{ id: string; name: string; provider_type: DnsProviderType } | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [syncingId, setSyncingId] = useState<string | null>(null);
  const [syncResult, setSyncResult] = useState<DnsSyncResult | null>(null);

  async function handleSync(id: string) {
    setSyncingId(id);
    setSyncResult(null);
    try {
      const result = await syncMutation.mutateAsync(id);
      setSyncResult(result);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Sync failed';
      alert(msg);
    } finally {
      setSyncingId(null);
    }
  }

  function maskedConfigSummary(masked: Record<string, string>): string {
    // Show first masked credential value as summary
    const values = Object.values(masked).filter((v) => v !== '****');
    return values[0] || '****';
  }

  return (
    <>
      <div className="mt-4 flex items-center justify-between">
        <div />
        <button
          onClick={() => setShowCreate(true)}
          className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700"
        >
          <Plus size={16} /> Add Account
        </button>
      </div>

      {syncResult && (
        <div className="mt-3 p-3 bg-green-50 border border-green-200 rounded-md text-sm text-green-800">
          Sync complete: {syncResult.zones_count} zones, {syncResult.records_count} records,{' '}
          {syncResult.records_linked} linked to VPS, {syncResult.records_deleted} deleted
          <button onClick={() => setSyncResult(null)} className="ml-3 text-green-600 hover:text-green-800 font-medium">
            Dismiss
          </button>
        </div>
      )}

      <div className="mt-4 bg-white rounded-lg border overflow-x-auto">
        {isLoading ? (
          <div className="p-8 text-center text-gray-500">Loading...</div>
        ) : !result?.data.length ? (
          <div className="p-8 text-center text-gray-400">No DNS accounts configured</div>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-gray-500 border-b bg-gray-50">
                <th className="px-4 py-3 font-medium">Name</th>
                <th className="px-4 py-3 font-medium">Type</th>
                <th className="px-4 py-3 font-medium">Credentials</th>
                <th className="px-4 py-3 font-medium text-right">Zones</th>
                <th className="px-4 py-3 font-medium text-right">Records</th>
                <th className="px-4 py-3 font-medium">Last Synced</th>
                <th className="px-4 py-3 font-medium w-36">Actions</th>
              </tr>
            </thead>
            <tbody>
              {result.data.map((acc) => (
                <tr key={acc.id} className="border-b last:border-0 hover:bg-gray-50">
                  <td className="px-4 py-3 font-medium text-gray-900">{acc.name}</td>
                  <td className="px-4 py-3">
                    <span className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${
                      PROVIDER_COLORS[acc.provider_type] || 'bg-gray-100 text-gray-600'
                    }`}>
                      {PROVIDER_LABELS[acc.provider_type] || acc.provider_type}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-gray-500 font-mono text-xs">
                    {maskedConfigSummary(acc.masked_config)}
                  </td>
                  <td className="px-4 py-3 text-right">{acc.zones_count}</td>
                  <td className="px-4 py-3 text-right">{acc.records_count}</td>
                  <td className="px-4 py-3 text-gray-500">
                    {acc.last_synced
                      ? new Date(acc.last_synced).toLocaleString()
                      : 'Never'}
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-1">
                      <button
                        onClick={() => handleSync(acc.id)}
                        disabled={syncingId === acc.id}
                        className="p-1.5 text-blue-600 hover:text-blue-800 disabled:opacity-50"
                        title="Sync"
                      >
                        <RefreshCw size={15} className={syncingId === acc.id ? 'animate-spin' : ''} />
                      </button>
                      <button
                        onClick={() => setEditAccount({ id: acc.id, name: acc.name, provider_type: acc.provider_type })}
                        className="p-1.5 text-gray-500 hover:text-gray-700"
                        title="Edit"
                      >
                        <Pencil size={15} />
                      </button>
                      <button
                        onClick={() => setConfirmDelete(acc.id)}
                        className="p-1.5 text-red-500 hover:text-red-700"
                        title="Delete"
                      >
                        <Trash2 size={15} />
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {result && (
        <Pagination
          page={result.page}
          perPage={result.per_page}
          total={result.total}
          onPageChange={setPage}
        />
      )}

      {showCreate && <DnsAccountDialog onClose={() => setShowCreate(false)} />}
      {editAccount && (
        <DnsAccountDialog account={editAccount} onClose={() => setEditAccount(null)} />
      )}

      <ConfirmDialog
        open={!!confirmDelete}
        title="Delete DNS Account"
        message="This will delete this account and all its synced zones and records."
        confirmLabel="Delete"
        variant="danger"
        onConfirm={() => {
          if (confirmDelete) deleteMutation.mutate(confirmDelete);
          setConfirmDelete(null);
        }}
        onCancel={() => setConfirmDelete(null)}
      />
    </>
  );
}

// ─── Zones Tab ────────────────────────────────────────────

function ExpiryCell({ date }: { date: string | null }) {
  if (!date) return <span className="text-gray-400">-</span>;
  const now = new Date();
  const exp = new Date(date);
  const days = Math.ceil((exp.getTime() - now.getTime()) / (1000 * 60 * 60 * 24));

  let colorClass = 'text-green-700 bg-green-50';
  if (days < 7) colorClass = 'text-red-700 bg-red-50';
  else if (days < 30) colorClass = 'text-yellow-700 bg-yellow-50';

  return (
    <span className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${colorClass}`}>
      {exp.toLocaleDateString()} ({days}d)
    </span>
  );
}

function ZonesTab() {
  const [page, setPage] = useState(1);
  const [accountFilter, setAccountFilter] = useState('');
  const { data: accounts } = useDnsAccounts({ per_page: 100 });
  const { data: result, isLoading } = useDnsZones({
    account_id: accountFilter || undefined,
    page,
    per_page: 25,
  });

  return (
    <>
      <div className="mt-4 flex items-center gap-3">
        <select
          value={accountFilter}
          onChange={(e) => { setAccountFilter(e.target.value); setPage(1); }}
          className="border rounded-md px-3 py-1.5 text-sm bg-white"
        >
          <option value="">All Accounts</option>
          {accounts?.data.map((a) => (
            <option key={a.id} value={a.id}>{a.name}</option>
          ))}
        </select>
      </div>

      <div className="mt-4 bg-white rounded-lg border overflow-x-auto">
        {isLoading ? (
          <div className="p-8 text-center text-gray-500">Loading...</div>
        ) : !result?.data.length ? (
          <div className="p-8 text-center text-gray-400">No zones found. Sync an account first.</div>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-gray-500 border-b bg-gray-50">
                <th className="px-4 py-3 font-medium">Zone</th>
                <th className="px-4 py-3 font-medium">Status</th>
                <th className="px-4 py-3 font-medium">Domain Expires</th>
                <th className="px-4 py-3 font-medium">Cert Expires</th>
                <th className="px-4 py-3 font-medium">Last Synced</th>
              </tr>
            </thead>
            <tbody>
              {result.data.map((zone) => (
                <tr key={zone.id} className="border-b last:border-0 hover:bg-gray-50">
                  <td className="px-4 py-3 font-medium text-gray-900">{zone.zone_name}</td>
                  <td className="px-4 py-3">
                    <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${
                      zone.status === 'active'
                        ? 'bg-green-100 text-green-800'
                        : 'bg-gray-100 text-gray-600'
                    }`}>
                      {zone.status}
                    </span>
                  </td>
                  <td className="px-4 py-3"><ExpiryCell date={zone.domain_expires_at} /></td>
                  <td className="px-4 py-3"><ExpiryCell date={zone.cert_expires_at} /></td>
                  <td className="px-4 py-3 text-gray-500 text-xs">
                    {new Date(zone.synced_at).toLocaleString()}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {result && (
        <Pagination
          page={result.page}
          perPage={result.per_page}
          total={result.total}
          onPageChange={setPage}
        />
      )}
    </>
  );
}

// ─── Records Tab ──────────────────────────────────────────

const RECORD_TYPE_COLORS: Record<string, string> = {
  A: 'bg-blue-100 text-blue-800',
  AAAA: 'bg-indigo-100 text-indigo-800',
  CNAME: 'bg-purple-100 text-purple-800',
  MX: 'bg-orange-100 text-orange-800',
  TXT: 'bg-gray-100 text-gray-700',
  NS: 'bg-green-100 text-green-800',
  SRV: 'bg-cyan-100 text-cyan-800',
};

function RecordsTab() {
  const [page, setPage] = useState(1);
  const [accountFilter, setAccountFilter] = useState('');
  const [zoneFilter, setZoneFilter] = useState('');
  const [typeFilter, setTypeFilter] = useState('');
  const [vpsFilter, setVpsFilter] = useState('');

  const { data: accounts } = useDnsAccounts({ per_page: 100 });
  const { data: zones } = useDnsZones({
    account_id: accountFilter || undefined,
    per_page: 1000,
  });

  const { data: result, isLoading } = useDnsRecords({
    account_id: accountFilter || undefined,
    zone_name: zoneFilter || undefined,
    record_type: typeFilter || undefined,
    has_vps: vpsFilter === 'linked' ? true : vpsFilter === 'unlinked' ? false : undefined,
    page,
    per_page: 25,
  });

  const uniqueZoneNames = zones?.data
    ? [...new Set(zones.data.map((z) => z.zone_name))].sort()
    : [];

  return (
    <>
      <div className="mt-4 flex flex-wrap items-center gap-3">
        <select
          value={accountFilter}
          onChange={(e) => { setAccountFilter(e.target.value); setZoneFilter(''); setPage(1); }}
          className="border rounded-md px-3 py-1.5 text-sm bg-white"
        >
          <option value="">All Accounts</option>
          {accounts?.data.map((a) => (
            <option key={a.id} value={a.id}>{a.name}</option>
          ))}
        </select>

        <select
          value={zoneFilter}
          onChange={(e) => { setZoneFilter(e.target.value); setPage(1); }}
          className="border rounded-md px-3 py-1.5 text-sm bg-white"
        >
          <option value="">All Zones</option>
          {uniqueZoneNames.map((name) => (
            <option key={name} value={name}>{name}</option>
          ))}
        </select>

        <select
          value={typeFilter}
          onChange={(e) => { setTypeFilter(e.target.value); setPage(1); }}
          className="border rounded-md px-3 py-1.5 text-sm bg-white"
        >
          <option value="">All Types</option>
          {['A', 'AAAA', 'CNAME', 'MX', 'TXT', 'NS', 'SRV'].map((t) => (
            <option key={t} value={t}>{t}</option>
          ))}
        </select>

        <select
          value={vpsFilter}
          onChange={(e) => { setVpsFilter(e.target.value); setPage(1); }}
          className="border rounded-md px-3 py-1.5 text-sm bg-white"
        >
          <option value="">All Records</option>
          <option value="linked">Linked to VPS</option>
          <option value="unlinked">Not linked</option>
        </select>
      </div>

      <div className="mt-4 bg-white rounded-lg border overflow-x-auto">
        {isLoading ? (
          <div className="p-8 text-center text-gray-500">Loading...</div>
        ) : !result?.data.length ? (
          <div className="p-8 text-center text-gray-400">No DNS records found</div>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-gray-500 border-b bg-gray-50">
                <th className="px-4 py-3 font-medium">Zone</th>
                <th className="px-4 py-3 font-medium">Name</th>
                <th className="px-4 py-3 font-medium">Type</th>
                <th className="px-4 py-3 font-medium">Content</th>
                <th className="px-4 py-3 font-medium">TTL</th>
                <th className="px-4 py-3 font-medium">Proxied</th>
                <th className="px-4 py-3 font-medium">VPS</th>
              </tr>
            </thead>
            <tbody>
              {result.data.map((rec) => {
                const proxied = rec.extra?.proxied === true;
                return (
                  <tr key={rec.id} className="border-b last:border-0 hover:bg-gray-50">
                    <td className="px-4 py-3 text-gray-500">{rec.zone_name}</td>
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
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      {result && (
        <Pagination
          page={result.page}
          perPage={result.per_page}
          total={result.total}
          onPageChange={setPage}
        />
      )}
    </>
  );
}
