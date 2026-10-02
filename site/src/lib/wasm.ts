import init, {
  create,
  decode,
  verify,
  type CreateReport,
  type DecodeReport,
  type VerifyReport,
} from "../wasm/zdisclosure_wasm.js";
import wasmUrl from "../wasm/zdisclosure_wasm_bg.wasm?url";

export type {
  CreateReport,
  DecodeReport,
  OutputInfo,
  VerifyItem,
  VerifyReport,
} from "../wasm/zdisclosure_wasm.js";

export interface Sample {
  network: string;
  height: number;
  tx: string;
  receiverUfvk: string;
  senderUfvk: string;
  disclosure: string;
}

let ready: Promise<unknown> | null = null;

/** Loads the crypto module once. Every tool call after the first resolves immediately. */
export function load(): Promise<unknown> {
  ready ??= init(wasmUrl);
  return ready;
}

function run<T>(fn: () => T): T | string {
  try {
    return fn();
  } catch (e) {
    return e instanceof Error ? e.message : String(e);
  }
}

export function decodeDisclosure(disclosure: string): DecodeReport | string {
  return run(() => decode(disclosure));
}

export function verifyDisclosure(
  tx: string,
  height: number,
  network: string,
  disclosure: string,
): VerifyReport | string {
  return run(() =>
    verify(new TextEncoder().encode(tx), height, network, disclosure),
  );
}

export function createDisclosure(
  ufvk: string,
  tx: string,
  height: number,
  network: string,
  allowInternal: boolean,
  selected: number[],
): CreateReport | string {
  return run(() =>
    create(
      ufvk,
      new TextEncoder().encode(tx),
      height,
      network,
      false,
      allowInternal,
      new Uint32Array(selected),
    ),
  );
}
