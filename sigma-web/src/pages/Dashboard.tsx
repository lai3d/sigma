import { useStats } from '@/hooks/useStats';
import { formatDate, daysUntil } from '@/lib/utils';
import { Server, Activity, Building2, AlertTriangle } from 'lucide-react';
import StatusBadge from '@/components/StatusBadge';
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  PieChart,
  Pie,
  Cell,
} from 'recharts';

const PIE_COLORS = ['#3b82f6', '#10b981', '#f59e0b', '#6b7280', '#ef4444', '#8b5cf6'];

export default function Dashboard() {
  const { data: stats, isLoading, error } = useStats();

  if (isLoading) return <div className="text-gray-500">Loading...</div>;
  if (error) return <div className="text-red-500">Failed to load stats</div>;
  if (!stats) return null;

  const countryData = stats.by_country.map((c) => ({
    name: c.label || 'Unknown',
    value: c.count || 0,
  }));

  const providerData = stats.by_provider.map((p) => ({
    name: p.label || 'Unknown',
    value: p.count || 0,
  }));

  const statusData = stats.by_status.map((s) => ({
    name: s.label || 'Unknown',
    value: s.count || 0,
  }));

  return (
    <div>
      <h2 className="text-2xl font-bold text-gray-900">Dashboard</h2>

      {/* Stats cards */}
      <div className="mt-6 grid grid-cols-1 sm:grid-cols-3 gap-4">
        <StatCard
          icon={<Server size={24} />}
          label="Total VPS"
          value={stats.total_vps}
          color="blue"
        />
        <StatCard
          icon={<Activity size={24} />}
          label="Active VPS"
          value={stats.active_vps}
          color="green"
        />
        <StatCard
          icon={<Building2 size={24} />}
          label="Providers"
          value={stats.total_providers}
          color="purple"
        />
      </div>

      {/* Charts */}
      <div className="mt-8 grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* By Country */}
        <div className="bg-white rounded-lg border p-5">
          <h3 className="text-sm font-semibold text-gray-700 mb-4">By Country</h3>
          {countryData.length > 0 ? (
            <ResponsiveContainer width="100%" height={250}>
              <BarChart data={countryData}>
                <XAxis dataKey="name" tick={{ fontSize: 12 }} />
                <YAxis allowDecimals={false} tick={{ fontSize: 12 }} />
                <Tooltip />
                <Bar dataKey="value" fill="#3b82f6" radius={[4, 4, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          ) : (
            <p className="text-gray-400 text-sm">No data</p>
          )}
        </div>

        {/* By Status */}
        <div className="bg-white rounded-lg border p-5">
          <h3 className="text-sm font-semibold text-gray-700 mb-4">By Status</h3>
          {statusData.length > 0 ? (
            <ResponsiveContainer width="100%" height={250}>
              <PieChart>
                <Pie
                  data={statusData}
                  dataKey="value"
                  nameKey="name"
                  cx="50%"
                  cy="50%"
                  outerRadius={90}
                  label={({ name, value }) => `${name}: ${value}`}
                >
                  {statusData.map((_, i) => (
                    <Cell key={i} fill={PIE_COLORS[i % PIE_COLORS.length]} />
                  ))}
                </Pie>
                <Tooltip />
              </PieChart>
            </ResponsiveContainer>
          ) : (
            <p className="text-gray-400 text-sm">No data</p>
          )}
        </div>

        {/* By Provider */}
        <div className="bg-white rounded-lg border p-5 lg:col-span-2">
          <h3 className="text-sm font-semibold text-gray-700 mb-4">By Provider</h3>
          {providerData.length > 0 ? (
            <ResponsiveContainer width="100%" height={Math.max(200, providerData.length * 36)}>
              <BarChart data={providerData} layout="vertical">
                <XAxis type="number" allowDecimals={false} tick={{ fontSize: 12 }} />
                <YAxis
                  type="category"
                  dataKey="name"
                  width={140}
                  tick={{ fontSize: 12 }}
                />
                <Tooltip />
                <Bar dataKey="value" fill="#8b5cf6" radius={[0, 4, 4, 0]} />
              </BarChart>
            </ResponsiveContainer>
          ) : (
            <p className="text-gray-400 text-sm">No data</p>
          )}
        </div>
      </div>

      {/* Expiring soon */}
      {stats.expiring_soon.length > 0 && (
        <div className="mt-8 bg-white rounded-lg border p-5">
          <h3 className="text-sm font-semibold text-gray-700 mb-4 flex items-center gap-2">
            <AlertTriangle size={16} className="text-orange-500" />
            Expiring Within 14 Days
          </h3>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-gray-500 border-b">
                  <th className="pb-2 font-medium">Hostname</th>
                  <th className="pb-2 font-medium">IP</th>
                  <th className="pb-2 font-medium">Country</th>
                  <th className="pb-2 font-medium">Status</th>
                  <th className="pb-2 font-medium">Expires</th>
                  <th className="pb-2 font-medium">Days Left</th>
                </tr>
              </thead>
              <tbody>
                {stats.expiring_soon.map((vps) => {
                  const days = daysUntil(vps.expire_date);
                  return (
                    <tr key={vps.id} className="border-b last:border-0">
                      <td className="py-2 font-mono">{vps.hostname}</td>
                      <td className="py-2 font-mono text-xs">
                        {vps.ip_addresses.map((e) => e.ip).join(', ')}
                      </td>
                      <td className="py-2">{vps.country}</td>
                      <td className="py-2">
                        <StatusBadge status={vps.status} />
                      </td>
                      <td className="py-2">{formatDate(vps.expire_date)}</td>
                      <td className="py-2">
                        <span
                          className={
                            days !== null && days <= 3
                              ? 'text-red-600 font-semibold'
                              : 'text-orange-600'
                          }
                        >
                          {days !== null ? `${days}d` : '-'}
                        </span>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}

function StatCard({
  icon,
  label,
  value,
  color,
}: {
  icon: React.ReactNode;
  label: string;
  value: number;
  color: 'blue' | 'green' | 'purple';
}) {
  const bg = { blue: 'bg-blue-50', green: 'bg-green-50', purple: 'bg-purple-50' }[color];
  const iconColor = {
    blue: 'text-blue-600',
    green: 'text-green-600',
    purple: 'text-purple-600',
  }[color];

  return (
    <div className="bg-white rounded-lg border p-5">
      <div className="flex items-center gap-3">
        <div className={`${bg} ${iconColor} p-2 rounded-lg`}>{icon}</div>
        <div>
          <p className="text-sm text-gray-500">{label}</p>
          <p className="text-2xl font-bold text-gray-900">{value}</p>
        </div>
      </div>
    </div>
  );
}
