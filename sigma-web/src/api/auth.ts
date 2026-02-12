import { apiClient } from './client';
import type {
  LoginRequest, LoginResponse, LoginResult, ChangePasswordRequest, User,
  TotpLoginRequest, TotpSetupResponse, TotpVerifyRequest, TotpDisableRequest,
} from '@/types/api';

export async function login(input: LoginRequest): Promise<LoginResult> {
  const { data } = await apiClient.post('/auth/login', input);
  return data;
}

export async function loginTotp(input: TotpLoginRequest): Promise<LoginResponse> {
  const { data } = await apiClient.post('/auth/login/totp', input);
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

export async function totpSetup(): Promise<TotpSetupResponse> {
  const { data } = await apiClient.post('/auth/totp/setup');
  return data;
}

export async function totpVerify(input: TotpVerifyRequest): Promise<void> {
  await apiClient.post('/auth/totp/verify', input);
}

export async function totpDisable(input: TotpDisableRequest): Promise<void> {
  await apiClient.post('/auth/totp/disable', input);
}
