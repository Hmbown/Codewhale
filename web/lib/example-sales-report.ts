/** Illustrative local data only: no customer records or model execution. */
const SAMPLE_ORDERS = [
  ["2026-09-07", 12_000],
  ["2026-09-09", 18_000],
  ["2026-09-11", 20_000],
  ["2026-09-14", 16_000],
  ["2026-09-16", 20_000],
  ["2026-09-18", 24_000],
  ["2026-09-21", 18_000],
  ["2026-09-23", 22_000],
  ["2026-09-25", 30_000],
  ["2026-09-28", 16_000],
  ["2026-09-29", 19_000],
  ["2026-09-30", 21_000],
  ["2026-10-02", 24_000],
] as const;

type WeeklySalesRow = { weekStart: string; orders: number; salesCents: number };

const weeks = new Map<string, WeeklySalesRow>();
for (const [date, salesCents] of SAMPLE_ORDERS) {
  const monday = new Date(`${date}T00:00:00Z`);
  monday.setUTCDate(monday.getUTCDate() - (monday.getUTCDay() + 6) % 7);
  const weekStart = monday.toISOString().slice(0, 10);
  const row = weeks.get(weekStart) ?? { weekStart, orders: 0, salesCents: 0 };
  row.orders += 1;
  row.salesCents += salesCents;
  weeks.set(weekStart, row);
}

const rows = [...weeks.values()];
export const EXAMPLE_SALES_REPORT = {
  rows,
  orders: SAMPLE_ORDERS.length,
  salesCents: rows.reduce((sum, row) => sum + row.salesCents, 0),
  change: rows[rows.length - 1].salesCents / rows[0].salesCents - 1,
};

/** Quoted CSV fields and a UTF-8 marker keep localized downloads readable. */
export function csvDataUri(rows: readonly (readonly (string | number)[])[]): string {
  const csv = rows.map(row => row.map(value => `"${String(value).replaceAll('"', '""')}"`).join(",")).join("\r\n");
  return `data:text/csv;charset=utf-8,${encodeURIComponent(`\uFEFF${csv}\r\n`)}`;
}

export const SAMPLE_SALES_CSV_URI = csvDataUri([
  ["date", "sales_usd"],
  ...SAMPLE_ORDERS.map(([date, cents]) => [date, (cents / 100).toFixed(2)]),
]);
