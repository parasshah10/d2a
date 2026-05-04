import { useState, useCallback } from 'react';
import { AuthContext } from '@/lib/auth-context';
import { getToken, setToken, clearToken, apiLogin, apiSetup, ApiError } from '@/lib/api';

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [token, setTokenState] = useState<string | null>(getToken());

  const login = useCallback(async (password: string): Promise<{ success: boolean; error?: string }> => {
    try {
      const res = await apiLogin(password);
      setToken(res.token);
      setTokenState(res.token);
      return { success: true };
    } catch (e) {
      const msg = e instanceof ApiError ? e.message : '登录失败';
      return { success: false, error: msg };
    }
  }, []);

  const setup = useCallback(async (password: string): Promise<{ success: boolean; error?: string }> => {
    try {
      const res = await apiSetup(password);
      setToken(res.token);
      setTokenState(res.token);
      return { success: true };
    } catch (e) {
      const msg = e instanceof ApiError ? e.message : '设置失败';
      return { success: false, error: msg };
    }
  }, []);

  const logout = useCallback(() => {
    clearToken();
    setTokenState(null);
  }, []);

  return (
    <AuthContext.Provider value={{ token, login, setup, logout, isAuthenticated: token !== null }}>
      {children}
    </AuthContext.Provider>
  );
}
