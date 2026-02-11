import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import * as api from '@/api/providers';
import type { CreateProvider, UpdateProvider, ImportResult } from '@/types/api';

export function useProviders(query?: { page?: number; per_page?: number }) {
  return useQuery({
    queryKey: ['providers', query],
    queryFn: () => api.listProviders(query),
  });
}

export function useProvider(id: string) {
  return useQuery({
    queryKey: ['providers', id],
    queryFn: () => api.getProvider(id),
    enabled: !!id,
  });
}

export function useCreateProvider() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateProvider) => api.createProvider(input),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['providers'] }),
  });
}

export function useUpdateProvider() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateProvider }) =>
      api.updateProvider(id, data),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['providers'] }),
  });
}

export function useDeleteProvider() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.deleteProvider(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['providers'] }),
  });
}

export function useImportProviders() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ format, data }: { format: 'csv' | 'json'; data: string }) =>
      api.importProviders(format, data),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['providers'] }),
  });
}
