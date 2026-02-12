import { useQuery } from '@tanstack/react-query';
import * as api from '@/api/auditLogs';
import type { AuditLogQuery } from '@/types/api';

export function useAuditLogs(query?: AuditLogQuery) {
  return useQuery({
    queryKey: ['audit-logs', query],
    queryFn: () => api.listAuditLogs(query),
  });
}
