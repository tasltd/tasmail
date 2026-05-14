import { useState } from 'react';
import { Link } from 'react-router';
import { Calendar } from '@/components/ui/calendar';
import { Button } from '@/components/ui/button';
import { Plus, Trash2, Clock, CalendarDays, ArrowLeft } from 'lucide-react';

interface CalendarEvent {
  id: string;
  title: string;
  date: Date;
  time: string;
  description: string;
  color: string;
}

const EVENT_COLORS = [
  { label: 'Blue', value: 'bg-blue-500' },
  { label: 'Green', value: 'bg-green-500' },
  { label: 'Red', value: 'bg-red-500' },
  { label: 'Purple', value: 'bg-purple-500' },
  { label: 'Orange', value: 'bg-orange-500' },
];

const initialEvents: CalendarEvent[] = [
  {
    id: '1',
    title: 'Team Standup',
    date: new Date(),
    time: '09:00',
    description: 'Daily team sync',
    color: 'bg-blue-500',
  },
  {
    id: '2',
    title: 'Design Review',
    date: new Date(),
    time: '14:00',
    description: 'Review new UI mockups',
    color: 'bg-purple-500',
  },
];

export function CalendarView() {
  const [selectedDate, setSelectedDate] = useState<Date | undefined>(new Date());
  const [events, setEvents] = useState<CalendarEvent[]>(initialEvents);
  const [isAdding, setIsAdding] = useState(false);
  const [newTitle, setNewTitle] = useState('');
  const [newTime, setNewTime] = useState('09:00');
  const [newDescription, setNewDescription] = useState('');
  const [newColor, setNewColor] = useState('bg-blue-500');

  const eventsOnSelectedDate = events.filter(
    (e) =>
      selectedDate &&
      e.date.toDateString() === selectedDate.toDateString()
  );

  const datesWithEvents = events.map((e) => e.date);

  const handleSave = () => {
    if (!newTitle.trim() || !selectedDate) return;
    const event: CalendarEvent = {
      id: Date.now().toString(),
      title: newTitle.trim(),
      date: selectedDate,
      time: newTime,
      description: newDescription.trim(),
      color: newColor,
    };
    setEvents([...events, event]);
    setNewTitle('');
    setNewTime('09:00');
    setNewDescription('');
    setNewColor('bg-blue-500');
    setIsAdding(false);
  };

  const handleDelete = (id: string) => {
    setEvents(events.filter((e) => e.id !== id));
  };

  // Mobile: toggle between calendar panel and day view
  const [mobilePanel, setMobilePanel] = useState<'calendar' | 'day'>('calendar');

  return (
    <div className="flex h-full bg-white dark:bg-zinc-950 overflow-hidden">
      {/* Left: Calendar Picker — full width on mobile, fixed width on md+ */}
      <div className={`
        w-full md:w-80 border-r border-zinc-200 dark:border-zinc-800 flex flex-col shrink-0
        ${mobilePanel === 'day' ? 'hidden md:flex' : 'flex'}
      `}>
        <div className="h-14 border-b border-zinc-200 dark:border-zinc-800 flex items-center px-4 gap-2">
          <Link to="/">
            <Button variant="ghost" size="icon" title="Back to Mail">
              <ArrowLeft className="size-4" />
            </Button>
          </Link>
          <CalendarDays className="size-5 text-blue-600" />
          <h2 className="font-semibold text-base flex-1">Calendar</h2>
        </div>

        <div className="p-3 sm:p-4">
          <Calendar
            mode="single"
            selected={selectedDate}
            onSelect={(date) => {
              setSelectedDate(date);
              setMobilePanel('day'); // auto-navigate to day view on mobile
            }}
            modifiers={{ hasEvent: datesWithEvents }}
            modifiersClassNames={{
              hasEvent: 'underline decoration-blue-500 decoration-2 font-semibold',
            }}
            className="rounded-xl border border-zinc-200 dark:border-zinc-700 w-full"
          />
        </div>

        <div className="px-4 pb-4 flex gap-2">
          <Button
            className="flex-1"
            onClick={() => { setIsAdding(true); setMobilePanel('day'); }}
            disabled={!selectedDate}
          >
            <Plus className="size-4 mr-2" />
            New Event
          </Button>
          <Button
            variant="outline"
            className="md:hidden"
            onClick={() => setMobilePanel('day')}
            disabled={!selectedDate}
          >
            View Day
          </Button>
        </div>

        {/* Mini event summary for all upcoming */}
        <div className="flex-1 overflow-y-auto px-4 space-y-1">
          <p className="text-xs text-zinc-400 font-medium uppercase tracking-wide mb-2">Upcoming</p>
          {events
            .sort((a, b) => a.date.getTime() - b.date.getTime())
            .slice(0, 8)
            .map((e) => (
              <div
                key={e.id}
                className="flex items-center gap-2 text-sm py-1 cursor-pointer hover:text-blue-600 transition-colors"
                onClick={() => { setSelectedDate(new Date(e.date)); setMobilePanel('day'); }}
              >
                <span className={`size-2 rounded-full shrink-0 ${e.color}`} />
                <span className="truncate flex-1">{e.title}</span>
                <span className="text-xs text-zinc-400 shrink-0">
                  {e.date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })}
                </span>
              </div>
            ))}
        </div>
      </div>

      {/* Right: Day View — full width on mobile */}
      <div className={`
        flex-1 flex flex-col overflow-hidden
        ${mobilePanel === 'day' ? 'flex' : 'hidden md:flex'}
      `}>
        {/* Header */}
        <div className="h-14 border-b border-zinc-200 dark:border-zinc-800 flex items-center justify-between px-4 sm:px-6 gap-2">
          {/* Mobile back to calendar */}
          <button
            className="md:hidden text-blue-600 flex items-center gap-1 text-sm font-medium shrink-0"
            onClick={() => setMobilePanel('calendar')}
          >
            ← Cal
          </button>
          <div className="flex-1 min-w-0">
            <h3 className="font-semibold text-sm sm:text-base truncate">
              {selectedDate
                ? selectedDate.toLocaleDateString('en-US', {
                    weekday: 'long',
                    month: 'long',
                    day: 'numeric',
                    year: 'numeric',
                  })
                : 'Select a date'}
            </h3>
            <p className="text-xs text-zinc-400">
              {eventsOnSelectedDate.length} event{eventsOnSelectedDate.length !== 1 ? 's' : ''}
            </p>
          </div>
          <Button variant="outline" size="sm" onClick={() => { setSelectedDate(new Date()); setMobilePanel('day'); }} className="shrink-0">
            Today
          </Button>
        </div>

        {/* Add Event Form */}
        {isAdding && (
          <div className="mx-3 sm:mx-6 mt-3 sm:mt-4 p-3 sm:p-4 rounded-xl border border-zinc-200 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-900 space-y-3">
            <h4 className="font-semibold text-sm">Add New Event</h4>
            <input
              type="text"
              placeholder="Event title"
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
              className="w-full px-3 py-2 text-sm rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 outline-none focus:ring-2 focus:ring-blue-500"
              autoFocus
            />
            <div className="flex gap-3">
              <div className="flex-1">
                <label className="text-xs text-zinc-500 mb-1 block">Time</label>
                <input
                  type="time"
                  value={newTime}
                  onChange={(e) => setNewTime(e.target.value)}
                  className="w-full px-3 py-2 text-sm rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 outline-none focus:ring-2 focus:ring-blue-500"
                />
              </div>
              <div className="flex-1">
                <label className="text-xs text-zinc-500 mb-1 block">Color</label>
                <div className="flex gap-2 mt-1">
                  {EVENT_COLORS.map((c) => (
                    <button
                      key={c.value}
                      title={c.label}
                      onClick={() => setNewColor(c.value)}
                      className={`size-6 rounded-full ${c.value} border-2 transition-all ${
                        newColor === c.value ? 'border-zinc-900 dark:border-white scale-110' : 'border-transparent'
                      }`}
                    />
                  ))}
                </div>
              </div>
            </div>
            <textarea
              placeholder="Description (optional)"
              value={newDescription}
              onChange={(e) => setNewDescription(e.target.value)}
              rows={2}
              className="w-full px-3 py-2 text-sm rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 outline-none focus:ring-2 focus:ring-blue-500 resize-none"
            />
            <div className="flex gap-2 justify-end">
              <Button variant="ghost" size="sm" onClick={() => setIsAdding(false)}>
                Cancel
              </Button>
              <Button size="sm" onClick={handleSave} disabled={!newTitle.trim()}>
                Save Event
              </Button>
            </div>
          </div>
        )}

        {/* Events List */}
        <div className="flex-1 overflow-y-auto px-3 sm:px-6 py-3 sm:py-4 space-y-3">
          {eventsOnSelectedDate.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full text-center text-zinc-400 gap-3">
              <CalendarDays className="size-12 opacity-30" />
              <p className="text-sm">No events on this day</p>
              <Button variant="outline" size="sm" onClick={() => setIsAdding(true)}>
                <Plus className="size-4 mr-1" /> Add Event
              </Button>
            </div>
          ) : (
            eventsOnSelectedDate
              .sort((a, b) => a.time.localeCompare(b.time))
              .map((event) => (
                <div
                  key={event.id}
                  className="flex items-start gap-4 p-4 rounded-xl border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-900 hover:shadow-md transition-shadow group"
                >
                  <div className={`w-1.5 self-stretch rounded-full shrink-0 ${event.color}`} />
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center justify-between">
                      <h4 className="font-semibold text-sm truncate">{event.title}</h4>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="size-7 opacity-0 group-hover:opacity-100 text-zinc-400 hover:text-red-500 shrink-0"
                        onClick={() => handleDelete(event.id)}
                      >
                        <Trash2 className="size-4" />
                      </Button>
                    </div>
                    <div className="flex items-center gap-1 text-xs text-zinc-400 mt-0.5">
                      <Clock className="size-3" />
                      <span>{event.time}</span>
                    </div>
                    {event.description && (
                      <p className="text-xs text-zinc-500 mt-1">{event.description}</p>
                    )}
                  </div>
                </div>
              ))
          )}
        </div>
      </div>
    </div>
  );
}
