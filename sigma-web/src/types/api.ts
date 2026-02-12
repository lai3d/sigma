export type VpsStatus = 'provisioning' | 'active' | 'retiring' | 'retired';
export type VpsPurpose = 'vpn-exit' | 'vpn-relay' | 'vpn-entry' | 'monitor' | 'management' | '';

// ─── Auth & Users ────────────────────────────────────────

export type UserRole = 'admin' | 'operator' | 'readonly';

export interface User {
  id: string;
  email: string;
  name: string;
  role: UserRole;
  force_password_change: boolean;
  totp_enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface LoginRequest {
  email: string;
  password: string;
}

export interface LoginResponse {
  token: string;
  user: User;
}

export interface ChangePasswordRequest {
  current_password: string;
  new_password: string;
}

export interface CreateUser {
  email: string;
  password: string;
  name?: string;
  role?: UserRole;
}

export interface UpdateUser {
  email?: string;
  name?: string;
  role?: UserRole;
  password?: string;
  force_password_change?: boolean;
  totp_enabled?: boolean;
}

// ─── TOTP Types ────────────────────────────────────────────

export interface TotpChallengeResponse {
  requires_totp: boolean;
  totp_token: string;
}

export interface TotpSetupResponse {
  secret: string;
  otpauth_url: string;
  qr_code: string;
}

export interface TotpLoginRequest {
  totp_token: string;
  code: string;
}

export interface TotpVerifyRequest {
  code: string;
}

export interface TotpDisableRequest {
  code: string;
}

export type LoginResult = LoginResponse | TotpChallengeResponse;

export function isTotpChallenge(res: LoginResult): res is TotpChallengeResponse {
  return 'requires_totp' in res && (res as TotpChallengeResponse).requires_totp === true;
}

export interface Provider {
  id: string;
  name: string;
  country: string;
  website: string;
  panel_url: string;
  api_supported: boolean;
  rating: number | null;
  notes: string;
  created_at: string;
  updated_at: string;
}

export interface CreateProvider {
  name: string;
  country?: string;
  website?: string;
  panel_url?: string;
  api_supported?: boolean;
  rating?: number | null;
  notes?: string;
}

export interface UpdateProvider {
  name?: string;
  country?: string;
  website?: string;
  panel_url?: string;
  api_supported?: boolean;
  rating?: number | null;
  notes?: string;
}

export interface IpEntry {
  ip: string;
  label: string;
}

export interface Vps {
  id: string;
  hostname: string;
  alias: string;
  provider_id: string;
  ip_addresses: IpEntry[];
  ssh_port: number;
  country: string;
  city: string;
  dc_name: string;
  cpu_cores: number | null;
  ram_mb: number | null;
  disk_gb: number | null;
  bandwidth_tb: string | null;
  cost_monthly: string | null;
  currency: string;
  status: VpsStatus;
  purchase_date: string | null;
  expire_date: string | null;
  purpose: string;
  vpn_protocol: string;
  tags: string[];
  monitoring_enabled: boolean;
  node_exporter_port: number;
  extra: Record<string, unknown>;
  notes: string;
  created_at: string;
  updated_at: string;
}

export interface CreateVps {
  hostname: string;
  alias?: string;
  provider_id?: string;
  ip_addresses?: IpEntry[];
  ssh_port?: number;
  country?: string;
  city?: string;
  dc_name?: string;
  cpu_cores?: number | null;
  ram_mb?: number | null;
  disk_gb?: number | null;
  bandwidth_tb?: number | null;
  cost_monthly?: number | null;
  currency?: string;
  status?: string;
  purchase_date?: string | null;
  expire_date?: string | null;
  purpose?: string;
  vpn_protocol?: string;
  tags?: string[];
  monitoring_enabled?: boolean;
  node_exporter_port?: number;
  extra?: Record<string, unknown>;
  notes?: string;
}

export type UpdateVps = Partial<CreateVps>;

export interface VpsListQuery {
  status?: string;
  country?: string;
  provider_id?: string;
  purpose?: string;
  tag?: string;
  expiring_within_days?: number;
}

export interface CountStat {
  label: string | null;
  count: number | null;
}

export interface PaginatedResponse<T> {
  data: T[];
  total: number;
  page: number;
  per_page: number;
}

export interface ImportResult {
  imported: number;
  errors: string[];
}

// ─── Audit Logs ────────────────────────────────────────────

export interface AuditLog {
  id: string;
  user_id: string | null;
  user_email: string;
  action: string;
  resource: string;
  resource_id: string | null;
  details: Record<string, unknown>;
  created_at: string;
}

export interface AuditLogQuery {
  resource?: string;
  resource_id?: string;
  user_id?: string;
  action?: string;
  since?: string;
  until?: string;
  page?: number;
  per_page?: number;
}

// ─── Tickets ────────────────────────────────────────────

export type TicketStatus = 'open' | 'in-progress' | 'resolved' | 'closed';
export type TicketPriority = 'low' | 'medium' | 'high' | 'critical';

export interface Ticket {
  id: string;
  title: string;
  description: string;
  status: TicketStatus;
  priority: TicketPriority;
  vps_id: string | null;
  provider_id: string | null;
  created_by: string;
  assigned_to: string | null;
  created_at: string;
  updated_at: string;
}

export interface TicketComment {
  id: string;
  ticket_id: string;
  user_id: string;
  user_email: string;
  body: string;
  created_at: string;
}

export interface CreateTicket {
  title: string;
  description?: string;
  priority?: TicketPriority;
  vps_id?: string | null;
  provider_id?: string | null;
  assigned_to?: string | null;
}

export interface UpdateTicket {
  title?: string;
  description?: string;
  status?: TicketStatus;
  priority?: TicketPriority;
  vps_id?: string | null;
  provider_id?: string | null;
  assigned_to?: string | null;
}

export interface TicketListQuery {
  status?: string;
  priority?: string;
  assigned_to?: string;
  vps_id?: string;
  page?: number;
  per_page?: number;
}

export interface DashboardStats {
  total_vps: number;
  active_vps: number;
  total_providers: number;
  by_country: CountStat[];
  by_provider: CountStat[];
  by_status: CountStat[];
  expiring_soon: Vps[];
}
