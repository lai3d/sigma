import { apiClient } from './client';
import type { LoginRequest, LoginResponse, ChangePasswordRequest, User } from '@/types/api';

export async function login(input: LoginRequest): Promise<LoginResponse> {
  const { data } = await apiClient.post('/auth/login', input);
  return data;
}

export async function refresh(): Promise<LoginResponse> {
  const { data } = await apiClient.post('/auth/refresh');
  return data;
}

export async function changePassword(input: ChangePasswordRequest): Promise<User> {
  const { data } = await apiClient.post('/auth/change-password', input);
  return data;
}

export async function getMe(): Promise<User> {
  const { data } = await apiClient.get('/auth/me');
  return data;
}
