import { useState, useEffect } from 'react';
import { Key } from 'lucide-react';

export default function SettingsPage() {
  const [apiKey, setApiKey] = useState('');
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    setApiKey(localStorage.getItem('sigma_api_key') || '');
  }, []);

  function handleSave() {
    if (apiKey.trim()) {
      localStorage.setItem('sigma_api_key', apiKey.trim());
    } else {
      localStorage.removeItem('sigma_api_key');
    }
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  }

  return (
    <div>
      <h2 className="text-2xl font-bold text-gray-900">Settings</h2>

      <div className="mt-6 max-w-lg bg-white rounded-lg border p-5 space-y-4">
        <div className="flex items-center gap-2 text-gray-700">
          <Key size={18} />
          <h3 className="text-sm font-semibold">API Key</h3>
        </div>
        <p className="text-sm text-gray-500">
          Enter the API key to authenticate with the Sigma API. This is stored in your browser's local storage.
        </p>
        <input
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          className="input w-full"
          placeholder="Enter API key..."
        />
        <div className="flex items-center gap-3">
          <button
            onClick={handleSave}
            className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700"
          >
            Save
          </button>
          {saved && <span className="text-sm text-green-600">Saved!</span>}
        </div>
      </div>
    </div>
  );
}
