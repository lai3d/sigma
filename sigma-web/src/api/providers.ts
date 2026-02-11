import { apiClient } from './client';
import type { Provider, CreateProvider, UpdateProvider, ImportResult, PaginatedResponse } from '@/types/api';

export async function listProviders(query?: { page?: number; per_page?: number }): Promise<PaginatedResponse<Provider>> {
  const params = new URLSearchParams();
  if (query?.page) params.set('page', String(query.page));
  if (query?.per_page) params.set('per_page', String(query.per_page));
  const { data } = await apiClient.get(`/providers?${params.toString()}`);
  return data;
}

export async function getProvider(id: string): Promise<Provider> {
  const { data } = await apiClient.get(`/providers/${id}`);
  return data;
}

export async function createProvider(input: CreateProvider): Promise<Provider> {
  const { data } = await apiClient.post('/providers', input);
  return data;
}

export async function updateProvider(id: string, input: UpdateProvider): Promise<Provider> {
  const { data } = await apiClient.put(`/providers/${id}`, input);
  return data;
}

export async function deleteProvider(id: string): Promise<void> {
  await apiClient.delete(`/providers/${id}`);
}

export async function exportProviders(format: 'csv' | 'json'): Promise<Blob> {
  const { data } = await apiClient.get(`/providers/export?format=${format}`, {
    responseType: 'blob',
  });
  return data;
}

export async function importProviders(format: 'csv' | 'json', data: string): Promise<ImportResult> {
  const { data: result } = await apiClient.post('/providers/import', { format, data });
  return result;
}
