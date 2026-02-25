import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import * as api from '@/api/vpsPurposes';
import type { CreateVpsPurpose, UpdateVpsPurpose } from '@/types/api';

export function useVpsPurposes(query?: { page?: number; per_page?: number }) {
  return useQuery({
    queryKey: ['vps-purposes', query],
    queryFn: () => api.listVpsPurposes(query),
  });
}

export function useVpsPurpose(id: string) {
  return useQuery({
    queryKey: ['vps-purposes', id],
    queryFn: () => api.getVpsPurpose(id),
    enabled: !!id,
  });
}

export function useCreateVpsPurpose() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateVpsPurpose) => api.createVpsPurpose(input),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['vps-purposes'] }),
  });
}

export function useUpdateVpsPurpose() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateVpsPurpose }) =>
      api.updateVpsPurpose(id, data),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['vps-purposes'] }),
  });
}

export function useDeleteVpsPurpose() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.deleteVpsPurpose(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['vps-purposes'] }),
  });
}
