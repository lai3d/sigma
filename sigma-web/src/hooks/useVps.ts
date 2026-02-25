import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import * as api from '@/api/vps';
import type { CreateVps, UpdateVps, VpsListQuery, VpsIpHistoryQuery } from '@/types/api';

export function useVpsList(query?: VpsListQuery & { page?: number; per_page?: number }) {
  return useQuery({
    queryKey: ['vps', query],
    queryFn: () => api.listVps(query),
  });
}

export function useVps(id: string) {
  return useQuery({
    queryKey: ['vps', id],
    queryFn: () => api.getVps(id),
    enabled: !!id,
  });
}

export function useCreateVps() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateVps) => api.createVps(input),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['vps'] }),
  });
}

export function useUpdateVps() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateVps }) =>
      api.updateVps(id, data),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['vps'] }),
  });
}

export function useDeleteVps() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.deleteVps(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['vps'] }),
  });
}

export function useRetireVps() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.retireVps(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['vps'] }),
  });
}

export function useImportVps() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ format, data }: { format: 'csv' | 'json'; data: string }) =>
      api.importVps(format, data),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['vps'] }),
  });
}

export function useVpsIpHistory(id: string, query?: VpsIpHistoryQuery) {
  return useQuery({
    queryKey: ['vps', id, 'ip-history', query],
    queryFn: () => api.getVpsIpHistory(id, query),
    enabled: !!id,
  });
}

export function useAllocatePorts() {
  return useMutation({
    mutationFn: ({ id, count }: { id: string; count: number }) =>
      api.allocatePorts(id, count),
  });
}
