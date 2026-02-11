import axios from 'axios';

export const apiClient = axios.create({
  baseURL: import.meta.env.VITE_API_URL || '/api',
});

apiClient.interceptors.request.use((config) => {
  const apiKey = localStorage.getItem('sigma_api_key');
  if (apiKey) {
    config.headers['X-Api-Key'] = apiKey;
  }
  return config;
});
