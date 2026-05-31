// TMAIL-351: RSVP responder. Shown when the authenticated user is in the
// event's attendee list (i.e. they were invited rather than organizing).
// The backend looks up the attendee row by claims.username so the SPA
// just sends the chosen status.
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Check, X, HelpCircle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { rsvpEvent, type EventAttendee, type RsvpRequest } from '@/api/calendar';

interface RsvpButtonsProps {
  eventId: string;
  currentRsvp: string;
  /** Called after a successful PATCH so the parent can update local state. */
  onRsvped?: (attendee: EventAttendee) => void;
}

const CHOICES: { value: RsvpRequest['status']; label: string; Icon: typeof Check }[] = [
  { value: 'accepted', label: 'Accept', Icon: Check },
  { value: 'maybe', label: 'Maybe', Icon: HelpCircle },
  { value: 'declined', label: 'Decline', Icon: X },
];

export function RsvpButtons({ eventId, currentRsvp, onRsvped }: RsvpButtonsProps) {
  const qc = useQueryClient();
  const rsvpMut = useMutation({
    mutationFn: (status: RsvpRequest['status']) => rsvpEvent(eventId, { status }),
    onSuccess: (attendee) => {
      qc.invalidateQueries({ queryKey: ['calendar'] });
      onRsvped?.(attendee);
    },
  });

  return (
    <div className="flex flex-wrap gap-1.5 mt-2" data-testid="rsvp-buttons">
      {CHOICES.map(({ value, label, Icon }) => {
        const isSelected = currentRsvp === value;
        return (
          <Button
            key={value}
            type="button"
            size="sm"
            variant={isSelected ? 'default' : 'outline'}
            disabled={rsvpMut.isPending}
            onClick={() => rsvpMut.mutate(value)}
            data-testid={`rsvp-${value}`}
            className="h-7 px-2 text-xs"
          >
            <Icon className="size-3 mr-1" />
            {label}
          </Button>
        );
      })}
      {rsvpMut.isError && (
        <p className="text-[11px] text-red-600 w-full">
          {(rsvpMut.error as Error).message}
        </p>
      )}
    </div>
  );
}
