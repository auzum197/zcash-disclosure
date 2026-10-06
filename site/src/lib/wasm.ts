import type {
	CreateReport,
	DecodeReport,
	VerifyReport,
} from "../wasm/zdisclosure_wasm.js";

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

type Report = DecodeReport | VerifyReport | CreateReport;

export type Request =
	| { kind: "call"; id: number; op: "decode"; args: [disclosure: string] }
	| {
			kind: "call";
			id: number;
			op: "verify";
			args: [tx: string, height: number, network: string, disclosure: string];
	  }
	| {
			kind: "call";
			id: number;
			op: "create";
			args: [
				ufvk: string,
				tx: string,
				height: number,
				network: string,
				allowInternal: boolean,
				selected: number[],
			];
	  };

export type Reply =
	| { kind: "ready" }
	| { kind: "ok"; id: number; result: Report }
	| { kind: "err"; id: number; message: string };

let seq = 0;
const pending = new Map<number, (r: Report | string) => void>();
let worker: Worker | null = null;
let ready: Promise<unknown> | null = null;

/**
 * Starts the crypto worker and loads its module once. Every tool call after the first
 * resolves immediately. The decryption runs on the worker's thread, so the page stays
 * responsive.
 */
export function load(): Promise<unknown> {
	ready ??= new Promise((resolve, reject) => {
		worker = new Worker(new URL("./wasm-worker.ts", import.meta.url), {
			type: "module",
		});
		worker.onmessage = (e: MessageEvent<Reply>) => {
			const reply = e.data;
			if (reply.kind === "ready") {
				resolve(undefined);
				return;
			}
			const settle = pending.get(reply.id);
			if (settle !== undefined) {
				pending.delete(reply.id);
				settle(reply.kind === "ok" ? reply.result : reply.message);
			}
		};
		worker.onerror = (e) => {
			reject(new Error(`the crypto worker failed to load: ${e.message}`));
		};
	});
	return ready;
}

function call(op: "decode", args: [string]): Promise<DecodeReport | string>;
function call(
	op: "verify",
	args: [string, number, string, string],
): Promise<VerifyReport | string>;
function call(
	op: "create",
	args: [string, string, number, string, boolean, number[]],
): Promise<CreateReport | string>;
async function call(op: string, args: unknown[]): Promise<Report | string> {
	await load();
	const id = seq++;
	return new Promise((resolve) => {
		pending.set(id, resolve);
		worker?.postMessage({ kind: "call", id, op, args });
	});
}

export function decodeDisclosure(
	disclosure: string,
): Promise<DecodeReport | string> {
	return call("decode", [disclosure]);
}

export function verifyDisclosure(
	tx: string,
	height: number,
	network: string,
	disclosure: string,
): Promise<VerifyReport | string> {
	return call("verify", [tx, height, network, disclosure]);
}

export function createDisclosure(
	ufvk: string,
	tx: string,
	height: number,
	network: string,
	allowInternal: boolean,
	selected: number[],
): Promise<CreateReport | string> {
	return call("create", [ufvk, tx, height, network, allowInternal, selected]);
}
