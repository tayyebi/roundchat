/**
 * AuthContext.
 *
 * Provides session state and login/logout actions to the component tree.
 * Only this context interacts with AuthService; screens import useAuth only.
 */
import React, { createContext, useCallback, useContext, useEffect, useState } from 'react';
import { AuthService } from '../services/AuthService';
import type { Session } from '../services/AuthService';

interface AuthContextValue {
  session: Session | null;
  loading: boolean;
  login: (email: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
}

const AuthContext = createContext<AuthContextValue>({
  session: null,
  loading: true,
  login: async () => {},
  logout: async () => {},
});

const authService = new AuthService();

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [session, setSession] = useState<Session | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    authService.getSession().then((s) => {
      setSession(s);
      setLoading(false);
    });
  }, []);

  const login = useCallback(async (email: string, password: string) => {
    const s = await authService.login(email, password);
    setSession(s);
  }, []);

  const logout = useCallback(async () => {
    await authService.logout();
    setSession(null);
  }, []);

  return (
    <AuthContext.Provider value={{ session, loading, login, logout }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth(): AuthContextValue {
  return useContext(AuthContext);
}
