import { apiClient } from './client';
import type {
  EnvoyNode,
  CreateEnvoyNode,
  UpdateEnvoyNode,
  EnvoyNodeListQuery,
  EnvoyRoute,
  CreateEnvoyRoute,
  UpdateEnvoyRoute,
  EnvoyRouteListQuery,
  BatchCreateEnvoyRoutes,
  PaginatedResponse,
  TopologyResponse,
} from '@/types/api';

// ─── Envoy Nodes ─────────────────────────────────────────

export async function listEnvoyNodes(query?: EnvoyNodeListQuery): Promise<PaginatedResponse<EnvoyNode>> {
  const params = new URLSearchParams();
  if (query) {
    for (const [key, value] of Object.entries(query)) {
      if (value !== undefined && value !== '') {
        params.set(key, String(value));
      }
    }
  }
  const { data } = await apiClient.get(`/envoy-nodes?${params.toString()}`);
  return data;
}

export async function getEnvoyNode(id: string): Promise<EnvoyNode> {
  const { data } = await apiClient.get(`/envoy-nodes/${id}`);
  return data;
}

export async function createEnvoyNode(input: CreateEnvoyNode): Promise<EnvoyNode> {
  const { data } = await apiClient.post('/envoy-nodes', input);
  return data;
}

export async function updateEnvoyNode(id: string, input: UpdateEnvoyNode): Promise<EnvoyNode> {
  const { data } = await apiClient.put(`/envoy-nodes/${id}`, input);
  return data;
}

export async function deleteEnvoyNode(id: string): Promise<void> {
  await apiClient.delete(`/envoy-nodes/${id}`);
}

// ─── Envoy Routes ────────────────────────────────────────

export async function listEnvoyRoutes(query?: EnvoyRouteListQuery): Promise<PaginatedResponse<EnvoyRoute>> {
  const params = new URLSearchParams();
  if (query) {
    for (const [key, value] of Object.entries(query)) {
      if (value !== undefined && value !== '') {
        params.set(key, String(value));
      }
    }
  }
  const { data } = await apiClient.get(`/envoy-routes?${params.toString()}`);
  return data;
}

export async function getEnvoyRoute(id: string): Promise<EnvoyRoute> {
  const { data } = await apiClient.get(`/envoy-routes/${id}`);
  return data;
}

export async function createEnvoyRoute(input: CreateEnvoyRoute): Promise<EnvoyRoute> {
  const { data } = await apiClient.post('/envoy-routes', input);
  return data;
}

export async function updateEnvoyRoute(id: string, input: UpdateEnvoyRoute): Promise<EnvoyRoute> {
  const { data } = await apiClient.put(`/envoy-routes/${id}`, input);
  return data;
}

export async function deleteEnvoyRoute(id: string): Promise<void> {
  await apiClient.delete(`/envoy-routes/${id}`);
}

export async function batchCreateEnvoyRoutes(input: BatchCreateEnvoyRoutes): Promise<EnvoyRoute[]> {
  const { data } = await apiClient.post('/envoy-routes/batch', input);
  return data;
}

// ─── Topology ────────────────────────────────────────────

export async function getEnvoyTopology(): Promise<TopologyResponse> {
  const { data } = await apiClient.get('/envoy-topology');
  return data;
}
