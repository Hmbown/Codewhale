/**
 * A translated label that ends in a direction arrow ("Install →"). The arrow
 * is decoration, so screen readers hear only the words.
 */
export function ArrowLabel({ text }: { text: string }) {
  const match = text.match(/^(.*?)\s*([→←])\s*$/u);
  if (!match) return <>{text}</>;
  return (
    <>
      {match[1]}
      <span aria-hidden="true" style={{ marginInlineStart: "0.35em" }}>{match[2]}</span>
    </>
  );
}
