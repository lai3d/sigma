import { apiClient } from './client';
import type { ApiKeyResponse, ApiKeyCreatedResponse, CreateApiKey, PaginatedResponse } from '@/types/api';

export async function listApiKeys(query?: { page?: number; per_page?: number }): Promise<PaginatedResponse<ApiKeyResponse>> {
  const params = new URLSearchParams();
  if (query?.page) params.set('page', String(query.page));
  if (query?.per_page) params.set('per_page', String(query.per_page));
  const { data } = await apiClient.get(`/api-keys?${params.toString()}`);
  return data;
}

export async function getApiKey(id: string): Promise<ApiKeyResponse> {
  const { data } = await apiClient.get(`/api-keys/${id}`);
  return data;
}

export async function createApiKey(input: CreateApiKey): Promise<ApiKeyCreatedResponse> {
  const { data } = await apiClient.post('/api-keys', input);
  return data;
}

export async function deleteApiKey(id: string): Promise<void> {
  await apiClient.delete(`/api-keys/${id}`);
}
