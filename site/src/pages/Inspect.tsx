import { createSignal, For, Show } from "solid-js";
import sample from "../data/sample.json";
import {
	decodeDisclosure,
	load,
	verifyDisclosure,
	type DecodeReport,
	type Sample,
	type VerifyReport,
	type VerifyItem,
} from "../lib/wasm";
import { NETWORKS, parseHeight, shortHex, zec } from "../lib/format";
import { CopyButton, Field } from "../components/ui";

const fixture = sample as Sample;

// Module scope: the tab switcher unmounts pages, and the work on them survives.
const [text, setText] = createSignal("");
const [report, setReport] = createSignal<DecodeReport | null>(null);
const [error, setError] = createSignal("");
const [busy, setBusy] = createSignal(false);
const [checking, setChecking] = createSignal(false);
const [network, setNetwork] = createSignal<string>("main");
const [height, setHeight] = createSignal("");
const [tx, setTx] = createSignal("");
const [verified, setVerified] = createSignal<VerifyReport | null>(null);
const [checkError, setCheckError] = createSignal("");

export default function Inspect() {
	async function run(input?: string) {
		const source = (input ?? text()).trim();
		if (input !== undefined) setText(input);
		setError("");
		setReport(null);
		setVerified(null);
		if (!source) {
			setError("Give a disclosure string, or load the example.");
			return;
		}
		setBusy(true);
		await load();
		const r = decodeDisclosure(source);
		setBusy(false);
		if (typeof r === "string") {
			setError(r);
		} else {
			setReport(r);
			setNetwork(r.network);
		}
	}

	function loadExample() {
		void run(fixture.disclosure);
		setChecking(false);
	}

	async function check() {
		setCheckError("");
		setVerified(null);
		const h = parseHeight(height());
		if (h === null) {
			setCheckError("Give the block height as a whole number.");
			return;
		}
		setBusy(true);
		await load();
		const r = verifyDisclosure(tx().trim(), h, network(), text().trim());
		setBusy(false);
		if (typeof r === "string") {
			setCheckError(r);
		} else {
			setVerified(r);
		}
	}

	const resultFor = (index: number): VerifyItem | undefined =>
		verified()?.items.find((i) => i.index === index);
	const allOk = (v: VerifyReport) =>
		v.items.every((i) => i.status === "verified");
	const opened = (v: VerifyItem) => (v.status === "verified" ? v : undefined);
	const verdict = (v: VerifyItem) =>
		v.status === "verified" ? "verified" : `FAILED: ${v.error}`;

	return (
		<main class="narrow">
			<h1>Inspect</h1>
			<p class="lede">See what a disclosure contains</p>

			<div class="stack" style={{ "margin-top": "1.5rem" }}>
				<section>
					<Field label="Disclosure string, or a whole .zdisc line">
						<textarea
							rows="5"
							placeholder="zdu1…"
							value={text()}
							onInput={(e) => setText(e.currentTarget.value)}
							spellcheck={false}
						/>
					</Field>
					<div class="btn-row" style={{ "margin-top": "0.8rem" }}>
						<button
							class="btn primary"
							disabled={busy()}
							onClick={() => void run()}
						>
							Decode
						</button>
						<button class="btn" onClick={loadExample}>
							Load example
						</button>
						<Show when={report()}>
							<button class="btn" onClick={() => setChecking(!checking())}>
								{checking()
									? "Hide transaction check"
									: "Check against transaction"}
							</button>
						</Show>
					</div>
					<Show when={error()}>
						<p class="error-line" style={{ "margin-bottom": 0 }}>
							{error()}
						</p>
					</Show>
				</section>

				<Show when={checking()}>
					<section class="result entry">
						<div class="grid" style={{ "grid-template-columns": "1fr 1fr" }}>
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
							<Field label="Block height">
								<input
									type="number"
									value={height()}
									onInput={(e) => setHeight(e.currentTarget.value)}
								/>
							</Field>
						</div>
						<div style={{ "margin-top": "0.8rem" }}>
							<Field label="Transaction, as hex or raw">
								<textarea
									rows="4"
									placeholder="04000080…"
									value={tx()}
									onInput={(e) => setTx(e.currentTarget.value)}
									spellcheck={false}
								/>
							</Field>
						</div>
						<div class="btn-row" style={{ "margin-top": "0.8rem" }}>
							<button
								class="btn primary"
								disabled={busy()}
								onClick={() => void check()}
							>
								Check
							</button>
							<button
								class="btn"
								onClick={() => {
									setNetwork(fixture.network);
									setHeight(String(fixture.height));
									setTx(fixture.tx);
								}}
							>
								Use the example transaction
							</button>
						</div>
						<Show when={checkError()}>
							<p class="error-line" style={{ "margin-bottom": 0 }}>
								{checkError()}
							</p>
						</Show>
					</section>
				</Show>

				<Show when={verified()}>
					{(v) => (
						<div class={`verdict entry ${allOk(v()) ? "ok" : "fail"}`}>
							<span class="mark">{allOk(v()) ? "✓" : "✕"}</span>
							<span>
								{allOk(v())
									? `All ${v().items.length} Items of this Disclosure match the transaction.`
									: "At least one Item does not match the transaction."}
							</span>
						</div>
					)}
				</Show>

				<Show when={report()}>
					{(r) => (
						<>
							<section class="result entry">
								<dl class="kv">
									<dt>network</dt>
									<dd>{r().network}</dd>
									<dt>txid</dt>
									<dd>
										{r().txid} <CopyButton text={() => r().txid} />
									</dd>
									<dt>items</dt>
									<dd>{r().items.length}</dd>
								</dl>
							</section>

							<For each={r().items}>
								{(item, i) => {
									const res = () => resultFor(item.index);
									return (
										<section class="result entry" style={{ "--i": i() }}>
											<h3
												style={{
													margin: "0 0 0.6rem",
													display: "flex",
													gap: "0.6rem",
													"align-items": "center",
												}}
											>
												<span class="mono" style={{ "font-size": "0.85rem" }}>
													output {item.index}
												</span>
												<span class="note mono">type 0x0{item.typecode}</span>
											</h3>
											<dl class="kv">
												<dt>type</dt>
												<dd>{item.type}</dd>
												<dt>pk_d</dt>
												<dd>
													{item.pk_d} <CopyButton text={() => item.pk_d} />
												</dd>
												<dt>secret</dt>
												<dd>
													{item.secret} <CopyButton text={() => item.secret} />
												</dd>
											</dl>
											<Show when={res()}>
												{(res) => (
													<>
														<div
															class={`verdict ${res().status === "verified" ? "ok" : "fail"}`}
															style={{
																"margin-top": "0.8rem",
																padding: "0.6rem 0.9rem",
																"font-size": "0.9rem",
															}}
														>
															<span class="mark">
																{res().status === "verified" ? "✓" : "✕"}
															</span>
															<span>{verdict(res())}</span>
														</div>
														<Show when={opened(res())}>
															{(note) => (
																<dl
																	class="kv"
																	style={{ "margin-top": "0.6rem" }}
																>
																	<dt>value</dt>
																	<dd>{zec(note().value)}</dd>
																	<dt>address</dt>
																	<dd>
																		{note().address}{" "}
																		<CopyButton text={() => note().address} />
																	</dd>
																	<dt>memo</dt>
																	<dd>{note().memo}</dd>
																	<dt>receiver-decryptable</dt>
																	<dd>
																		{note().receiver_decryptable ? "yes" : "no"}
																	</dd>
																</dl>
															)}
														</Show>
													</>
												)}
											</Show>
										</section>
									);
								}}
							</For>

							<For each={r().unknown_items}>
								{(u) => (
									<section class="result entry">
										<h3 style={{ margin: "0 0 0.6rem" }}>
											Skipped unknown typecode
										</h3>
										<p class="note" style={{ margin: 0 }}>
											typecode{" "}
											<span class="mono">0x{u.typecode.toString(16)}</span>,
											body <span class="mono">{shortHex(u.body, 24, 16)}</span>
										</p>
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
