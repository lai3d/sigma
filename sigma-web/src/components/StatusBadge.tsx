import { cn } from '@/lib/utils';

const statusColors: Record<string, string> = {
  provisioning: 'bg-yellow-100 text-yellow-800',
  active: 'bg-green-100 text-green-800',
  retiring: 'bg-orange-100 text-orange-800',
  retired: 'bg-gray-100 text-gray-600',
};

export default function StatusBadge({ status }: { status: string }) {
  return (
    <span
      className={cn(
        'inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium',
        statusColors[status] || 'bg-gray-100 text-gray-800'
      )}
    >
      {status}
    </span>
  );
}
