import { apiClient } from './client';

export async function getSettings(): Promise<Record<string, string>> {
  const { data } = await apiClient.get('/settings');
  return data;
}

export async function updateSettings(settings: Record<string, string>): Promise<Record<string, string>> {
  const { data } = await apiClient.put('/settings', settings);
  return data;
}
