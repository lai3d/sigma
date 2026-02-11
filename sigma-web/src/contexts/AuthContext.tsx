import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from 'react';
import type { User, LoginRequest } from '@/types/api';
import * as authApi from '@/api/auth';

interface AuthContextType {
  user: User | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  login: (input: LoginRequest) => Promise<User>;
  logout: () => void;
  updateUser: (user: User) => void;
}

const AuthContext = createContext<AuthContextType | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(() => {
    const stored = localStorage.getItem('sigma_user');
    return stored ? JSON.parse(stored) : null;
  });
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const token = localStorage.getItem('sigma_token');
    if (!token) {
      setIsLoading(false);
      return;
    }

    authApi.getMe()
      .then((me) => {
        setUser(me);
        localStorage.setItem('sigma_user', JSON.stringify(me));
      })
      .catch(() => {
        localStorage.removeItem('sigma_token');
        localStorage.removeItem('sigma_user');
        setUser(null);
      })
      .finally(() => setIsLoading(false));
  }, []);

  const login = useCallback(async (input: LoginRequest) => {
    const res = await authApi.login(input);
    localStorage.setItem('sigma_token', res.token);
    localStorage.setItem('sigma_user', JSON.stringify(res.user));
    setUser(res.user);
    return res.user;
  }, []);

  const logout = useCallback(() => {
    localStorage.removeItem('sigma_token');
    localStorage.removeItem('sigma_user');
    setUser(null);
  }, []);

  const updateUser = useCallback((u: User) => {
    setUser(u);
    localStorage.setItem('sigma_user', JSON.stringify(u));
  }, []);

  return (
    <AuthContext.Provider value={{ user, isAuthenticated: !!user, isLoading, login, logout, updateUser }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within AuthProvider');
  return ctx;
}
