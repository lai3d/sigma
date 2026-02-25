import type { VpsPurposeRecord } from '@/types/api';

interface PurposeColorSet {
  bg: string;
  border: string;
  badge: string;
  minimap: string;
}

// All Tailwind class strings are written out in full so the build tool detects them.
const COLOR_PALETTE: Record<string, PurposeColorSet> = {
  blue: {
    bg: 'bg-blue-50',
    border: 'border-blue-300',
    badge: 'bg-blue-100 text-blue-700',
    minimap: '#93c5fd',
  },
  green: {
    bg: 'bg-green-50',
    border: 'border-green-300',
    badge: 'bg-green-100 text-green-700',
    minimap: '#86efac',
  },
  orange: {
    bg: 'bg-orange-50',
    border: 'border-orange-300',
    badge: 'bg-orange-100 text-orange-700',
    minimap: '#fdba74',
  },
  purple: {
    bg: 'bg-purple-50',
    border: 'border-purple-300',
    badge: 'bg-purple-100 text-purple-700',
    minimap: '#d8b4fe',
  },
  gray: {
    bg: 'bg-gray-50',
    border: 'border-gray-300',
    badge: 'bg-gray-100 text-gray-700',
    minimap: '#d1d5db',
  },
  cyan: {
    bg: 'bg-cyan-50',
    border: 'border-cyan-300',
    badge: 'bg-cyan-100 text-cyan-700',
    minimap: '#67e8f9',
  },
  red: {
    bg: 'bg-red-50',
    border: 'border-red-300',
    badge: 'bg-red-100 text-red-700',
    minimap: '#fca5a5',
  },
  yellow: {
    bg: 'bg-yellow-50',
    border: 'border-yellow-300',
    badge: 'bg-yellow-100 text-yellow-700',
    minimap: '#fde047',
  },
  indigo: {
    bg: 'bg-indigo-50',
    border: 'border-indigo-300',
    badge: 'bg-indigo-100 text-indigo-700',
    minimap: '#a5b4fc',
  },
  pink: {
    bg: 'bg-pink-50',
    border: 'border-pink-300',
    badge: 'bg-pink-100 text-pink-700',
    minimap: '#f9a8d4',
  },
  teal: {
    bg: 'bg-teal-50',
    border: 'border-teal-300',
    badge: 'bg-teal-100 text-teal-700',
    minimap: '#5eead4',
  },
  emerald: {
    bg: 'bg-emerald-50',
    border: 'border-emerald-300',
    badge: 'bg-emerald-100 text-emerald-700',
    minimap: '#6ee7b7',
  },
};

const DEFAULT_COLOR: PurposeColorSet = COLOR_PALETTE.gray;

export const AVAILABLE_COLORS = Object.keys(COLOR_PALETTE);

export function getPurposeColor(colorKey: string): PurposeColorSet {
  return COLOR_PALETTE[colorKey] ?? DEFAULT_COLOR;
}

/**
 * Build a map from purpose name → PurposeColorSet using DB records.
 */
export function buildPurposeColorMap(
  purposes: VpsPurposeRecord[],
): Record<string, PurposeColorSet> {
  const map: Record<string, PurposeColorSet> = {};
  for (const p of purposes) {
    map[p.name] = getPurposeColor(p.color);
  }
  return map;
}

/**
 * Build a map from purpose name → label using DB records.
 */
export function buildPurposeLabelMap(
  purposes: VpsPurposeRecord[],
): Record<string, string> {
  const map: Record<string, string> = {};
  for (const p of purposes) {
    map[p.name] = p.label;
  }
  return map;
}
