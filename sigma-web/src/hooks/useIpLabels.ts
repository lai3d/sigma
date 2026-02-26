import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import * as api from '@/api/ipLabels';
import type { CreateIpLabel, UpdateIpLabel } from '@/types/api';

export function useIpLabels(query?: { page?: number; per_page?: number }) {
  return useQuery({
    queryKey: ['ip-labels', query],
    queryFn: () => api.listIpLabels(query),
  });
}

export function useIpLabel(id: string) {
  return useQuery({
    queryKey: ['ip-labels', id],
    queryFn: () => api.getIpLabel(id),
    enabled: !!id,
  });
}

export function useCreateIpLabel() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateIpLabel) => api.createIpLabel(input),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['ip-labels'] }),
  });
}

export function useUpdateIpLabel() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateIpLabel }) =>
      api.updateIpLabel(id, data),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['ip-labels'] }),
  });
}

export function useDeleteIpLabel() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.deleteIpLabel(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['ip-labels'] }),
  });
}
