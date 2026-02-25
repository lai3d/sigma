import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import * as api from '@/api/cloudAccounts';
import type { CreateCloudAccount, UpdateCloudAccount } from '@/types/api';

export function useCloudAccounts(query?: { page?: number; per_page?: number }) {
  return useQuery({
    queryKey: ['cloud-accounts', query],
    queryFn: () => api.listAccounts(query),
  });
}

export function useCreateCloudAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateCloudAccount) => api.createAccount(input),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['cloud-accounts'] }),
  });
}

export function useUpdateCloudAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateCloudAccount }) =>
      api.updateAccount(id, data),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['cloud-accounts'] }),
  });
}

export function useDeleteCloudAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.deleteAccount(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['cloud-accounts'] }),
  });
}

export function useSyncCloudAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.syncAccount(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['cloud-accounts'] });
      qc.invalidateQueries({ queryKey: ['vps'] });
    },
  });
}
