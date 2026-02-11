import { apiClient } from './client';
import type { Vps, CreateVps, UpdateVps, VpsListQuery } from '@/types/api';

export async function listVps(query?: VpsListQuery): Promise<Vps[]> {
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
