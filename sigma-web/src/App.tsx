import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AuthProvider } from '@/contexts/AuthContext';
import ProtectedRoute from '@/components/ProtectedRoute';
import Layout from '@/components/Layout';
import Dashboard from '@/pages/Dashboard';
import VpsList from '@/pages/VpsList';
import VpsForm from '@/pages/VpsForm';
import ProviderList from '@/pages/ProviderList';
import SettingsPage from '@/pages/SettingsPage';
import LoginPage from '@/pages/LoginPage';
import ChangePasswordPage from '@/pages/ChangePasswordPage';
import UserList from '@/pages/UserList';
import AuditLogList from '@/pages/AuditLogList';
import TicketList from '@/pages/TicketList';
import TicketDetail from '@/pages/TicketDetail';
import TicketForm from '@/pages/TicketForm';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: 1,
    },
  },
});

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <AuthProvider>
        <BrowserRouter>
          <Routes>
            <Route path="/login" element={<LoginPage />} />
            <Route path="/change-password" element={
              <ProtectedRoute><ChangePasswordPage /></ProtectedRoute>
            } />
            <Route element={
              <ProtectedRoute><Layout /></ProtectedRoute>
            }>
              <Route path="/" element={<Dashboard />} />
              <Route path="/vps" element={<VpsList />} />
              <Route path="/vps/new" element={<VpsForm />} />
              <Route path="/vps/:id" element={<VpsForm />} />
              <Route path="/providers" element={<ProviderList />} />
              <Route path="/tickets" element={<TicketList />} />
              <Route path="/tickets/new" element={<TicketForm />} />
              <Route path="/tickets/:id" element={<TicketDetail />} />
              <Route path="/tickets/:id/edit" element={<TicketForm />} />
              <Route path="/users" element={
                <ProtectedRoute requiredRole="admin"><UserList /></ProtectedRoute>
              } />
              <Route path="/audit-log" element={
                <ProtectedRoute requiredRole="admin"><AuditLogList /></ProtectedRoute>
              } />
              <Route path="/settings" element={<SettingsPage />} />
            </Route>
          </Routes>
        </BrowserRouter>
      </AuthProvider>
    </QueryClientProvider>
  );
}
