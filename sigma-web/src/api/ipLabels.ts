import { apiClient } from './client';
import type { IpLabelRecord, CreateIpLabel, UpdateIpLabel, PaginatedResponse } from '@/types/api';

export async function listIpLabels(query?: { page?: number; per_page?: number }): Promise<PaginatedResponse<IpLabelRecord>> {
  const params = new URLSearchParams();
  if (query?.page) params.set('page', String(query.page));
  if (query?.per_page) params.set('per_page', String(query.per_page));
  const { data } = await apiClient.get(`/ip-labels?${params.toString()}`);
  return data;
}

export async function getIpLabel(id: string): Promise<IpLabelRecord> {
  const { data } = await apiClient.get(`/ip-labels/${id}`);
  return data;
}

export async function createIpLabel(input: CreateIpLabel): Promise<IpLabelRecord> {
  const { data } = await apiClient.post('/ip-labels', input);
  return data;
}

export async function updateIpLabel(id: string, input: UpdateIpLabel): Promise<IpLabelRecord> {
  const { data } = await apiClient.put(`/ip-labels/${id}`, input);
  return data;
}

export async function deleteIpLabel(id: string): Promise<void> {
  await apiClient.delete(`/ip-labels/${id}`);
}
