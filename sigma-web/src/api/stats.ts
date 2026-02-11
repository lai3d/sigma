import { apiClient } from './client';
import type { DashboardStats } from '@/types/api';

export async function getStats(): Promise<DashboardStats> {
  const { data } = await apiClient.get('/stats');
  return data;
}
