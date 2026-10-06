import init, { create, decode, verify } from "../wasm/zdisclosure_wasm.js";
import wasmUrl from "../wasm/zdisclosure_wasm_bg.wasm?url";
import type { CreateReport, DecodeReport, VerifyReport } from "../wasm/zdisclosure_wasm.js";
import type { Reply, Request } from "./wasm";

const ctx = self as unknown as {
	postMessage(message: Reply): void;
	onmessage: ((event: MessageEvent<Request>) => void) | null;
};

const enc = new TextEncoder();

function dispatch(request: Request): DecodeReport | VerifyReport | CreateReport {
	switch (request.op) {
		case "decode": {
			const [disclosure] = request.args;
			return decode(disclosure);
		}
		case "verify": {
			const [tx, height, network, disclosure] = request.args;
			return verify(enc.encode(tx), height, network, disclosure);
		}
		case "create": {
			const [ufvk, tx, height, network, allowInternal, selected] = request.args;
			return create(
				ufvk,
				enc.encode(tx),
				height,
				network,
				false,
				allowInternal,
				new Uint32Array(selected),
			);
		}
	}
}

async function main(): Promise<void> {
	await init(wasmUrl);
	ctx.postMessage({ kind: "ready" });
	ctx.onmessage = (event) => {
		const { id } = event.data;
		let reply: Reply;
		try {
			reply = { kind: "ok", id, result: dispatch(event.data) };
		} catch (e) {
			reply = {
				kind: "err",
				id,
				message: e instanceof Error ? e.message : String(e),
			};
		}
		ctx.postMessage(reply);
	};
}

void main();
