import { apiClient } from './client';
import type { Provider, CreateProvider, UpdateProvider } from '@/types/api';

export async function listProviders(): Promise<Provider[]> {
  const { data } = await apiClient.get('/providers');
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
