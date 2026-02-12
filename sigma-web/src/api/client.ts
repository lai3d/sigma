import axios from 'axios';

export const apiClient = axios.create({
  baseURL: import.meta.env.VITE_API_URL || '/api',
});

apiClient.interceptors.request.use((config) => {
  // Don't attach auth headers to public login endpoints
  const url = config.url || '';
  if (url.startsWith('/auth/login')) {
    return config;
  }

  const token = localStorage.getItem('sigma_token');
  if (token) {
    config.headers['Authorization'] = `Bearer ${token}`;
  } else {
    const apiKey = localStorage.getItem('sigma_api_key');
    if (apiKey) {
      config.headers['X-Api-Key'] = apiKey;
    }
  }
  return config;
});

apiClient.interceptors.response.use(
  (response) => response,
  (error) => {
    if (error.response?.status === 401) {
      localStorage.removeItem('sigma_token');
      localStorage.removeItem('sigma_user');
      if (window.location.pathname !== '/login') {
        window.location.href = '/login';
      }
    }
    return Promise.reject(error);
  }
);
