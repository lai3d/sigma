import { apiClient } from './client';
import type {
  CloudflareAccountResponse,
  CreateCloudflareAccount,
  UpdateCloudflareAccount,
  CloudflareZone,
  CloudflareDnsRecord,
  CloudflareSyncResult,
  CloudflareZoneListQuery,
  CloudflareDnsListQuery,
  PaginatedResponse,
} from '@/types/api';

export async function listAccounts(query?: { page?: number; per_page?: number }): Promise<PaginatedResponse<CloudflareAccountResponse>> {
  const params = new URLSearchParams();
  if (query?.page) params.set('page', String(query.page));
  if (query?.per_page) params.set('per_page', String(query.per_page));
  const { data } = await apiClient.get(`/cloudflare-accounts?${params.toString()}`);
  return data;
}

export async function getAccount(id: string): Promise<CloudflareAccountResponse> {
  const { data } = await apiClient.get(`/cloudflare-accounts/${id}`);
  return data;
}

export async function createAccount(input: CreateCloudflareAccount): Promise<CloudflareAccountResponse> {
  const { data } = await apiClient.post('/cloudflare-accounts', input);
  return data;
}

export async function updateAccount(id: string, input: UpdateCloudflareAccount): Promise<CloudflareAccountResponse> {
  const { data } = await apiClient.put(`/cloudflare-accounts/${id}`, input);
  return data;
}

export async function deleteAccount(id: string): Promise<void> {
  await apiClient.delete(`/cloudflare-accounts/${id}`);
}

export async function syncAccount(id: string): Promise<CloudflareSyncResult> {
  const { data } = await apiClient.post(`/cloudflare-accounts/${id}/sync`);
  return data;
}

export async function listZones(query?: CloudflareZoneListQuery): Promise<PaginatedResponse<CloudflareZone>> {
  const params = new URLSearchParams();
  if (query?.account_id) params.set('account_id', query.account_id);
  if (query?.page) params.set('page', String(query.page));
  if (query?.per_page) params.set('per_page', String(query.per_page));
  const { data } = await apiClient.get(`/cloudflare-zones?${params.toString()}`);
  return data;
}

export async function listDnsRecords(query?: CloudflareDnsListQuery): Promise<PaginatedResponse<CloudflareDnsRecord>> {
  const params = new URLSearchParams();
  if (query?.account_id) params.set('account_id', query.account_id);
  if (query?.zone_name) params.set('zone_name', query.zone_name);
  if (query?.record_type) params.set('record_type', query.record_type);
  if (query?.has_vps !== undefined) params.set('has_vps', String(query.has_vps));
  if (query?.page) params.set('page', String(query.page));
  if (query?.per_page) params.set('per_page', String(query.per_page));
  const { data } = await apiClient.get(`/cloudflare-dns-records?${params.toString()}`);
  return data;
}
