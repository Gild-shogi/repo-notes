const formatter = new Intl.DateTimeFormat("ja-JP", {
  timeZone: "Asia/Tokyo",
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
});

export function formatDateTime(input: number | string | Date): string {
  const date = new Date(input);
  if (Number.isNaN(date.getTime())) return "";
  return formatter.format(date);
}
