// Changed: Billing management UI now matches PayPro's provider set — Paystack, Mastercard, Cybersource, Bank Transfer (TMAIL-46).
// MoMo provider removed: TASMail pivoted to mirror PayPro, which does not use MoMo.
// PURPOSE: Allows users to view plans, manage subscriptions, and see payment history.
// EXTERNAL: Uses TanStack Query for data fetching; redirects to provider checkout when authorization_url is a URL,
//           otherwise renders inline instructions (bank transfer) or session/invoice references.

import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, CreditCard, Landmark, FileText, CheckCircle, XCircle, Clock, RefreshCw } from 'lucide-react';
import {
  listPlans,
  getSubscription,
  subscribe,
  listPayments,
  providerLabel,
  type BillingPlan,
  type BillingProvider,
  type SubscribeRequest,
} from '../../api/billing';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// Added: Status badge styling for payment statuses
function statusBadgeClass(status: string): string {
  switch (status) {
    case 'success':
    case 'active':
      return 'badge badge--success';
    case 'failed':
    case 'cancelled':
      return 'badge badge--danger';
    case 'pending':
      return 'badge badge--warning';
    case 'refunded':
      return 'badge badge--info';
    default:
      return 'badge';
  }
}

// Added: Format GHS currency for display
function formatGHS(amount: number): string {
  return `GHS ${amount.toFixed(2)}`;
}

// Added: True when the backend returned a real HTTP(S) checkout URL we can redirect to.
function isHttpUrl(value: string | null | undefined): value is string {
  return !!value && (value.startsWith('http://') || value.startsWith('https://'));
}

// Added: Extract human-readable bank-transfer instructions from the backend's
// "bank_transfer:..." authorization_url payload, falling back to the raw string.
function bankInstructionsFrom(authUrl: string | null | undefined): string | null {
  if (!authUrl) return null;
  return authUrl.startsWith('bank_transfer:') ? authUrl.slice('bank_transfer:'.length) : authUrl;
}

