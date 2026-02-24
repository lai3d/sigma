import { apiClient } from './client';
import type {
  DnsAccountResponse,
  CreateDnsAccount,
  UpdateDnsAccount,
  DnsZone,
  DnsRecord,
  DnsSyncResult,
  DnsZoneListQuery,
  DnsRecordListQuery,
  PaginatedResponse,
} from '@/types/api';

export async function listAccounts(query?: { page?: number; per_page?: number }): Promise<PaginatedResponse<DnsAccountResponse>> {
  const params = new URLSearchParams();
  if (query?.page) params.set('page', String(query.page));
  if (query?.per_page) params.set('per_page', String(query.per_page));
  const { data } = await apiClient.get(`/dns-accounts?${params.toString()}`);
  return data;
}

export async function getAccount(id: string): Promise<DnsAccountResponse> {
  const { data } = await apiClient.get(`/dns-accounts/${id}`);
  return data;
}

export async function createAccount(input: CreateDnsAccount): Promise<DnsAccountResponse> {
  const { data } = await apiClient.post('/dns-accounts', input);
  return data;
}

export async function updateAccount(id: string, input: UpdateDnsAccount): Promise<DnsAccountResponse> {
  const { data } = await apiClient.put(`/dns-accounts/${id}`, input);
  return data;
}

export async function deleteAccount(id: string): Promise<void> {
  await apiClient.delete(`/dns-accounts/${id}`);
}

export async function syncAccount(id: string): Promise<DnsSyncResult> {
  const { data } = await apiClient.post(`/dns-accounts/${id}/sync`);
  return data;
}

export async function listZones(query?: DnsZoneListQuery): Promise<PaginatedResponse<DnsZone>> {
  const params = new URLSearchParams();
  if (query?.account_id) params.set('account_id', query.account_id);
  if (query?.page) params.set('page', String(query.page));
  if (query?.per_page) params.set('per_page', String(query.per_page));
  const { data } = await apiClient.get(`/dns-zones?${params.toString()}`);
  return data;
}

export async function listDnsRecords(query?: DnsRecordListQuery): Promise<PaginatedResponse<DnsRecord>> {
  const params = new URLSearchParams();
  if (query?.account_id) params.set('account_id', query.account_id);
  if (query?.zone_name) params.set('zone_name', query.zone_name);
  if (query?.record_type) params.set('record_type', query.record_type);
  if (query?.has_vps !== undefined) params.set('has_vps', String(query.has_vps));
  if (query?.page) params.set('page', String(query.page));
  if (query?.per_page) params.set('per_page', String(query.per_page));
  const { data } = await apiClient.get(`/dns-records?${params.toString()}`);
  return data;
}
