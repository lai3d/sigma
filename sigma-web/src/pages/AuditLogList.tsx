import { Fragment, useState } from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { useAuditLogs } from '@/hooks/useAuditLogs';
import Pagination from '@/components/Pagination';

const RESOURCE_OPTIONS = ['', 'vps', 'provider', 'user'];
const ACTION_OPTIONS = ['', 'create', 'update', 'delete', 'retire', 'import', 'login'];

const ACTION_COLORS: Record<string, string> = {
  create: 'bg-green-50 text-green-700',
  update: 'bg-blue-50 text-blue-700',
  delete: 'bg-red-50 text-red-700',
  retire: 'bg-orange-50 text-orange-700',
  import: 'bg-purple-50 text-purple-700',
  login: 'bg-gray-100 text-gray-600',
};

function timeAgo(dateStr: string): string {
  const now = Date.now();
  const then = new Date(dateStr).getTime();
  const seconds = Math.floor((now - then) / 1000);
  if (seconds < 60) return 'just now';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  return new Date(dateStr).toLocaleDateString();
}

function resourceLink(resource: string, resourceId: string | null): string | null {
  if (!resourceId) return null;
  if (resource === 'vps') return `/vps/${resourceId}`;
  return null;
}

export default function AuditLogList() {
  const [page, setPage] = useState(1);
  const [resource, setResource] = useState('');
  const [action, setAction] = useState('');
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  const { data: result, isLoading } = useAuditLogs({
    page,
    per_page: 50,
    resource: resource || undefined,
    action: action || undefined,
  });

  const logs = result?.data;

  const toggleExpand = (id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  return (
    <div>
      <h2 className="text-2xl font-bold text-gray-900">Audit Log</h2>

      {/* Filters */}
      <div className="mt-4 flex items-center gap-3">
        <select
          value={resource}
          onChange={(e) => { setResource(e.target.value); setPage(1); }}
          className="border rounded-md px-3 py-1.5 text-sm bg-white"
        >
          <option value="">All resources</option>
          {RESOURCE_OPTIONS.filter(Boolean).map((r) => (
            <option key={r} value={r}>{r}</option>
          ))}
        </select>
        <select
          value={action}
          onChange={(e) => { setAction(e.target.value); setPage(1); }}
          className="border rounded-md px-3 py-1.5 text-sm bg-white"
        >
          <option value="">All actions</option>
          {ACTION_OPTIONS.filter(Boolean).map((a) => (
            <option key={a} value={a}>{a}</option>
          ))}
        </select>
      </div>

      <div className="mt-4 bg-white rounded-lg border overflow-x-auto">
        {isLoading ? (
          <div className="p-8 text-center text-gray-500">Loading...</div>
        ) : !logs?.length ? (
          <div className="p-8 text-center text-gray-400">No audit log entries</div>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-gray-500 border-b bg-gray-50">
                <th className="px-4 py-3 font-medium w-8"></th>
                <th className="px-4 py-3 font-medium">Time</th>
                <th className="px-4 py-3 font-medium">User</th>
                <th className="px-4 py-3 font-medium">Action</th>
                <th className="px-4 py-3 font-medium">Resource</th>
                <th className="px-4 py-3 font-medium">Resource ID</th>
              </tr>
            </thead>
            <tbody>
              {logs.map((log) => {
                const isExpanded = expanded.has(log.id);
                const link = resourceLink(log.resource, log.resource_id);
                const hasDetails = Object.keys(log.details).length > 0;
                return (
                  <Fragment key={log.id}>
                    <tr className="border-b last:border-0 hover:bg-gray-50 align-top">
                      <td className="px-4 py-3">
                        {hasDetails && (
                          <button onClick={() => toggleExpand(log.id)} className="text-gray-400 hover:text-gray-600">
                            {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                          </button>
                        )}
                      </td>
                      <td className="px-4 py-3 text-gray-500 whitespace-nowrap" title={new Date(log.created_at).toLocaleString()}>
                        {timeAgo(log.created_at)}
                      </td>
                      <td className="px-4 py-3 text-gray-900">{log.user_email}</td>
                      <td className="px-4 py-3">
                        <span className={`inline-block px-2 py-0.5 rounded text-xs font-medium ${ACTION_COLORS[log.action] || 'bg-gray-100 text-gray-600'}`}>
                          {log.action}
                        </span>
                      </td>
                      <td className="px-4 py-3 text-gray-700">{log.resource}</td>
                      <td className="px-4 py-3">
                        {log.resource_id ? (
                          link ? (
                            <a href={link} className="text-blue-600 hover:underline font-mono text-xs">{log.resource_id.slice(0, 8)}</a>
                          ) : (
                            <span className="font-mono text-xs text-gray-500">{log.resource_id.slice(0, 8)}</span>
                          )
                        ) : (
                          <span className="text-gray-400">-</span>
                        )}
                      </td>
                    </tr>
                    {isExpanded && hasDetails && (
                      <tr className="bg-gray-50 border-b">
                        <td></td>
                        <td colSpan={5} className="px-4 py-2">
                          <pre className="text-xs text-gray-600 whitespace-pre-wrap">{JSON.stringify(log.details, null, 2)}</pre>
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

      {result && (
        <Pagination
          page={result.page}
          perPage={result.per_page}
          total={result.total}
          onPageChange={setPage}
        />
      )}
    </div>
  );
}