export function BillingManager() {
  const setViewMode = useMailStore((s) => s.setViewMode);
  const queryClient = useQueryClient();

  // Added: After a non-redirect provider (bank transfer / mpgs session / cybersource invoice)
  // returns instructions or a reference, surface them inline so the user can act on them.
  const [pendingInstruction, setPendingInstruction] = useState<{
    provider: BillingProvider;
    reference: string;
    detail: string | null;
  } | null>(null);

  // Added: Fetch billing data
  const { data: plans, isLoading: plansLoading } = useQuery({
    queryKey: ['billing-plans'],
    queryFn: listPlans,
  });

  const { data: currentSub, isLoading: subLoading } = useQuery({
    queryKey: ['billing-subscription'],
    queryFn: getSubscription,
  });

  const { data: payments, isLoading: paymentsLoading } = useQuery({
    queryKey: ['billing-payments'],
    queryFn: listPayments,
  });

  // Added: Subscribe mutation
  const subscribeMut = useMutation({
    mutationFn: (data: SubscribeRequest) => subscribe(data),
    onSuccess: (resp) => {
      queryClient.invalidateQueries({ queryKey: ['billing-subscription'] });
      queryClient.invalidateQueries({ queryKey: ['billing-payments'] });
      // Changed: Only Paystack returns a redirectable URL. Mastercard returns "mpgs:session:{id}",
      // Cybersource returns "cybersource:invoice:{id}", Bank Transfer returns "bank_transfer:{instructions}".
      // Render those inline rather than navigating.
      if (isHttpUrl(resp.authorization_url)) {
        window.location.href = resp.authorization_url;
        return;
      }
      setPendingInstruction({
        provider: resp.provider as BillingProvider,
        reference: resp.reference,
        detail: bankInstructionsFrom(resp.authorization_url),
      });
    },
    onError: () => {
      alert('Payment initialization failed. Please try again.');
    },
  });

  // Added: Handle plan selection for any provider — backend whitelists paystack/mastercard/cybersource/bank_transfer.
  const handleSubscribe = (plan: BillingPlan, provider: BillingProvider) => {
    subscribeMut.mutate({ plan_id: plan.id, provider });
  };

  if (plansLoading || subLoading) {
    return <LoadingSkeleton />;
  }

  return (
    <div className="settings-panel" data-testid="billing-manager">
      <div className="settings-panel__header">
        <button className="btn btn--ghost" onClick={() => setViewMode('list')}>
          <ArrowLeft size={18} />
        </button>
        <h2>Billing & Subscription</h2>
      </div>

      {/* Added: Current subscription status */}
      <section className="settings-section" data-testid="subscription-status">
        <h3>Current Subscription</h3>
        {currentSub ? (
          <div className="card" style={{ padding: '16px', marginBottom: '16px' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div>
                <strong>Status:</strong>{' '}
                <span className={statusBadgeClass(currentSub.status)}>{currentSub.status}</span>
              </div>
              <div>
                <strong>Provider:</strong> {providerLabel(currentSub.provider)}
              </div>
            </div>
            {currentSub.current_period_end && (
              <p style={{ marginTop: '8px', color: 'var(--color-text-secondary)' }}>
                Current period ends: {new Date(currentSub.current_period_end).toLocaleDateString()}
              </p>
            )}
          </div>
        ) : (
          <p style={{ color: 'var(--color-text-secondary)' }}>No active subscription. Choose a plan below.</p>
        )}
      </section>

      {/* Added: Plan selection cards */}
      <section className="settings-section" data-testid="plan-cards">
        <h3>Available Plans</h3>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))', gap: '16px' }}>
          {plans?.map((plan) => (
            <div
              key={plan.id}
              className="card"
              style={{
                padding: '20px',
                border: '1px solid var(--color-border)',
                borderRadius: '8px',
              }}
              data-testid={`plan-card-${plan.id}`}
            >
              <h4 style={{ marginBottom: '8px' }}>{plan.name}</h4>
              {plan.description && (
                <p style={{ color: 'var(--color-text-secondary)', marginBottom: '12px' }}>
                  {plan.description}
                </p>
              )}
              <div style={{ fontSize: '1.5rem', fontWeight: 'bold', marginBottom: '12px' }}>
                {formatGHS(plan.price_cedis)}
                <span style={{ fontSize: '0.875rem', fontWeight: 'normal' }}>/{plan.interval}</span>
              </div>
              <ul style={{ listStyle: 'none', padding: 0, marginBottom: '16px' }}>
                <li>Mailboxes: {plan.max_mailboxes}</li>
                <li>Storage: {plan.storage_gb} GB</li>
              </ul>
              <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                <button
                  className="btn btn--primary"
                  onClick={() => handleSubscribe(plan, 'paystack')}
                  disabled={subscribeMut.isPending}
                  data-testid={`pay-paystack-${plan.id}`}
                >
                  <CreditCard size={16} />
                  Pay with Card
                </button>
                <button
                  className="btn btn--secondary"
                  onClick={() => handleSubscribe(plan, 'mastercard')}
                  disabled={subscribeMut.isPending}
                  data-testid={`pay-mastercard-${plan.id}`}
                >
                  <CreditCard size={16} />
                  Mastercard
                </button>
                <button
                  className="btn btn--secondary"
                  onClick={() => handleSubscribe(plan, 'cybersource')}
                  disabled={subscribeMut.isPending}
                  data-testid={`pay-cybersource-${plan.id}`}
                >
                  <FileText size={16} />
                  Invoice
                </button>
                <button
                  className="btn btn--ghost"
                  onClick={() => handleSubscribe(plan, 'bank_transfer')}
                  disabled={subscribeMut.isPending}
                  data-testid={`pay-bank-${plan.id}`}
                >
                  <Landmark size={16} />
                  Bank Transfer
                </button>
              </div>
            </div>
          ))}
        </div>
      </section>

      {/* Added: Non-redirect provider instructions (bank transfer, mpgs session id, cybersource invoice id). */}
      {pendingInstruction && (
        <section className="settings-section" data-testid="payment-instructions">
          <h3>Next steps — {providerLabel(pendingInstruction.provider)}</h3>
          <p>
            <strong>Reference:</strong>{' '}
            <span style={{ fontFamily: 'monospace' }}>{pendingInstruction.reference}</span>
          </p>
          {pendingInstruction.detail && (
            <pre
              style={{
                whiteSpace: 'pre-wrap',
                background: 'var(--color-surface)',
                padding: '12px',
                borderRadius: '6px',
                marginTop: '8px',
              }}
              data-testid="payment-instructions-detail"
            >
              {pendingInstruction.detail}
            </pre>
          )}
          <button
            className="btn btn--ghost"
            onClick={() => setPendingInstruction(null)}
            style={{ marginTop: '8px' }}
          >
            Dismiss
          </button>
          {subscribeMut.isPending && <RefreshCw size={16} className="spin" />}
        </section>
      )}

      {/* Added: Payment history table */}
      <section className="settings-section" data-testid="payment-history">
        <h3>Payment History</h3>
        {paymentsLoading ? (
          <LoadingSkeleton />
        ) : payments && payments.length > 0 ? (
          <table className="table" style={{ width: '100%' }}>
            <thead>
              <tr>
                <th>Date</th>
                <th>Amount</th>
                <th>Provider</th>
                <th>Reference</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {payments.map((p) => (
                <tr key={p.id}>
                  <td>{p.created_at ? new Date(p.created_at).toLocaleDateString() : '—'}</td>
                  <td>{formatGHS(p.amount_cedis)}</td>
                  <td>{providerLabel(p.provider)}</td>
                  <td style={{ fontFamily: 'monospace', fontSize: '0.85rem' }}>{p.provider_ref}</td>
                  <td>
                    <span className={statusBadgeClass(p.status)}>
                      {p.status === 'success' && <CheckCircle size={14} />}
                      {p.status === 'failed' && <XCircle size={14} />}
                      {p.status === 'pending' && <Clock size={14} />}
                      {' '}{p.status}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p style={{ color: 'var(--color-text-secondary)' }}>No payments yet.</p>
        )}
      </section>
    </div>
  );
}
