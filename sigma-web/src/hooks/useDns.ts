import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import * as api from '@/api/dns';
import type {
  CreateDnsAccount,
  UpdateDnsAccount,
  DnsZoneListQuery,
  DnsRecordListQuery,
} from '@/types/api';

export function useDnsAccounts(query?: { page?: number; per_page?: number }) {
  return useQuery({
    queryKey: ['dns-accounts', query],
    queryFn: () => api.listAccounts(query),
  });
}

export function useCreateDnsAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateDnsAccount) => api.createAccount(input),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['dns-accounts'] }),
  });
}

export function useUpdateDnsAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateDnsAccount }) =>
      api.updateAccount(id, data),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['dns-accounts'] }),
  });
}

export function useDeleteDnsAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.deleteAccount(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['dns-accounts'] }),
  });
}

export function useSyncDnsAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.syncAccount(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['dns-accounts'] });
      qc.invalidateQueries({ queryKey: ['dns-zones'] });
      qc.invalidateQueries({ queryKey: ['dns-records'] });
    },
  });
}

export function useDnsZones(query?: DnsZoneListQuery) {
  return useQuery({
    queryKey: ['dns-zones', query],
    queryFn: () => api.listZones(query),
  });
}

export function useDnsRecords(query?: DnsRecordListQuery) {
  return useQuery({
    queryKey: ['dns-records', query],
    queryFn: () => api.listDnsRecords(query),
  });
}
