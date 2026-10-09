"use client";

import { useId, useState } from "react";
import { getMerchSizingCopy, merchSizingGarmentLabel } from "@/lib/content/merch-sizing";
import { MERCH_SIZE_CHARTS, merchMeasurement, type MerchSizingProductCode, type MerchSizingUnit } from "@/lib/merch/sizing";
import styles from "./merch-size-guide.module.css";

/** Supplier guidance only. This component does not choose a purchase size or enable variants. */
export function MerchSizeGuide({ locale, initialProductCode = "DJCTX" }: {
  locale: string;
  initialProductCode?: MerchSizingProductCode;
}) {
  const d = getMerchSizingCopy(locale);
  const id = useId();
  const [productCode, setProductCode] = useState<MerchSizingProductCode>(initialProductCode);
  const [unit, setUnit] = useState<MerchSizingUnit>("cm");
  const chart = MERCH_SIZE_CHARTS.find((item) => item.code === productCode)!;
  const garment = merchSizingGarmentLabel(chart.code, locale);
  const number = new Intl.NumberFormat(locale, { maximumFractionDigits: 1 });
  const measurement = (value: number | null) => value === null ? "—" : number.format(merchMeasurement(value, unit));
  const caption = d.caption.replace("{garment}", garment).replace("{unit}", d[unit]);
  const tolerance = d.tolerance.replace("{value}", measurement(chart.toleranceCm)).replace("{unit}", d[unit]);
  const tableNotes = `${id}-bust ${id}-tolerance${chart.warning ? ` ${id}-warning` : ""}`;

  return <section className={styles.guide} aria-labelledby={`${id}-title`}>
    <h2 id={`${id}-title`}>{d.title}</h2>
    <p>{d.introduction}</p>
    <p className={styles.note}>{d.guidance}</p>
    <div className={styles.controls}>
      <div className={styles.field}>
        <label htmlFor={`${id}-garment`}>{d.garment}</label>
        <select id={`${id}-garment`} value={productCode} onChange={(event) => {
          const selected = MERCH_SIZE_CHARTS.find((item) => item.code === event.target.value);
          if (selected) setProductCode(selected.code);
        }}>
          {MERCH_SIZE_CHARTS.map((item) => <option key={item.code} value={item.code}>{merchSizingGarmentLabel(item.code, locale)}</option>)}
        </select>
      </div>
      <fieldset className={styles.units}>
        <legend>{d.units}</legend>
        {(["cm", "in"] as const).map((value) => <label key={value}>
          <input type="radio" name={`${id}-unit`} value={value} checked={unit === value} onChange={() => setUnit(value)} />
          {d[value]}
        </label>)}
      </fieldset>
    </div>
    {unit === "in" ? <p className={styles.note}>{d.conversion}</p> : null}
    <div className={styles.tableScroll} role="region" aria-label={d.tableRegion} tabIndex={0}>
      <table aria-describedby={tableNotes}>
        <caption>{caption}</caption>
        <thead><tr>
          <th scope="col">{d.size}</th><th scope="col">{d.length}</th><th scope="col">{d.shoulder}</th><th scope="col">{d.bust}</th><th scope="col">{d.sleeve}</th>
        </tr></thead>
        <tbody>{chart.rows.map((row) => <tr key={row.size} data-confirmation={chart.confirmationNeededSizes.includes(row.size) || undefined}>
          <th scope="row">{row.size}{chart.confirmationNeededSizes.includes(row.size) ? <abbr title={d.confirmationRequired}> *</abbr> : null}</th>
          <td>{measurement(row.lengthCm)}</td><td>{measurement(row.shoulderCm)}</td><td>{measurement(row.bustCm)}</td><td>{measurement(row.sleeveCm)}</td>
        </tr>)}</tbody>
      </table>
    </div>
    <p id={`${id}-bust`} className={styles.note}>{d.bustNote}</p>
    <p id={`${id}-tolerance`} className={styles.note}>{tolerance}</p>
    {chart.rows.some((row) => row.sleeveCm === null) ? <p className={styles.note}>{d.missingSleeve}</p> : null}
    {chart.warning ? <p id={`${id}-warning`} className={styles.warning}>{d[chart.warning]}</p> : null}
    <p className={styles.note}><a href={chart.sourceUrl} target="_blank" rel="noopener noreferrer">{d.source} · {chart.code}</a></p>
    <p className={styles.note}>{d.dated}</p>
  </section>;
}
