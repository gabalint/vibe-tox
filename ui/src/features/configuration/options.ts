/** Choice options `CODE Name` for coded catalog records, empty until the list has loaded. */
export function codedOptions(
  records: readonly { id: string; code: string; name: string }[] | undefined,
): { id: string; label: string }[] {
  return (records ?? []).map((record) => ({
    id: record.id,
    label: `${record.code} ${record.name}`,
  }));
}
