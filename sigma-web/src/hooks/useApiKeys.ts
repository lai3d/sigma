import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import * as api from '@/api/apiKeys';
import type { CreateApiKey } from '@/types/api';

export function useApiKeys(query?: { page?: number; per_page?: number }) {
  return useQuery({
    queryKey: ['apiKeys', query],
    queryFn: () => api.listApiKeys(query),
  });
}

export function useCreateApiKey() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateApiKey) => api.createApiKey(input),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['apiKeys'] }),
  });
}

export function useDeleteApiKey() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.deleteApiKey(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['apiKeys'] }),
  });
}
