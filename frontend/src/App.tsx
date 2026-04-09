import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useAuth } from './hooks/useAuth';
import { AppShell } from './components/layout/AppShell';
import { LoginPage } from './components/auth/LoginPage';
import { ErrorBoundary } from './components/shared/ErrorBoundary';
import './App.css';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

function AppContent() {
  const { isAuthenticated, isLoading, login, logout } = useAuth();

  if (isLoading) {
    return <div className="app-loading">Loading...</div>;
  }

  if (!isAuthenticated) {
    return (
      <LoginPage
        onLogin={async (username, password) => {
          await login({ username, password });
        }}
      />
    );
  }

  return <AppShell onLogout={logout} />;
}

export default function App() {
  return (
    <ErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <AppContent />
      </QueryClientProvider>
    </ErrorBoundary>
  );
}
