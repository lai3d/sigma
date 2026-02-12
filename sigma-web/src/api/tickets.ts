import { apiClient } from './client';
import type {
  Ticket,
  TicketComment,
  CreateTicket,
  UpdateTicket,
  TicketListQuery,
  PaginatedResponse,
} from '@/types/api';

export async function listTickets(query?: TicketListQuery): Promise<PaginatedResponse<Ticket>> {
  const params = new URLSearchParams();
  if (query?.status) params.set('status', query.status);
  if (query?.priority) params.set('priority', query.priority);
  if (query?.assigned_to) params.set('assigned_to', query.assigned_to);
  if (query?.vps_id) params.set('vps_id', query.vps_id);
  if (query?.page) params.set('page', String(query.page));
  if (query?.per_page) params.set('per_page', String(query.per_page));
  const { data } = await apiClient.get(`/tickets?${params.toString()}`);
  return data;
}

export async function getTicket(id: string): Promise<Ticket> {
  const { data } = await apiClient.get(`/tickets/${id}`);
  return data;
}

export async function createTicket(input: CreateTicket): Promise<Ticket> {
  const { data } = await apiClient.post('/tickets', input);
  return data;
}

export async function updateTicket(id: string, input: UpdateTicket): Promise<Ticket> {
  const { data } = await apiClient.put(`/tickets/${id}`, input);
  return data;
}

export async function deleteTicket(id: string): Promise<void> {
  await apiClient.delete(`/tickets/${id}`);
}

export async function listComments(ticketId: string): Promise<TicketComment[]> {
  const { data } = await apiClient.get(`/tickets/${ticketId}/comments`);
  return data;
}

export async function addComment(ticketId: string, body: string): Promise<TicketComment> {
  const { data } = await apiClient.post(`/tickets/${ticketId}/comments`, { body });
  return data;
}
