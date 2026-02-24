import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import * as api from '@/api/cloudflare';
import type {
  CreateCloudflareAccount,
  UpdateCloudflareAccount,
  CloudflareZoneListQuery,
  CloudflareDnsListQuery,
} from '@/types/api';

export function useCloudflareAccounts(query?: { page?: number; per_page?: number }) {
  return useQuery({
    queryKey: ['cloudflare-accounts', query],
    queryFn: () => api.listAccounts(query),
  });
}

export function useCreateCloudflareAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateCloudflareAccount) => api.createAccount(input),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['cloudflare-accounts'] }),
  });
}

export function useUpdateCloudflareAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateCloudflareAccount }) =>
      api.updateAccount(id, data),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['cloudflare-accounts'] }),
  });
}

export function useDeleteCloudflareAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.deleteAccount(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['cloudflare-accounts'] }),
  });
}

export function useSyncCloudflareAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.syncAccount(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['cloudflare-accounts'] });
      qc.invalidateQueries({ queryKey: ['cloudflare-zones'] });
      qc.invalidateQueries({ queryKey: ['cloudflare-dns-records'] });
    },
  });
}

export function useCloudflareZones(query?: CloudflareZoneListQuery) {
  return useQuery({
    queryKey: ['cloudflare-zones', query],
    queryFn: () => api.listZones(query),
  });
}

export function useCloudflareDnsRecords(query?: CloudflareDnsListQuery) {
  return useQuery({
    queryKey: ['cloudflare-dns-records', query],
    queryFn: () => api.listDnsRecords(query),
  });
}
