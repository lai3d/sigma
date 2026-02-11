import { apiClient } from './client';
import type { User, CreateUser, UpdateUser, PaginatedResponse } from '@/types/api';

export async function listUsers(query?: { page?: number; per_page?: number; role?: string }): Promise<PaginatedResponse<User>> {
  const params = new URLSearchParams();
  if (query?.page) params.set('page', String(query.page));
  if (query?.per_page) params.set('per_page', String(query.per_page));
  if (query?.role) params.set('role', query.role);
  const { data } = await apiClient.get(`/users?${params.toString()}`);
  return data;
}

export async function getUser(id: string): Promise<User> {
  const { data } = await apiClient.get(`/users/${id}`);
  return data;
}

export async function createUser(input: CreateUser): Promise<User> {
  const { data } = await apiClient.post('/users', input);
  return data;
}

export async function updateUser(id: string, input: UpdateUser): Promise<User> {
  const { data } = await apiClient.put(`/users/${id}`, input);
  return data;
}

export async function deleteUser(id: string): Promise<void> {
  await apiClient.delete(`/users/${id}`);
}
