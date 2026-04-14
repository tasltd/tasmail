// Added: WebAuthn/FIDO2 passkey API module for TMAIL-83
import { apiClient } from './client';

// -- Types --

/** PURPOSE: Relying party info returned by the server */
export interface RelyingParty {
  name: string;
  id: string;
}

/** PURPOSE: User info for credential creation */
export interface PublicKeyUser {
  id: string;
  name: string;
  display_name: string;
}

/** PURPOSE: Credential algorithm parameter */
export interface PubKeyCredParam {
  type: string;
  alg: number;
}

/** PURPOSE: Server response when starting passkey registration */
export interface RegisterBeginResponse {
  challenge: string;
  rp: RelyingParty;
  user: PublicKeyUser;
  pub_key_cred_params: PubKeyCredParam[];
  timeout: number;
  attestation: string;
}

/** PURPOSE: Request body to complete passkey registration */
export interface RegisterCompleteRequest {
  credential_id: string;
  public_key: string;
  attestation_object: unknown;
  client_data_json: unknown;
  name: string;
}

/** PURPOSE: Server response after successful registration */
export interface RegisterCompleteResponse {
  id: string;
  credential_id: string;
  name: string;
}

/** PURPOSE: Allowed credential for authentication */
export interface AllowedCredential {
  type: string;
  id: string;
}

/** PURPOSE: Server response when starting passkey authentication */
export interface AuthenticateBeginResponse {
  challenge: string;
  timeout: number;
  rp_id: string;
  allow_credentials: AllowedCredential[];
}

/** PURPOSE: Request body to complete passkey authentication */
export interface AuthenticateCompleteRequest {
  credential_id: string;
  authenticator_data: unknown;
  client_data_json: unknown;
  signature: string;
}

/** PURPOSE: Server response after successful authentication */
export interface AuthenticateCompleteResponse {
  verified: boolean;
  sign_count: number;
}

/** PURPOSE: Stored passkey info shown in the settings UI */
export interface PasskeyCredential {
  id: string;
  credential_id: string;
  name: string;
  sign_count: number;
  created_at: string;
  last_used_at: string | null;
}

// -- Utility functions --

/** PURPOSE: Convert ArrayBuffer to base64url string (no padding) for WebAuthn data transport */
export function bufferToBase64url(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = '';
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

/** PURPOSE: Convert base64url string to ArrayBuffer for WebAuthn credential creation */
export function base64urlToBuffer(base64url: string): ArrayBuffer {
  const base64 = base64url.replace(/-/g, '+').replace(/_/g, '/');
  const padded = base64 + '='.repeat((4 - (base64.length % 4)) % 4);
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes.buffer;
}

// -- API client --

export const webauthnApi = {
  /** PURPOSE: Start passkey registration ceremony */
  registerBegin: () =>
    apiClient.post<RegisterBeginResponse>('/webauthn/register/begin'),

  /** PURPOSE: Complete passkey registration with attestation data */
  registerComplete: (data: RegisterCompleteRequest) =>
    apiClient.post<RegisterCompleteResponse>('/webauthn/register/complete', data),

  /** PURPOSE: Start passkey authentication ceremony */
  authenticateBegin: () =>
    apiClient.post<AuthenticateBeginResponse>('/webauthn/authenticate/begin'),

  /** PURPOSE: Complete passkey authentication with assertion data */
  authenticateComplete: (data: AuthenticateCompleteRequest) =>
    apiClient.post<AuthenticateCompleteResponse>('/webauthn/authenticate/complete', data),

  /** PURPOSE: List all registered passkeys for the current user */
  listCredentials: () =>
    apiClient.get<PasskeyCredential[]>('/webauthn/credentials'),

  /** PURPOSE: Delete a registered passkey by ID */
  deleteCredential: (id: string) =>
    apiClient.delete<void>(`/webauthn/credentials/${id}`),
};
