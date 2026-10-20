import { useFormatTime } from "@/lib/format";

interface TimestampProps {
  /** An engine timestamp (RFC 3339). */
  value: string;
  /** Adds seconds, where events within one minute must be told apart. */
  seconds?: boolean;
}

/** An engine timestamp in the laboratory time zone, with the zone named. */
export function Timestamp({ value, seconds }: TimestampProps) {
  const formatTime = useFormatTime();
  return <time dateTime={value}>{formatTime(value, { seconds })}</time>;
}
