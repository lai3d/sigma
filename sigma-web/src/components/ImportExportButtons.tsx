import { useState, useRef, useEffect } from 'react';
import { Download, Upload, ChevronDown } from 'lucide-react';
import type { ImportResult } from '@/types/api';

interface Props {
  onExport: (format: 'csv' | 'json') => Promise<Blob>;
  onImport: (format: 'csv' | 'json', data: string) => Promise<ImportResult>;
  entityName: string;
}

export default function ImportExportButtons({ onExport, onImport, entityName }: Props) {
  const [exportOpen, setExportOpen] = useState(false);
  const [importing, setImporting] = useState(false);
  const [result, setResult] = useState<ImportResult | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const dialogRef = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    if (result) dialogRef.current?.showModal();
    else dialogRef.current?.close();
  }, [result]);

  useEffect(() => {
    if (!exportOpen) return;
    const handler = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setExportOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [exportOpen]);

  const handleExport = async (format: 'csv' | 'json') => {
    setExportOpen(false);
    const blob = await onExport(format);
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${entityName}.${format}`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  };

  const handleImport = () => {
    fileRef.current?.click();
  };

  const handleFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const ext = file.name.split('.').pop()?.toLowerCase();
    const format: 'csv' | 'json' = ext === 'csv' ? 'csv' : 'json';
    const data = await file.text();

    setImporting(true);
    try {
      const res = await onImport(format, data);
      setResult(res);
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : 'Import failed';
      setResult({ imported: 0, errors: [message] });
    } finally {
      setImporting(false);
      if (fileRef.current) fileRef.current.value = '';
    }
  };

  return (
    <>
      <div className="flex items-center gap-2">
        {/* Export dropdown */}
        <div className="relative" ref={dropdownRef}>
          <button
            onClick={() => setExportOpen(!exportOpen)}
            className="inline-flex items-center gap-1.5 px-3 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-md hover:bg-gray-50"
          >
            <Download size={15} />
            Export
            <ChevronDown size={14} />
          </button>
          {exportOpen && (
            <div className="absolute right-0 z-10 mt-1 w-36 bg-white border border-gray-200 rounded-md shadow-lg">
              <button
                onClick={() => handleExport('csv')}
                className="block w-full text-left px-4 py-2 text-sm text-gray-700 hover:bg-gray-100"
              >
                Export CSV
              </button>
              <button
                onClick={() => handleExport('json')}
                className="block w-full text-left px-4 py-2 text-sm text-gray-700 hover:bg-gray-100"
              >
                Export JSON
              </button>
            </div>
          )}
        </div>

        {/* Import button */}
        <button
          onClick={handleImport}
          disabled={importing}
          className="inline-flex items-center gap-1.5 px-3 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-md hover:bg-gray-50 disabled:opacity-50"
        >
          <Upload size={15} />
          {importing ? 'Importing...' : 'Import'}
        </button>
        <input
          ref={fileRef}
          type="file"
          accept=".csv,.json"
          onChange={handleFile}
          className="hidden"
        />
      </div>

      {/* Result dialog */}
      <dialog
        ref={dialogRef}
        onClose={() => setResult(null)}
        className="rounded-lg shadow-xl p-0 backdrop:bg-black/40"
      >
        {result && (
          <div className="p-6 min-w-80 max-w-lg">
            <h3 className="text-lg font-semibold text-gray-900">Import Result</h3>
            <p className="mt-2 text-sm text-gray-600">
              Successfully imported <span className="font-semibold text-green-700">{result.imported}</span> record{result.imported !== 1 ? 's' : ''}.
            </p>
            {result.errors.length > 0 && (
              <div className="mt-3">
                <p className="text-sm font-medium text-red-700">
                  {result.errors.length} error{result.errors.length !== 1 ? 's' : ''}:
                </p>
                <ul className="mt-1 max-h-48 overflow-y-auto text-xs text-red-600 space-y-1">
                  {result.errors.map((err, i) => (
                    <li key={i} className="bg-red-50 rounded px-2 py-1">{err}</li>
                  ))}
                </ul>
              </div>
            )}
            <div className="mt-5 flex justify-end">
              <button
                onClick={() => setResult(null)}
                className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700"
              >
                Close
              </button>
            </div>
          </div>
        )}
      </dialog>
    </>
  );
}
