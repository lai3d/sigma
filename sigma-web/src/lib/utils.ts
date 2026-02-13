import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatDate(date: string | null): string {
  if (!date) return '-';
  return new Date(date).toLocaleDateString();
}

export function daysUntil(date: string | null): number | null {
  if (!date) return null;
  const now = new Date();
  const target = new Date(date);
  return Math.ceil((target.getTime() - now.getTime()) / (1000 * 60 * 60 * 24));
}

export function formatIp(ip: string): string {
  return ip.replace(/\/(32|128)$/, '');
}

const LABEL_COLORS: Record<string, string> = {
  'china-telecom': 'bg-red-50 text-red-700',
  'china-unicom': 'bg-orange-50 text-orange-700',
  'china-mobile': 'bg-green-50 text-green-700',
  'china-cernet': 'bg-purple-50 text-purple-700',
  'overseas': 'bg-blue-50 text-blue-700',
  'internal': 'bg-gray-100 text-gray-600',
  'anycast': 'bg-cyan-50 text-cyan-700',
};

const LABEL_SHORT: Record<string, string> = {
  'china-telecom': 'CT',
  'china-unicom': 'CU',
  'china-mobile': 'CM',
  'china-cernet': 'EDU',
  'overseas': 'OS',
  'internal': 'LAN',
  'anycast': 'AC',
};

export function ipLabelColor(label: string): string {
  return LABEL_COLORS[label] || 'bg-gray-50 text-gray-500';
}

export function ipLabelShort(label: string): string {
  return LABEL_SHORT[label] || label;
}

export function timeAgo(isoString: string): string {
  const diff = Date.now() - new Date(isoString).getTime();
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

/** Returns [displayText, cssClasses] for a tag. Tags with `p:` prefix are project tags. */
export function tagStyle(tag: string): { label: string; className: string } {
  if (tag.startsWith('p:')) {
    return { label: tag.slice(2), className: 'bg-purple-50 text-purple-700' };
  }
  return { label: tag, className: 'bg-blue-50 text-blue-700' };
}

export function formatUptime(seconds: number): string {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  if (days > 0) return `${days}d ${hours}h`;
  const minutes = Math.floor((seconds % 3600) / 60);
  return `${hours}h ${minutes}m`;
}
