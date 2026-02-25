import { apiClient } from './client';
import type { Vps, CreateVps, UpdateVps, VpsListQuery, VpsIpHistory, VpsIpHistoryQuery, ImportResult, PaginatedResponse } from '@/types/api';

export async function listVps(query?: VpsListQuery & { page?: number; per_page?: number }): Promise<PaginatedResponse<Vps>> {
  const params = new URLSearchParams();
  if (query) {
    for (const [key, value] of Object.entries(query)) {
      if (value !== undefined && value !== '') {
        params.set(key, String(value));
      }
    }
  }
  const { data } = await apiClient.get(`/vps?${params.toString()}`);
  return data;
}

export async function getVps(id: string): Promise<Vps> {
  const { data } = await apiClient.get(`/vps/${id}`);
  return data;
}

export async function createVps(input: CreateVps): Promise<Vps> {
  const { data } = await apiClient.post('/vps', input);
  return data;
}

export async function updateVps(id: string, input: UpdateVps): Promise<Vps> {
  const { data } = await apiClient.put(`/vps/${id}`, input);
  return data;
}

export async function deleteVps(id: string): Promise<void> {
  await apiClient.delete(`/vps/${id}`);
}

export async function retireVps(id: string): Promise<Vps> {
  const { data } = await apiClient.post(`/vps/${id}/retire`);
  return data;
}

export async function exportVps(format: 'csv' | 'json'): Promise<Blob> {
  const { data } = await apiClient.get(`/vps/export?format=${format}`, {
    responseType: 'blob',
  });
  return data;
}

export async function importVps(format: 'csv' | 'json', data: string): Promise<ImportResult> {
  const { data: result } = await apiClient.post('/vps/import', { format, data });
  return result;
}

export async function getVpsIpHistory(id: string, query?: VpsIpHistoryQuery): Promise<PaginatedResponse<VpsIpHistory>> {
  const params = new URLSearchParams();
  if (query) {
    for (const [key, value] of Object.entries(query)) {
      if (value !== undefined && value !== '') {
        params.set(key, String(value));
      }
    }
  }
  const { data } = await apiClient.get(`/vps/${id}/ip-history?${params.toString()}`);
  return data;
}

export async function allocatePorts(id: string, count: number): Promise<{ ports: number[] }> {
  const { data } = await apiClient.post(`/vps/${id}/allocate-ports`, { count });
  return data;
}
