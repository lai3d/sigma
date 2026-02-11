import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import Layout from '@/components/Layout';
import Dashboard from '@/pages/Dashboard';
import VpsList from '@/pages/VpsList';
import VpsForm from '@/pages/VpsForm';
import ProviderList from '@/pages/ProviderList';
import SettingsPage from '@/pages/SettingsPage';

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
      <BrowserRouter>
        <Routes>
          <Route element={<Layout />}>
            <Route path="/" element={<Dashboard />} />
            <Route path="/vps" element={<VpsList />} />
            <Route path="/vps/new" element={<VpsForm />} />
            <Route path="/vps/:id" element={<VpsForm />} />
            <Route path="/providers" element={<ProviderList />} />
            <Route path="/settings" element={<SettingsPage />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
