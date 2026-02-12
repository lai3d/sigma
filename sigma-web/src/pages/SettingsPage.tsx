import { useState, useEffect } from 'react';
import { Key, User, Shield } from 'lucide-react';
import { Link } from 'react-router-dom';
import { useAuth } from '@/contexts/AuthContext';
import * as authApi from '@/api/auth';
import type { TotpSetupResponse } from '@/types/api';

export default function SettingsPage() {
  const { user, updateUser } = useAuth();
  const [apiKey, setApiKey] = useState('');
  const [saved, setSaved] = useState(false);

  // TOTP state
  const [totpSetup, setTotpSetup] = useState<TotpSetupResponse | null>(null);
  const [totpCode, setTotpCode] = useState('');
  const [disableCode, setDisableCode] = useState('');
  const [totpLoading, setTotpLoading] = useState(false);
  const [totpError, setTotpError] = useState('');
  const [totpSuccess, setTotpSuccess] = useState('');

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

  async function handleTotpSetup() {
    setTotpError('');
    setTotpSuccess('');
    setTotpLoading(true);
    try {
      const setup = await authApi.totpSetup();
      setTotpSetup(setup);
    } catch {
      setTotpError('Failed to start TOTP setup');
    } finally {
      setTotpLoading(false);
    }
  }

  async function handleTotpVerify(e: React.FormEvent) {
    e.preventDefault();
    setTotpError('');
    setTotpLoading(true);
    try {
      await authApi.totpVerify({ code: totpCode });
      setTotpSetup(null);
      setTotpCode('');
      setTotpSuccess('Two-factor authentication enabled');
      if (user) updateUser({ ...user, totp_enabled: true });
      setTimeout(() => setTotpSuccess(''), 3000);
    } catch {
      setTotpError('Invalid code. Please try again.');
    } finally {
      setTotpLoading(false);
    }
  }

  async function handleTotpDisable(e: React.FormEvent) {
    e.preventDefault();
    setTotpError('');
    setTotpLoading(true);
    try {
      await authApi.totpDisable({ code: disableCode });
      setDisableCode('');
      setTotpSuccess('Two-factor authentication disabled');
      if (user) updateUser({ ...user, totp_enabled: false });
      setTimeout(() => setTotpSuccess(''), 3000);
    } catch {
      setTotpError('Invalid code. Please try again.');
    } finally {
      setTotpLoading(false);
    }
  }

  return (
    <div>
      <h2 className="text-2xl font-bold text-gray-900">Settings</h2>

      {/* Profile Card */}
      {user && (
        <div className="mt-6 max-w-lg bg-white rounded-lg border p-5 space-y-3">
          <div className="flex items-center gap-2 text-gray-700">
            <User size={18} />
            <h3 className="text-sm font-semibold">Profile</h3>
          </div>
          <div className="grid grid-cols-2 gap-3 text-sm">
            <div>
              <span className="text-gray-500">Name</span>
              <p className="font-medium text-gray-900">{user.name || '-'}</p>
            </div>
            <div>
              <span className="text-gray-500">Email</span>
              <p className="font-medium text-gray-900">{user.email}</p>
            </div>
            <div>
              <span className="text-gray-500">Role</span>
              <p className="font-medium text-gray-900 capitalize">{user.role}</p>
            </div>
          </div>
          <Link
            to="/change-password"
            className="inline-block text-sm text-blue-600 hover:underline"
          >
            Change password
          </Link>
        </div>
      )}

      {/* Two-Factor Authentication Card */}
      {user && (
        <div className="mt-4 max-w-lg bg-white rounded-lg border p-5 space-y-4">
          <div className="flex items-center gap-2 text-gray-700">
            <Shield size={18} />
            <h3 className="text-sm font-semibold">Two-Factor Authentication</h3>
          </div>

          {totpError && (
            <div className="p-3 text-sm text-red-700 bg-red-50 rounded-md">{totpError}</div>
          )}
          {totpSuccess && (
            <div className="p-3 text-sm text-green-700 bg-green-50 rounded-md">{totpSuccess}</div>
          )}

          {/* State: Disabled, no setup in progress */}
          {!user.totp_enabled && !totpSetup && (
            <>
              <p className="text-sm text-gray-500">
                Add an extra layer of security by enabling TOTP-based two-factor authentication with an app like Google Authenticator or Authy.
              </p>
              <button
                onClick={handleTotpSetup}
                disabled={totpLoading}
                className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50"
              >
                {totpLoading ? 'Setting up...' : 'Enable 2FA'}
              </button>
            </>
          )}

          {/* State: Setup in progress */}
          {!user.totp_enabled && totpSetup && (
            <form onSubmit={handleTotpVerify} className="space-y-4">
              <p className="text-sm text-gray-500">
                Scan the QR code with your authenticator app, then enter the 6-digit code to verify.
              </p>
              <div className="flex justify-center">
                <img
                  src={`data:image/png;base64,${totpSetup.qr_code}`}
                  alt="TOTP QR Code"
                  className="w-48 h-48"
                />
              </div>
              <div>
                <p className="text-xs text-gray-500 mb-1">Manual entry key:</p>
                <code className="block text-xs bg-gray-100 p-2 rounded font-mono break-all select-all">
                  {totpSetup.secret}
                </code>
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700">Verification code</label>
                <input
                  type="text"
                  inputMode="numeric"
                  pattern="[0-9]{6}"
                  maxLength={6}
                  value={totpCode}
                  onChange={(e) => setTotpCode(e.target.value.replace(/\D/g, ''))}
                  className="input w-full mt-1 text-center tracking-widest"
                  placeholder="000000"
                  required
                />
              </div>
              <div className="flex gap-2">
                <button
                  type="submit"
                  disabled={totpLoading || totpCode.length !== 6}
                  className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50"
                >
                  {totpLoading ? 'Verifying...' : 'Verify & Enable'}
                </button>
                <button
                  type="button"
                  onClick={() => { setTotpSetup(null); setTotpCode(''); setTotpError(''); }}
                  className="px-4 py-2 text-sm font-medium text-gray-700 bg-gray-100 rounded-md hover:bg-gray-200"
                >
                  Cancel
                </button>
              </div>
            </form>
          )}

          {/* State: Enabled */}
          {user.totp_enabled && (
            <form onSubmit={handleTotpDisable} className="space-y-4">
              <div className="flex items-center gap-2">
                <span className="inline-block w-2 h-2 bg-green-500 rounded-full"></span>
                <span className="text-sm font-medium text-green-700">Two-factor authentication is enabled</span>
              </div>
              <p className="text-sm text-gray-500">
                To disable 2FA, enter a current code from your authenticator app.
              </p>
              <div>
                <label className="block text-sm font-medium text-gray-700">TOTP Code</label>
                <input
                  type="text"
                  inputMode="numeric"
                  pattern="[0-9]{6}"
                  maxLength={6}
                  value={disableCode}
                  onChange={(e) => setDisableCode(e.target.value.replace(/\D/g, ''))}
                  className="input w-full mt-1 text-center tracking-widest"
                  placeholder="000000"
                  required
                />
              </div>
              <button
                type="submit"
                disabled={totpLoading || disableCode.length !== 6}
                className="px-4 py-2 text-sm font-medium text-white bg-red-600 rounded-md hover:bg-red-700 disabled:opacity-50"
              >
                {totpLoading ? 'Disabling...' : 'Disable 2FA'}
              </button>
            </form>
          )}
        </div>
      )}

      {/* API Key Card */}
      <div className="mt-4 max-w-lg bg-white rounded-lg border p-5 space-y-4">
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
