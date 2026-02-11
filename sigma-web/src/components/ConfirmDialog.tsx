import { useRef, useEffect } from 'react';

interface Props {
  open: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  variant?: 'danger' | 'default';
  onConfirm: () => void;
  onCancel: () => void;
}

export default function ConfirmDialog({
  open,
  title,
  message,
  confirmLabel = 'Confirm',
  variant = 'default',
  onConfirm,
  onCancel,
}: Props) {
  const dialogRef = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    if (open) dialogRef.current?.showModal();
    else dialogRef.current?.close();
  }, [open]);

  return (
    <dialog
      ref={dialogRef}
      onClose={onCancel}
      className="rounded-lg shadow-xl p-0 backdrop:bg-black/40"
    >
      <div className="p-6 min-w-80">
        <h3 className="text-lg font-semibold text-gray-900">{title}</h3>
        <p className="mt-2 text-sm text-gray-600">{message}</p>
        <div className="mt-5 flex justify-end gap-3">
          <button
            onClick={onCancel}
            className="px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-md hover:bg-gray-50"
          >
            Cancel
          </button>
          <button
            onClick={onConfirm}
            className={
              variant === 'danger'
                ? 'px-4 py-2 text-sm font-medium text-white bg-red-600 rounded-md hover:bg-red-700'
                : 'px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700'
            }
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </dialog>
  );
}
