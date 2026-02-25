import { apiClient } from './client';
import type { VpsPurposeRecord, CreateVpsPurpose, UpdateVpsPurpose, PaginatedResponse } from '@/types/api';

export async function listVpsPurposes(query?: { page?: number; per_page?: number }): Promise<PaginatedResponse<VpsPurposeRecord>> {
  const params = new URLSearchParams();
  if (query?.page) params.set('page', String(query.page));
  if (query?.per_page) params.set('per_page', String(query.per_page));
  const { data } = await apiClient.get(`/vps-purposes?${params.toString()}`);
  return data;
}

export async function getVpsPurpose(id: string): Promise<VpsPurposeRecord> {
  const { data } = await apiClient.get(`/vps-purposes/${id}`);
  return data;
}

export async function createVpsPurpose(input: CreateVpsPurpose): Promise<VpsPurposeRecord> {
  const { data } = await apiClient.post('/vps-purposes', input);
  return data;
}

export async function updateVpsPurpose(id: string, input: UpdateVpsPurpose): Promise<VpsPurposeRecord> {
  const { data } = await apiClient.put(`/vps-purposes/${id}`, input);
  return data;
}

export async function deleteVpsPurpose(id: string): Promise<void> {
  await apiClient.delete(`/vps-purposes/${id}`);
}
