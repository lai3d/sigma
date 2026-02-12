import { formatDate, daysUntil, formatIp, ipLabelColor, ipLabelShort, cn } from '@/lib/utils';

describe('formatDate', () => {
  it('returns dash for null', () => {
    expect(formatDate(null)).toBe('-');
  });

  it('formats a valid date string', () => {
    const result = formatDate('2025-06-15');
    expect(result).toBeTruthy();
    expect(result).not.toBe('-');
  });
});

describe('daysUntil', () => {
  it('returns null for null input', () => {
    expect(daysUntil(null)).toBeNull();
  });

  it('returns positive number for future date', () => {
    const future = new Date();
    future.setDate(future.getDate() + 10);
    const result = daysUntil(future.toISOString());
    expect(result).toBeGreaterThanOrEqual(9);
    expect(result).toBeLessThanOrEqual(11);
  });

  it('returns negative number for past date', () => {
    const past = new Date();
    past.setDate(past.getDate() - 5);
    const result = daysUntil(past.toISOString());
    expect(result).toBeLessThan(0);
  });
});

describe('formatIp', () => {
  it('strips /32 suffix', () => {
    expect(formatIp('192.168.1.1/32')).toBe('192.168.1.1');
  });

  it('strips /128 suffix', () => {
    expect(formatIp('::1/128')).toBe('::1');
  });

  it('preserves other CIDR notations', () => {
    expect(formatIp('10.0.0.0/24')).toBe('10.0.0.0/24');
  });

  it('preserves plain IPs', () => {
    expect(formatIp('1.2.3.4')).toBe('1.2.3.4');
  });
});

describe('ipLabelColor', () => {
  it('returns correct color for china-telecom', () => {
    expect(ipLabelColor('china-telecom')).toContain('red');
  });

  it('returns correct color for overseas', () => {
    expect(ipLabelColor('overseas')).toContain('blue');
  });

  it('returns fallback for unknown label', () => {
    expect(ipLabelColor('unknown-label')).toContain('gray');
  });
});

describe('ipLabelShort', () => {
  it('returns CT for china-telecom', () => {
    expect(ipLabelShort('china-telecom')).toBe('CT');
  });

  it('returns CU for china-unicom', () => {
    expect(ipLabelShort('china-unicom')).toBe('CU');
  });

  it('returns CM for china-mobile', () => {
    expect(ipLabelShort('china-mobile')).toBe('CM');
  });

  it('returns the label itself for unknown', () => {
    expect(ipLabelShort('custom')).toBe('custom');
  });
});

describe('cn', () => {
  it('merges class names', () => {
    const result = cn('px-2', 'py-1');
    expect(result).toContain('px-2');
    expect(result).toContain('py-1');
  });

  it('handles conditional classes', () => {
    const result = cn('base', false && 'hidden', 'extra');
    expect(result).toContain('base');
    expect(result).toContain('extra');
    expect(result).not.toContain('hidden');
  });
});
