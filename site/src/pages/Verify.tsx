import { createSignal, For, Show } from "solid-js";
import sample from "../data/sample.json";
import {
	load,
	verifyDisclosure,
	type Sample,
	type VerifyReport,
} from "../lib/wasm";
import { NETWORKS, parseHeight, zec } from "../lib/format";
import { Field } from "../components/ui";

const fixture = sample as Sample;

const [disclosure, setDisclosure] = createSignal("");
const [tx, setTx] = createSignal("");
const [height, setHeight] = createSignal("");
const [network, setNetwork] = createSignal<string>("main");
const [report, setReport] = createSignal<VerifyReport | null>(null);
const [error, setError] = createSignal("");
const [busy, setBusy] = createSignal(false);

export default function Verify() {
	function loadExample() {
		setDisclosure(fixture.disclosure);
		setTx(fixture.tx);
		setHeight(String(fixture.height));
		setNetwork(fixture.network);
		setError("");
		setReport(null);
	}

	async function run() {
		setError("");
		setReport(null);
		if (!disclosure().trim() || !tx().trim()) {
			setError("Give a Disclosure and the transaction it claims to cover.");
			return;
		}
		const h = parseHeight(height());
		if (h === null) {
			setError("Give the block height as a whole number.");
			return;
		}
		setBusy(true);
		await load();
		const r = await verifyDisclosure(
			tx().trim(),
			h,
			network(),
			disclosure().trim(),
		);
		setBusy(false);
		if (typeof r === "string") {
			setError(r);
		} else {
			setReport(r);
		}
	}

	const allOk = () =>
		(report()?.items ?? []).length > 0 &&
		report()!.items.every((i) => i.status === "verified");

	return (
		<main class="narrow">
			<h1>Verify</h1>
			<p class="lede">
				Check that a disclosure matches it's corresponding transaction.
			</p>

			<div class="stack" style={{ "margin-top": "1.5rem" }}>
				<section>
					<div class="grid two">
						<Field label="Disclosure string">
							<textarea
								rows="5"
								placeholder="zdu1…"
								value={disclosure()}
								onInput={(e) => setDisclosure(e.currentTarget.value)}
								spellcheck={false}
							/>
						</Field>
						<Field label="Transaction, as hex or raw">
							<textarea
								rows="5"
								placeholder="04000080…"
								value={tx()}
								onInput={(e) => setTx(e.currentTarget.value)}
								spellcheck={false}
							/>
						</Field>
					</div>
					<div
						class="grid"
						style={{
							"grid-template-columns": "1fr 2fr",
							"margin-top": "0.8rem",
						}}
					>
						<Field label="Block height">
							<input
								type="number"
								value={height()}
								onInput={(e) => setHeight(e.currentTarget.value)}
							/>
						</Field>
						<Field label="Network">
							<select
								value={network()}
								onInput={(e) => setNetwork(e.currentTarget.value)}
							>
								<For each={[...NETWORKS]}>
									{(n) => <option value={n}>{n}</option>}
								</For>
							</select>
						</Field>
					</div>
					<div class="btn-row" style={{ "margin-top": "0.8rem" }}>
						<button
							class="btn primary"
							disabled={busy()}
							onClick={() => void run()}
						>
							Verify
						</button>
						<button class="btn" onClick={loadExample}>
							Load example
						</button>
					</div>
					<Show when={error()}>
						<p class="error-line" style={{ "margin-bottom": 0 }}>
							{error()}
						</p>
					</Show>
				</section>

				<Show when={report()}>
					{(r) => (
						<>
							<div class={`verdict entry ${allOk() ? "ok" : "fail"}`}>
								<span class="mark">{allOk() ? "✓" : "✕"}</span>
								<span>
									{allOk()
										? `Verified. Every Item of ${r().items.length} matches the transaction.`
										: "Failed. This Disclosure does not fully match the transaction."}
								</span>
							</div>
							<For each={r().items}>
								{(item, i) => (
									<section class="result entry" style={{ "--i": i() }}>
										<div
											style={{
												display: "flex",
												gap: "0.6rem",
												"align-items": "center",
											}}
										>
											<span class="mark">
												{item.status === "verified" ? "✓" : "✕"}
											</span>
											<span class="mono" style={{ "font-size": "0.85rem" }}>
												output {item.index}
											</span>
											<span
												class="mono"
												style={{
													"margin-left": "auto",
													"font-size": "0.85rem",
													color:
														item.status === "verified"
															? "var(--ok)"
															: "var(--fail)",
												}}
											>
												{item.status === "verified" ? "verified" : "FAILED"}
											</span>
										</div>
										{item.status === "verified" ? (
											<dl class="kv" style={{ "margin-top": "0.6rem" }}>
												<dt>value</dt>
												<dd>{zec(item.value)}</dd>
												<dt>address</dt>
												<dd>{item.address}</dd>
												<dt>memo</dt>
												<dd>{item.memo}</dd>
											</dl>
										) : (
											<p class="error-line" style={{ margin: "0.6rem 0 0" }}>
												{item.error}
											</p>
										)}
									</section>
								)}
							</For>
						</>
					)}
				</Show>
			</div>
		</main>
	);
}
