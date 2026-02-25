import { apiClient } from './client';
import type {
  CloudAccountResponse,
  CreateCloudAccount,
  UpdateCloudAccount,
  CloudSyncResult,
  PaginatedResponse,
} from '@/types/api';

export async function listAccounts(query?: { page?: number; per_page?: number }): Promise<PaginatedResponse<CloudAccountResponse>> {
  const params = new URLSearchParams();
  if (query?.page) params.set('page', String(query.page));
  if (query?.per_page) params.set('per_page', String(query.per_page));
  const { data } = await apiClient.get(`/cloud-accounts?${params.toString()}`);
  return data;
}

export async function getAccount(id: string): Promise<CloudAccountResponse> {
  const { data } = await apiClient.get(`/cloud-accounts/${id}`);
  return data;
}

export async function createAccount(input: CreateCloudAccount): Promise<CloudAccountResponse> {
  const { data } = await apiClient.post('/cloud-accounts', input);
  return data;
}

export async function updateAccount(id: string, input: UpdateCloudAccount): Promise<CloudAccountResponse> {
  const { data } = await apiClient.put(`/cloud-accounts/${id}`, input);
  return data;
}

export async function deleteAccount(id: string): Promise<void> {
  await apiClient.delete(`/cloud-accounts/${id}`);
}

export async function syncAccount(id: string): Promise<CloudSyncResult> {
  const { data } = await apiClient.post(`/cloud-accounts/${id}/sync`);
  return data;
}
