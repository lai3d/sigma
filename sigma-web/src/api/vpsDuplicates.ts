import { apiClient } from './client';
import type {
  DuplicateDetectionResponse,
  MergeVpsRequest,
  MergeVpsResponse,
} from '@/types/api';

export async function detectDuplicates(): Promise<DuplicateDetectionResponse> {
  const { data } = await apiClient.get('/vps/duplicates');
  return data;
}

export async function mergeVps(input: MergeVpsRequest): Promise<MergeVpsResponse> {
  const { data } = await apiClient.post('/vps/merge', input);
  return data;
}
