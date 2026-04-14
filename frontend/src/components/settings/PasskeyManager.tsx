// Added: PasskeyManager component for TMAIL-83 WebAuthn/FIDO2 passkey management
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Fingerprint, Plus, Trash2, ShieldCheck, AlertCircle } from 'lucide-react';
import { webauthnApi, bufferToBase64url, base64urlToBuffer } from '../../api/webauthn';
import type { PasskeyCredential } from '../../api/webauthn';

/**
 * PURPOSE: Settings panel for managing WebAuthn/FIDO2 passkeys
 * CONSTRAINTS: Requires browser WebAuthn API support (navigator.credentials)
 * EXTERNAL: Uses /api/webauthn/* endpoints and browser navigator.credentials API
 */
export function PasskeyManager() {
  const queryClient = useQueryClient();
  const [passkeyName, setPasskeyName] = useState('');
  const [error, setError] = useState('');
  const [isRegistering, setIsRegistering] = useState(false);

  // Added: Check browser WebAuthn support
  const isWebAuthnSupported =
    typeof window !== 'undefined' &&
    typeof window.PublicKeyCredential !== 'undefined';

  // Added: Fetch registered passkeys
  const { data: credentials, isLoading } = useQuery<PasskeyCredential[]>({
    queryKey: ['webauthn-credentials'],
    queryFn: webauthnApi.listCredentials,
  });

  // Added: Delete passkey mutation
  const deleteMutation = useMutation({
    mutationFn: (id: string) => webauthnApi.deleteCredential(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['webauthn-credentials'] });
      setError('');
    },
    onError: (err: Error) => setError(err.message),
  });

  // Added: Register a new passkey using the browser WebAuthn API
  const handleRegister = async () => {
    if (!isWebAuthnSupported) {
      setError('Your browser does not support WebAuthn/passkeys.');
      return;
    }

    setIsRegistering(true);
    setError('');

    try {
      // Step 1: Get challenge from server
      const options = await webauthnApi.registerBegin();

      // Step 2: Create credential via browser API
      const credential = await navigator.credentials.create({
        publicKey: {
          challenge: base64urlToBuffer(options.challenge),
          rp: {
            name: options.rp.name,
            id: options.rp.id,
          },
          user: {
            id: base64urlToBuffer(options.user.id),
            name: options.user.name,
            displayName: options.user.display_name,
          },
          pubKeyCredParams: options.pub_key_cred_params.map((param) => ({
            type: param.type as PublicKeyCredentialType,
            alg: param.alg,
          })),
          timeout: options.timeout,
          attestation: options.attestation as AttestationConveyancePreference,
        },
      }) as PublicKeyCredential | null;

      if (!credential) {
        setError('Passkey registration was cancelled or failed.');
        return;
      }

      // Step 3: Send attestation to server
      const attestationResponse = credential.response as AuthenticatorAttestationResponse;
      await webauthnApi.registerComplete({
        credential_id: bufferToBase64url(credential.rawId),
        public_key: bufferToBase64url(attestationResponse.getPublicKey?.() || attestationResponse.attestationObject),
        attestation_object: bufferToBase64url(attestationResponse.attestationObject),
        client_data_json: bufferToBase64url(attestationResponse.clientDataJSON),
        name: passkeyName || 'Security Key',
      });

      // Added: Refresh the list and reset form
      queryClient.invalidateQueries({ queryKey: ['webauthn-credentials'] });
      setPasskeyName('');
    } catch (err) {
      // NOTE: DOMException from browser API or API error
      const message = err instanceof Error ? err.message : 'Passkey registration failed';
      setError(message);
    } finally {
      setIsRegistering(false);
    }
  };

  // Added: Format date for display
  const formatDate = (dateStr: string | null): string => {
    if (!dateStr) return 'Never';
    return new Date(dateStr).toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  return (
    <div style={{ padding: '24px', maxWidth: '600px' }}>
      <h2 style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '16px' }}>
        <Fingerprint size={24} />
        Passkeys (WebAuthn)
      </h2>

      <p style={{ marginBottom: '16px', color: 'var(--color-text-secondary)', fontSize: '14px' }}>
        Passkeys use your device's biometrics or security key for passwordless authentication.
        Register one or more passkeys as a second factor.
      </p>

      {/* Added: Browser support warning */}
      {!isWebAuthnSupported && (
        <div
          data-testid="webauthn-unsupported"
          style={{
            padding: '12px',
            background: 'var(--color-warning-bg, #fff3cd)',
            color: 'var(--color-warning, #856404)',
            borderRadius: '8px',
            marginBottom: '16px',
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
          }}
        >
          <AlertCircle size={20} />
          Your browser does not support WebAuthn. Please use a modern browser like Chrome, Firefox, or Safari.
        </div>
      )}

      {/* Added: Error display */}
      {error && (
        <div
          data-testid="passkey-error"
          style={{
            padding: '8px 12px',
            background: 'var(--color-error-bg, #ffeaea)',
            color: 'var(--color-error, #dc3545)',
            borderRadius: '4px',
            marginBottom: '12px',
          }}
        >
          {error}
        </div>
      )}

      {/* Added: Register new passkey form */}
      <div style={{ marginBottom: '24px', padding: '16px', background: 'var(--color-bg-secondary, #f8f9fa)', borderRadius: '8px' }}>
        <h4 style={{ marginBottom: '8px' }}>Register a new passkey</h4>
        <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
          <input
            type="text"
            value={passkeyName}
            onChange={(e) => setPasskeyName(e.target.value)}
            placeholder="Name (e.g., MacBook fingerprint)"
            data-testid="passkey-name-input"
            style={{ flex: 1, padding: '8px 12px' }}
          />
          <button
            className="btn btn--primary"
            onClick={handleRegister}
            disabled={isRegistering || !isWebAuthnSupported}
            data-testid="register-passkey-btn"
          >
            <Plus size={16} />
            {isRegistering ? 'Registering...' : 'Add Passkey'}
          </button>
        </div>
      </div>

      {/* Added: List of registered passkeys */}
      <h3 style={{ marginBottom: '12px' }}>Registered Passkeys</h3>

      {isLoading ? (
        <p style={{ color: 'var(--color-text-secondary)' }}>Loading passkeys...</p>
      ) : !credentials || credentials.length === 0 ? (
        <div
          data-testid="no-passkeys"
          style={{
            padding: '24px',
            textAlign: 'center',
            color: 'var(--color-text-secondary)',
            border: '1px dashed var(--color-border, #dee2e6)',
            borderRadius: '8px',
          }}
        >
          <Fingerprint size={32} style={{ marginBottom: '8px', opacity: 0.5 }} />
          <p>No passkeys registered yet.</p>
        </div>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
          {credentials.map((credential) => (
            <div
              key={credential.id}
              data-testid={`passkey-item-${credential.id}`}
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                padding: '12px 16px',
                background: 'var(--color-bg-secondary, #f8f9fa)',
                borderRadius: '8px',
                border: '1px solid var(--color-border, #dee2e6)',
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                <ShieldCheck size={20} color="var(--color-success, #28a745)" />
                <div>
                  <div style={{ fontWeight: 500 }}>{credential.name}</div>
                  <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)' }}>
                    Created {formatDate(credential.created_at)}
                    {' · '}
                    Last used {formatDate(credential.last_used_at)}
                    {' · '}
                    Used {credential.sign_count} times
                  </div>
                </div>
              </div>
              <button
                className="btn btn--danger btn--sm"
                onClick={() => deleteMutation.mutate(credential.id)}
                disabled={deleteMutation.isPending}
                title="Remove passkey"
                data-testid={`delete-passkey-${credential.id}`}
              >
                <Trash2 size={14} />
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
