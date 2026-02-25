import { useState, useEffect } from 'react';
import { Key, User, Shield, BarChart3, Tags, Pencil, Trash2, Plus } from 'lucide-react';
import { Link } from 'react-router-dom';
import { useAuth } from '@/contexts/AuthContext';
import * as authApi from '@/api/auth';
import { useVpsPurposes, useCreateVpsPurpose, useUpdateVpsPurpose, useDeleteVpsPurpose } from '@/hooks/useVpsPurposes';
import { AVAILABLE_COLORS, getPurposeColor } from '@/lib/purposeColors';
import type { TotpSetupResponse, VpsPurposeRecord } from '@/types/api';

export default function SettingsPage() {
  const { user, updateUser } = useAuth();
  const [apiKey, setApiKey] = useState('');
  const [saved, setSaved] = useState(false);

  // Grafana
  const [grafanaUrl, setGrafanaUrl] = useState('');
  const [grafanaSaved, setGrafanaSaved] = useState(false);

  // TOTP state
  const [totpSetup, setTotpSetup] = useState<TotpSetupResponse | null>(null);
  const [totpCode, setTotpCode] = useState('');
  const [disableCode, setDisableCode] = useState('');
  const [totpLoading, setTotpLoading] = useState(false);
  const [totpError, setTotpError] = useState('');
  const [totpSuccess, setTotpSuccess] = useState('');

  // VPS Purposes state
  const canMutate = user?.role === 'admin' || user?.role === 'operator';
  const { data: purposesResult } = useVpsPurposes({ per_page: 100 });
  const purposes = purposesResult?.data ?? [];
  const createPurpose = useCreateVpsPurpose();
  const updatePurpose = useUpdateVpsPurpose();
  const deletePurpose = useDeleteVpsPurpose();
  const [editingPurpose, setEditingPurpose] = useState<string | null>(null);
  const [addingPurpose, setAddingPurpose] = useState(false);
  const [purposeForm, setPurposeForm] = useState({ name: '', label: '', color: 'gray', sort_order: 0 });
  const [purposeError, setPurposeError] = useState('');
  const [confirmDeletePurpose, setConfirmDeletePurpose] = useState<VpsPurposeRecord | null>(null);

  useEffect(() => {
    setApiKey(localStorage.getItem('sigma_api_key') || '');
    setGrafanaUrl(localStorage.getItem('sigma_grafana_url') || '');
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

      {/* VPS Purposes Card */}
      {canMutate && (
        <div className="mt-4 max-w-2xl bg-white rounded-lg border p-5 space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 text-gray-700">
              <Tags size={18} />
              <h3 className="text-sm font-semibold">VPS Purposes</h3>
            </div>
            {!addingPurpose && (
              <button
                onClick={() => {
                  setPurposeForm({ name: '', label: '', color: 'gray', sort_order: purposes.length + 1 });
                  setAddingPurpose(true);
                  setEditingPurpose(null);
                  setPurposeError('');
                }}
                className="flex items-center gap-1 px-3 py-1.5 text-xs font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700"
              >
                <Plus size={14} /> Add Purpose
              </button>
            )}
          </div>

          {purposeError && (
            <div className="p-3 text-sm text-red-700 bg-red-50 rounded-md">{purposeError}</div>
          )}

          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-gray-500 border-b">
                <th className="pb-2 font-medium">Name</th>
                <th className="pb-2 font-medium">Label</th>
                <th className="pb-2 font-medium">Color</th>
                <th className="pb-2 font-medium">Order</th>
                <th className="pb-2 font-medium w-20"></th>
              </tr>
            </thead>
            <tbody>
              {purposes.map((p) => (
                editingPurpose === p.id ? (
                  <tr key={p.id} className="border-b">
                    <td className="py-2 pr-2">
                      <input
                        value={purposeForm.name}
                        onChange={(e) => setPurposeForm({ ...purposeForm, name: e.target.value })}
                        className="input w-full text-sm"
                      />
                    </td>
                    <td className="py-2 pr-2">
                      <input
                        value={purposeForm.label}
                        onChange={(e) => setPurposeForm({ ...purposeForm, label: e.target.value })}
                        className="input w-full text-sm"
                      />
                    </td>
                    <td className="py-2 pr-2">
                      <select
                        value={purposeForm.color}
                        onChange={(e) => setPurposeForm({ ...purposeForm, color: e.target.value })}
                        className="input text-sm"
                      >
                        {AVAILABLE_COLORS.map((c) => (
                          <option key={c} value={c}>{c}</option>
                        ))}
                      </select>
                    </td>
                    <td className="py-2 pr-2">
                      <input
                        type="number"
                        value={purposeForm.sort_order}
                        onChange={(e) => setPurposeForm({ ...purposeForm, sort_order: Number(e.target.value) })}
                        className="input w-16 text-sm"
                      />
                    </td>
                    <td className="py-2">
                      <div className="flex gap-1">
                        <button
                          onClick={async () => {
                            setPurposeError('');
                            try {
                              await updatePurpose.mutateAsync({ id: p.id, data: purposeForm });
                              setEditingPurpose(null);
                            } catch (err: unknown) {
                              const msg = err instanceof Error ? err.message : 'Failed to update';
                              setPurposeError(msg);
                            }
                          }}
                          className="px-2 py-1 text-xs font-medium text-white bg-blue-600 rounded hover:bg-blue-700"
                        >
                          Save
                        </button>
                        <button
                          onClick={() => setEditingPurpose(null)}
                          className="px-2 py-1 text-xs font-medium text-gray-600 bg-gray-100 rounded hover:bg-gray-200"
                        >
                          Cancel
                        </button>
                      </div>
                    </td>
                  </tr>
                ) : (
                  <tr key={p.id} className="border-b">
                    <td className="py-2 pr-2 font-mono text-xs">{p.name}</td>
                    <td className="py-2 pr-2">{p.label}</td>
                    <td className="py-2 pr-2">
                      <span className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded text-xs font-medium ${getPurposeColor(p.color).badge}`}>
                        <span className={`w-2 h-2 rounded-full ${getPurposeColor(p.color).bg} border ${getPurposeColor(p.color).border}`} />
                        {p.color}
                      </span>
                    </td>
                    <td className="py-2 pr-2 text-gray-500">{p.sort_order}</td>
                    <td className="py-2">
                      <div className="flex gap-1">
                        <button
                          onClick={() => {
                            setPurposeForm({ name: p.name, label: p.label, color: p.color, sort_order: p.sort_order });
                            setEditingPurpose(p.id);
                            setAddingPurpose(false);
                            setPurposeError('');
                          }}
                          className="p-1 text-gray-400 hover:text-blue-600"
                          title="Edit"
                        >
                          <Pencil size={14} />
                        </button>
                        <button
                          onClick={() => setConfirmDeletePurpose(p)}
                          className="p-1 text-gray-400 hover:text-red-600"
                          title="Delete"
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    </td>
                  </tr>
                )
              ))}
              {addingPurpose && (
                <tr className="border-b">
                  <td className="py-2 pr-2">
                    <input
                      value={purposeForm.name}
                      onChange={(e) => setPurposeForm({ ...purposeForm, name: e.target.value })}
                      className="input w-full text-sm"
                      placeholder="e.g. cdn-node"
                    />
                  </td>
                  <td className="py-2 pr-2">
                    <input
                      value={purposeForm.label}
                      onChange={(e) => setPurposeForm({ ...purposeForm, label: e.target.value })}
                      className="input w-full text-sm"
                      placeholder="e.g. CDN Node"
                    />
                  </td>
                  <td className="py-2 pr-2">
                    <select
                      value={purposeForm.color}
                      onChange={(e) => setPurposeForm({ ...purposeForm, color: e.target.value })}
                      className="input text-sm"
                    >
                      {AVAILABLE_COLORS.map((c) => (
                        <option key={c} value={c}>{c}</option>
                      ))}
                    </select>
                  </td>
                  <td className="py-2 pr-2">
                    <input
                      type="number"
                      value={purposeForm.sort_order}
                      onChange={(e) => setPurposeForm({ ...purposeForm, sort_order: Number(e.target.value) })}
                      className="input w-16 text-sm"
                    />
                  </td>
                  <td className="py-2">
                    <div className="flex gap-1">
                      <button
                        onClick={async () => {
                          setPurposeError('');
                          if (!purposeForm.name.trim() || !purposeForm.label.trim()) {
                            setPurposeError('Name and label are required');
                            return;
                          }
                          try {
                            await createPurpose.mutateAsync(purposeForm);
                            setAddingPurpose(false);
                          } catch (err: unknown) {
                            const msg = err instanceof Error ? err.message : 'Failed to create';
                            setPurposeError(msg);
                          }
                        }}
                        className="px-2 py-1 text-xs font-medium text-white bg-blue-600 rounded hover:bg-blue-700"
                      >
                        Add
                      </button>
                      <button
                        onClick={() => setAddingPurpose(false)}
                        className="px-2 py-1 text-xs font-medium text-gray-600 bg-gray-100 rounded hover:bg-gray-200"
                      >
                        Cancel
                      </button>
                    </div>
                  </td>
                </tr>
              )}
            </tbody>
          </table>

          {/* Delete confirmation */}
          {confirmDeletePurpose && (
            <div className="p-3 bg-red-50 border border-red-200 rounded-md">
              <p className="text-sm text-red-700">
                Delete purpose <strong>{confirmDeletePurpose.label}</strong> ({confirmDeletePurpose.name})?
                This will fail if any VPS instances use it.
              </p>
              <div className="flex gap-2 mt-2">
                <button
                  onClick={async () => {
                    setPurposeError('');
                    try {
                      await deletePurpose.mutateAsync(confirmDeletePurpose.id);
                      setConfirmDeletePurpose(null);
                    } catch (err: unknown) {
                      const msg = err instanceof Error ? err.message : 'Failed to delete';
                      setPurposeError(msg);
                      setConfirmDeletePurpose(null);
                    }
                  }}
                  className="px-3 py-1.5 text-xs font-medium text-white bg-red-600 rounded hover:bg-red-700"
                >
                  Delete
                </button>
                <button
                  onClick={() => setConfirmDeletePurpose(null)}
                  className="px-3 py-1.5 text-xs font-medium text-gray-600 bg-gray-100 rounded hover:bg-gray-200"
                >
                  Cancel
                </button>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Grafana Card */}
      <div className="mt-4 max-w-lg bg-white rounded-lg border p-5 space-y-4">
        <div className="flex items-center gap-2 text-gray-700">
          <BarChart3 size={18} />
          <h3 className="text-sm font-semibold">Grafana Dashboard</h3>
        </div>
        <p className="text-sm text-gray-500">
          Base URL for the Grafana dashboard. A "Grafana" button will appear on VPS pages linking to the dashboard filtered by IP.
          Example: <code className="text-xs bg-gray-100 px-1 py-0.5 rounded">https://grafana.example.com/d/abc123</code>
        </p>
        <input
          type="url"
          value={grafanaUrl}
          onChange={(e) => setGrafanaUrl(e.target.value)}
          className="input w-full"
          placeholder="https://grafana.example.com/d/..."
        />
        <div className="flex items-center gap-3">
          <button
            onClick={() => {
              if (grafanaUrl.trim()) {
                localStorage.setItem('sigma_grafana_url', grafanaUrl.trim());
              } else {
                localStorage.removeItem('sigma_grafana_url');
              }
              setGrafanaSaved(true);
              setTimeout(() => setGrafanaSaved(false), 2000);
            }}
            className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700"
          >
            Save
          </button>
          {grafanaSaved && <span className="text-sm text-green-600">Saved!</span>}
        </div>
      </div>

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
