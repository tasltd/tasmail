// Added: Billing management UI for Paystack/MoMo payment integration (TMAIL-46)
// PURPOSE: Allows users to view plans, manage subscriptions, and see payment history
// EXTERNAL: Uses TanStack Query for data fetching; redirects to Paystack checkout for card payments

import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, CreditCard, Phone, CheckCircle, XCircle, Clock, RefreshCw } from 'lucide-react';
import { listPlans, getSubscription, subscribe, listPayments } from '../../api/billing';
import type { BillingPlan, SubscribeRequest } from '../../api/billing';
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

export function BillingManager() {
  const setViewMode = useMailStore((s) => s.setViewMode);
  const queryClient = useQueryClient();

  // Added: State for MoMo phone number input
  const [momoPhone, setMomoPhone] = useState('');
  const [selectedPlan, setSelectedPlan] = useState<BillingPlan | null>(null);
  const [showMomoInput, setShowMomoInput] = useState(false);

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
      // Added: Redirect to Paystack checkout if authorization_url is provided
      if (resp.authorization_url) {
        window.location.href = resp.authorization_url;
      } else {
        // NOTE: MoMo — payment prompt sent to phone, show confirmation
        alert(`Payment request sent to your phone. Reference: ${resp.reference}`);
      }
      setSelectedPlan(null);
      setShowMomoInput(false);
      setMomoPhone('');
    },
    onError: () => {
      alert('Payment initialization failed. Please try again.');
    },
  });

  // Added: Handle plan selection for Paystack (card) payment
  const handlePaystack = (plan: BillingPlan) => {
    subscribeMut.mutate({
      plan_id: plan.id,
      provider: 'paystack',
    });
  };

  // Added: Handle plan selection for MoMo payment — shows phone input first
  const handleMomoSelect = (plan: BillingPlan) => {
    setSelectedPlan(plan);
    setShowMomoInput(true);
  };

  // Added: Submit MoMo payment with phone number
  const handleMomoSubmit = () => {
    if (!selectedPlan || !momoPhone.trim()) return;
    subscribeMut.mutate({
      plan_id: selectedPlan.id,
      provider: 'mtn_momo',
      phone_number: momoPhone.trim(),
    });
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
                <strong>Provider:</strong> {currentSub.provider === 'mtn_momo' ? 'MTN MoMo' : 'Paystack'}
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
                  onClick={() => handlePaystack(plan)}
                  disabled={subscribeMut.isPending}
                >
                  <CreditCard size={16} />
                  Pay with Card
                </button>
                <button
                  className="btn btn--secondary"
                  onClick={() => handleMomoSelect(plan)}
                  disabled={subscribeMut.isPending}
                >
                  <Phone size={16} />
                  MTN MoMo
                </button>
              </div>
            </div>
          ))}
        </div>
      </section>

      {/* Added: MoMo phone number input dialog */}
      {showMomoInput && selectedPlan && (
        <section className="settings-section" data-testid="momo-input">
          <h3>MTN MoMo Payment — {selectedPlan.name}</h3>
          <p>Enter your MTN MoMo phone number to receive a payment prompt.</p>
          <div style={{ display: 'flex', gap: '8px', marginTop: '8px', maxWidth: '400px' }}>
            <input
              type="tel"
              className="input"
              placeholder="0241234567"
              value={momoPhone}
              onChange={(e) => setMomoPhone(e.target.value)}
              data-testid="momo-phone-input"
            />
            <button
              className="btn btn--primary"
              onClick={handleMomoSubmit}
              disabled={!momoPhone.trim() || subscribeMut.isPending}
            >
              {subscribeMut.isPending ? <RefreshCw size={16} className="spin" /> : 'Send'}
            </button>
            <button
              className="btn btn--ghost"
              onClick={() => {
                setShowMomoInput(false);
                setSelectedPlan(null);
                setMomoPhone('');
              }}
            >
              Cancel
            </button>
          </div>
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
                  <td>{p.provider === 'mtn_momo' ? 'MTN MoMo' : 'Paystack'}</td>
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
