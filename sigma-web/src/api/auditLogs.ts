import { apiClient } from './client';
import type { AuditLog, AuditLogQuery, PaginatedResponse } from '@/types/api';

export async function listAuditLogs(query?: AuditLogQuery): Promise<PaginatedResponse<AuditLog>> {
  const params = new URLSearchParams();
  if (query?.page) params.set('page', String(query.page));
  if (query?.per_page) params.set('per_page', String(query.per_page));
  if (query?.resource) params.set('resource', query.resource);
  if (query?.resource_id) params.set('resource_id', query.resource_id);
  if (query?.user_id) params.set('user_id', query.user_id);
  if (query?.action) params.set('action', query.action);
  if (query?.since) params.set('since', query.since);
  if (query?.until) params.set('until', query.until);
  const { data } = await apiClient.get(`/audit-logs?${params.toString()}`);
  return data;
}
