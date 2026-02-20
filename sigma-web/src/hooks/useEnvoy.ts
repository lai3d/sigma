import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import * as api from '@/api/envoy';
import type {
  CreateEnvoyNode,
  UpdateEnvoyNode,
  EnvoyNodeListQuery,
  CreateEnvoyRoute,
  UpdateEnvoyRoute,
  EnvoyRouteListQuery,
} from '@/types/api';

// ─── Envoy Nodes ─────────────────────────────────────────

export function useEnvoyNodes(query?: EnvoyNodeListQuery) {
  return useQuery({
    queryKey: ['envoy-nodes', query],
    queryFn: () => api.listEnvoyNodes(query),
  });
}

export function useEnvoyNode(id: string) {
  return useQuery({
    queryKey: ['envoy-nodes', id],
    queryFn: () => api.getEnvoyNode(id),
    enabled: !!id,
  });
}

export function useCreateEnvoyNode() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateEnvoyNode) => api.createEnvoyNode(input),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['envoy-nodes'] }),
  });
}

export function useUpdateEnvoyNode() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateEnvoyNode }) =>
      api.updateEnvoyNode(id, data),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['envoy-nodes'] }),
  });
}

export function useDeleteEnvoyNode() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.deleteEnvoyNode(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['envoy-nodes'] }),
  });
}

// ─── Envoy Routes ────────────────────────────────────────

export function useEnvoyRoutes(query?: EnvoyRouteListQuery) {
  return useQuery({
    queryKey: ['envoy-routes', query],
    queryFn: () => api.listEnvoyRoutes(query),
  });
}

export function useEnvoyRoute(id: string) {
  return useQuery({
    queryKey: ['envoy-routes', id],
    queryFn: () => api.getEnvoyRoute(id),
    enabled: !!id,
  });
}

export function useCreateEnvoyRoute() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateEnvoyRoute) => api.createEnvoyRoute(input),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['envoy-routes'] });
      qc.invalidateQueries({ queryKey: ['envoy-nodes'] });
    },
  });
}

export function useUpdateEnvoyRoute() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateEnvoyRoute }) =>
      api.updateEnvoyRoute(id, data),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['envoy-routes'] });
      qc.invalidateQueries({ queryKey: ['envoy-nodes'] });
    },
  });
}

export function useDeleteEnvoyRoute() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.deleteEnvoyRoute(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['envoy-routes'] });
      qc.invalidateQueries({ queryKey: ['envoy-nodes'] });
    },
  });
}
