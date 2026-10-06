/** Zatoshis as a ZEC amount without trailing zeros, e.g. 1000000n → "0.01 ZEC". */
export function zec(zatoshis: bigint): string {
  const whole = zatoshis / 100_000_000n;
  const frac = String(zatoshis % 100_000_000n)
    .padStart(8, "0")
    .replace(/0+$/, "");
  return frac ? `${whole}.${frac} ZEC` : `${whole} ZEC`;
}

/** Head…tail for long hex, left alone when it already fits. */
export function shortHex(s: string, head = 16, tail = 10): string {
  return s.length <= head + tail + 1 ? s : `${s.slice(0, head)}…${s.slice(-tail)}`;
}

/** A block height from a form field. Null for an empty field or a value that is not a whole number. */
export function parseHeight(text: string): number | null {
  const h = Number(text);
  return text.trim() !== "" && Number.isInteger(h) && h >= 0 ? h : null;
}

export const NETWORKS = ["main", "test", "regtest"] as const;

export function hrpOf(network: string): string {
  return network === "main"
    ? "zdu"
    : network === "test"
      ? "zdutest"
      : "zduregtest";
}

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
