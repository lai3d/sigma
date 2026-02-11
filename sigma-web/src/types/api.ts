export type VpsStatus = 'provisioning' | 'active' | 'retiring' | 'retired';
export type VpsPurpose = 'vpn-exit' | 'vpn-relay' | 'vpn-entry' | 'monitor' | 'management' | '';

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
  provider_id: string;
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

export interface DashboardStats {
  total_vps: number;
  active_vps: number;
  total_providers: number;
  by_country: CountStat[];
  by_provider: CountStat[];
  by_status: CountStat[];
  expiring_soon: Vps[];
}
