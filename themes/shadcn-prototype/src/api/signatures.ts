// TMAIL-331: Modern UI signatures API client. Mirrors
// frontend/src/api/signatures.ts so the modern UI can list / create / edit /
// delete signatures and pick the default one to inject into the compose
// modal — without duplicating type drift across the two SPAs.
import { apiClient } from './client';

export interface Signature {
  id: string;
  mailbox_id: string;
  name: string;
  html_body: string;
  text_body: string;
  is_default: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateSignatureRequest {
  name: string;
  html_body: string;
  text_body: string;
  is_default?: boolean;
}

export interface UpdateSignatureRequest {
  name?: string;
  html_body?: string;
  text_body?: string;
  is_default?: boolean;
}

export async function fetchSignatures(): Promise<Signature[]> {
  return apiClient.get<Signature[]>('/signatures');
}

export async function createSignature(
  data: CreateSignatureRequest,
): Promise<Signature> {
  return apiClient.post<Signature>('/signatures', data);
}

export async function updateSignature(
  id: string,
  data: UpdateSignatureRequest,
): Promise<Signature> {
  return apiClient.put<Signature>(`/signatures/${id}`, data);
}

export async function deleteSignature(id: string): Promise<void> {
  await apiClient.delete(`/signatures/${id}`);
}

// TMAIL-331: shared helper so ComposeModal and SignaturesPanel agree on
// which signature is "the default". Returns the first signature flagged
// is_default. If none is flagged but the user has exactly one signature,
// fall back to that one — matches the classic SPA's behavior where users
// who created a single signature expect it to be used by default without
// having to explicitly tick a box.
export function pickDefaultSignature(
  sigs: Signature[] | undefined | null,
): Signature | null {
  if (!sigs || sigs.length === 0) return null;
  const flagged = sigs.find((s) => s.is_default);
  if (flagged) return flagged;
  if (sigs.length === 1) return sigs[0];
  return null;
}
