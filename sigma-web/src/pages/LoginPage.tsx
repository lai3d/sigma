import { useState } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import { useAuth } from '@/contexts/AuthContext';
import { isTotpChallenge } from '@/types/api';

export default function LoginPage() {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [totpCode, setTotpCode] = useState('');
  const [totpToken, setTotpToken] = useState('');
  const [phase, setPhase] = useState<'credentials' | 'totp'>('credentials');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const { login, loginTotp } = useAuth();
  const navigate = useNavigate();
  const location = useLocation();

  const from = (location.state as { from?: { pathname: string } })?.from?.pathname || '/';

  async function handleCredentials(e: React.FormEvent) {
    e.preventDefault();
    setError('');
    setLoading(true);

    try {
      const res = await login({ email, password });
      if (isTotpChallenge(res)) {
        setTotpToken(res.totp_token);
        setPhase('totp');
      } else {
        if (res.user.force_password_change) {
          navigate('/change-password', { replace: true });
        } else {
          navigate(from, { replace: true });
        }
      }
    } catch {
      setError('Invalid email or password');
    } finally {
      setLoading(false);
    }
  }

  async function handleTotp(e: React.FormEvent) {
    e.preventDefault();
    setError('');
    setLoading(true);

    try {
      const user = await loginTotp({ totp_token: totpToken, code: totpCode });
      if (user.force_password_change) {
        navigate('/change-password', { replace: true });
      } else {
        navigate(from, { replace: true });
      }
    } catch {
      setError('Invalid TOTP code');
    } finally {
      setLoading(false);
    }
  }

  function backToLogin() {
    setPhase('credentials');
    setTotpCode('');
    setTotpToken('');
    setError('');
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-50">
      <div className="w-full max-w-sm">
        <div className="text-center mb-8">
          <h1 className="text-3xl font-bold text-gray-900">
            <span className="text-blue-500">&#931;</span> Sigma
          </h1>
          <p className="text-sm text-gray-500 mt-1">VPS Fleet Management</p>
        </div>

        {phase === 'credentials' ? (
          <form onSubmit={handleCredentials} className="bg-white rounded-lg border p-6 space-y-4">
            <h2 className="text-lg font-semibold text-gray-900">Sign in</h2>

            {error && (
              <div className="p-3 text-sm text-red-700 bg-red-50 rounded-md">{error}</div>
            )}

            <div>
              <label className="block text-sm font-medium text-gray-700">Email</label>
              <input
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                className="input w-full mt-1"
                placeholder="admin@sigma.local"
                required
                autoFocus
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700">Password</label>
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="input w-full mt-1"
                placeholder="Enter password"
                required
              />
            </div>

            <button
              type="submit"
              disabled={loading}
              className="w-full px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50"
            >
              {loading ? 'Signing in...' : 'Sign in'}
            </button>
          </form>
        ) : (
          <form onSubmit={handleTotp} className="bg-white rounded-lg border p-6 space-y-4">
            <h2 className="text-lg font-semibold text-gray-900">Two-Factor Authentication</h2>
            <p className="text-sm text-gray-500">
              Enter the 6-digit code from your authenticator app.
            </p>

            {error && (
              <div className="p-3 text-sm text-red-700 bg-red-50 rounded-md">{error}</div>
            )}

            <div>
              <label className="block text-sm font-medium text-gray-700">TOTP Code</label>
              <input
                type="text"
                inputMode="numeric"
                pattern="[0-9]{6}"
                maxLength={6}
                value={totpCode}
                onChange={(e) => setTotpCode(e.target.value.replace(/\D/g, ''))}
                className="input w-full mt-1 text-center text-lg tracking-widest"
                placeholder="000000"
                required
                autoFocus
              />
            </div>

            <button
              type="submit"
              disabled={loading || totpCode.length !== 6}
              className="w-full px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50"
            >
              {loading ? 'Verifying...' : 'Verify'}
            </button>

            <button
              type="button"
              onClick={backToLogin}
              className="w-full text-sm text-gray-500 hover:text-gray-700"
            >
              Back to login
            </button>
          </form>
        )}
      </div>
    </div>
  );
}
