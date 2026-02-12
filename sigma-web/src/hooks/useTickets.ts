import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import * as api from '@/api/tickets';
import type { CreateTicket, UpdateTicket, TicketListQuery } from '@/types/api';

export function useTickets(query?: TicketListQuery) {
  return useQuery({
    queryKey: ['tickets', query],
    queryFn: () => api.listTickets(query),
  });
}

export function useTicket(id: string) {
  return useQuery({
    queryKey: ['tickets', id],
    queryFn: () => api.getTicket(id),
    enabled: !!id,
  });
}

export function useCreateTicket() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateTicket) => api.createTicket(input),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['tickets'] }),
  });
}

export function useUpdateTicket() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateTicket }) =>
      api.updateTicket(id, data),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['tickets'] }),
  });
}

export function useDeleteTicket() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.deleteTicket(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['tickets'] }),
  });
}

export function useTicketComments(ticketId: string) {
  return useQuery({
    queryKey: ['tickets', ticketId, 'comments'],
    queryFn: () => api.listComments(ticketId),
    enabled: !!ticketId,
  });
}

export function useAddComment() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ ticketId, body }: { ticketId: string; body: string }) =>
      api.addComment(ticketId, body),
    onSuccess: (_data, vars) =>
      qc.invalidateQueries({ queryKey: ['tickets', vars.ticketId, 'comments'] }),
  });
}
