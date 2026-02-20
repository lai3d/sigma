import { NavLink, Outlet } from 'react-router-dom';
import { LayoutDashboard, Server, Building2, Settings, Users, ClipboardList, Ticket, Network, LogOut } from 'lucide-react';
import { cn } from '@/lib/utils';
import { useAuth } from '@/contexts/AuthContext';

export default function Layout() {
  const { user, logout } = useAuth();

  const navItems = [
    { to: '/', icon: LayoutDashboard, label: 'Dashboard' },
    { to: '/vps', icon: Server, label: 'VPS' },
    { to: '/providers', icon: Building2, label: 'Providers' },
    { to: '/tickets', icon: Ticket, label: 'Tickets' },
    { to: '/envoy', icon: Network, label: 'Envoy' },
    ...(user?.role === 'admin'
      ? [
          { to: '/users', icon: Users, label: 'Users' },
          { to: '/audit-log', icon: ClipboardList, label: 'Audit Log' },
        ]
      : []),
    { to: '/settings', icon: Settings, label: 'Settings' },
  ];

  return (
    <div className="flex h-screen bg-gray-50">
      {/* Sidebar */}
      <aside className="w-56 bg-gray-900 text-white flex flex-col shrink-0">
        <div className="px-5 py-5 border-b border-gray-800">
          <h1 className="text-lg font-bold tracking-wide">
            <span className="text-blue-400">&#931;</span> Sigma
          </h1>
          <p className="text-xs text-gray-400 mt-0.5">VPS Fleet Management</p>
        </div>
        <nav className="flex-1 px-3 py-4 space-y-1">
          {navItems.map(({ to, icon: Icon, label }) => (
            <NavLink
              key={to}
              to={to}
              end={to === '/'}
              className={({ isActive }) =>
                cn(
                  'flex items-center gap-3 px-3 py-2 rounded-md text-sm transition-colors',
                  isActive
                    ? 'bg-blue-600 text-white'
                    : 'text-gray-300 hover:bg-gray-800 hover:text-white'
                )
              }
            >
              <Icon size={18} />
              {label}
            </NavLink>
          ))}
        </nav>

        {/* Sidebar footer */}
        {user && (
          <div className="px-3 py-4 border-t border-gray-800">
            <div className="px-3 mb-2">
              <p className="text-sm font-medium text-gray-200 truncate">{user.name || user.email}</p>
              <p className="text-xs text-gray-400 truncate">{user.email}</p>
              <span className="inline-block mt-1 px-1.5 py-0.5 text-xs rounded bg-gray-700 text-gray-300">
                {user.role}
              </span>
            </div>
            <button
              onClick={logout}
              className="flex items-center gap-2 w-full px-3 py-2 text-sm text-gray-400 hover:text-white hover:bg-gray-800 rounded-md transition-colors"
            >
              <LogOut size={16} />
              Sign out
            </button>
          </div>
        )}
      </aside>

      {/* Main content */}
      <main className="flex-1 overflow-auto">
        <div className="mx-auto p-6">
          <Outlet />
        </div>
      </main>
    </div>
  );
}
