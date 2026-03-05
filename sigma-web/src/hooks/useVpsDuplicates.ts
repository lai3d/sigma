import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import * as api from '@/api/vpsDuplicates';
import type { MergeVpsRequest } from '@/types/api';

export function useVpsDuplicates() {
  return useQuery({
    queryKey: ['vps-duplicates'],
    queryFn: () => api.detectDuplicates(),
  });
}

export function useMergeVps() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: MergeVpsRequest) => api.mergeVps(input),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['vps-duplicates'] });
      qc.invalidateQueries({ queryKey: ['vps'] });
    },
  });
}
