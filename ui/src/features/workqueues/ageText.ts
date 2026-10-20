const MINUTE_MS = 60_000;
const MINUTES_PER_HOUR = 60;
const MINUTES_PER_DAY = 24 * MINUTES_PER_HOUR;

/**
 * How long an item has waited, from `since` (an engine timestamp) to `now` (milliseconds
 * since the epoch), such as "3 h 20 min" or "2 d 4 h". A time that does not parse shows as
 * "Unknown"; a time after `now` as under a minute.
 */
export function ageText(since: string, now: number): string {
  const start = Date.parse(since);
  if (Number.isNaN(start)) return "Unknown";
  const minutes = Math.floor((now - start) / MINUTE_MS);
  if (minutes < 1) return "Under 1 min";
  if (minutes < MINUTES_PER_HOUR) return `${minutes} min`;
  if (minutes < MINUTES_PER_DAY) {
    return `${Math.floor(minutes / MINUTES_PER_HOUR)} h ${minutes % MINUTES_PER_HOUR} min`;
  }
  const hours = Math.floor((minutes % MINUTES_PER_DAY) / MINUTES_PER_HOUR);
  return `${Math.floor(minutes / MINUTES_PER_DAY)} d ${hours} h`;
}
