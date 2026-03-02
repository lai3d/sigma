import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import * as api from '@/api/settings';

export function useSettings() {
  return useQuery({
    queryKey: ['system-settings'],
    queryFn: api.getSettings,
  });
}

export function useUpdateSettings() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (settings: Record<string, string>) => api.updateSettings(settings),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['system-settings'] }),
  });
}
