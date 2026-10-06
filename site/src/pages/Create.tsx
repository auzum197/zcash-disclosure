import { createSignal, For, Show } from "solid-js";
import {
	createDisclosure,
	load,
	type CreateReport,
	type OutputInfo,
} from "../lib/wasm";
import { NETWORKS, parseHeight, zec } from "../lib/format";
import { CopyButton, Field } from "../components/ui";

const [ufvk, setUfvk] = createSignal("");
const [tx, setTx] = createSignal("");
const [height, setHeight] = createSignal("");
const [network, setNetwork] = createSignal<string>("main");
const [allowInternal, setAllowInternal] = createSignal(false);
const [selected, setSelected] = createSignal<Set<number>>(new Set());
const [report, setReport] = createSignal<CreateReport | null>(null);
const [error, setError] = createSignal("");
const [busy, setBusy] = createSignal(false);

export default function Create() {
	async function list() {
		setError("");
		setReport(null);
		if (!ufvk().trim() || !tx().trim()) {
			setError("Give a viewing key and the transaction.");
			return;
		}
		const h = parseHeight(height());
		if (h === null) {
			setError("Give the block height as a whole number.");
			return;
		}
		setBusy(true);
		await load();
		const r = await createDisclosure(
			ufvk().trim(),
			tx().trim(),
			h,
			network(),
			allowInternal(),
			[],
		);
		setBusy(false);
		if (typeof r === "string") {
			setError(r);
		} else {
			setReport(r);
			setSelected(
				new Set<number>(
					r.outputs.filter((o) => o.scope === "external").map((o) => o.index),
				),
			);
		}
	}

	async function build() {
		setError("");
		const h = parseHeight(height());
		if (h === null) {
			setError("Give the block height as a whole number.");
			return;
		}
		setBusy(true);
		await load();
		const r = await createDisclosure(
			ufvk().trim(),
			tx().trim(),
			h,
			network(),
			allowInternal(),
			[...selected()],
		);
		setBusy(false);
		if (typeof r === "string") {
			setError(r);
		} else {
			setReport(r);
		}
	}

	const chosen = (o: OutputInfo) => selected().has(o.index);

	function toggle(o: OutputInfo) {
		const next = new Set(selected());
		if (next.has(o.index)) {
			next.delete(o.index);
		} else {
			next.add(o.index);
		}
		setSelected(next);
	}

	return (
		<main class="narrow">
			<h1>Create</h1>
			<p class="lede">
				Paste a viewing key and a transaction, and select which payments you
				want to disclose
			</p>

			<div class="stack" style={{ "margin-top": "1.5rem" }}>
				<section>
					<div class="stack">
						<Field label="Unified full viewing key">
							<textarea
								rows="3"
								placeholder="uview1…"
								value={ufvk()}
								onInput={(e) => setUfvk(e.currentTarget.value)}
								spellcheck={false}
							/>
						</Field>
						<div class="grid" style={{ "grid-template-columns": "2fr 1fr" }}>
							<Field label="Transaction, as hex or raw">
								<textarea
									rows="3"
									placeholder="04000080…"
									value={tx()}
									onInput={(e) => setTx(e.currentTarget.value)}
									spellcheck={false}
								/>
							</Field>
							<div class="stack">
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
						</div>
					</div>
					<div class="btn-row" style={{ "margin-top": "0.8rem" }}>
						<button
							class="btn primary"
							disabled={busy()}
							onClick={() => void list()}
						>
							List outputs
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
							<Show when={r().outputs.length > 0}>
								<section class="result entry">
									<table class="outputs">
										<thead>
											<tr>
												<th></th>
												<th>index</th>
												<th>role</th>
												<th>scope</th>
												<th style={{ "text-align": "right" }}>value</th>
												<th></th>
											</tr>
										</thead>
										<tbody>
											<For each={r().outputs}>
												{(o) => {
													const internal = o.scope === "internal";
													return (
														<tr>
															<td>
																<input
																	type="checkbox"
																	checked={chosen(o)}
																	disabled={internal && !allowInternal()}
																	onChange={() => toggle(o)}
																/>
															</td>
															<td class="mono">{o.index}</td>
															<td>{o.role}</td>
															<td>
																<Show
																	when={!internal}
																	fallback={
																		<span class="badge internal">internal</span>
																	}
																>
																	{o.scope}
																</Show>
															</td>
															<td style={{ "text-align": "right" }}>
																{zec(o.value)}
															</td>
															<td>
																<Show when={o.secret_reused}>
																	<span class="badge warn">secret-reused</span>
																</Show>
															</td>
														</tr>
													);
												}}
											</For>
										</tbody>
									</table>
									<div
										class="stack"
										style={{
											"margin-top": "0.9rem",
											"align-items": "baseline",
										}}
									>
										<label
											style={{
												display: "flex",
												gap: "0.5rem",
												"align-items": "baseline",
												"font-size": "0.9rem",
											}}
										>
											<input
												type="checkbox"
												style={{
													width: "1rem",
													height: "1rem",
													"accent-color": "var(--accent)",
												}}
												checked={allowInternal()}
												onInput={(e) =>
													setAllowInternal(e.currentTarget.checked)
												}
											/>
											Allow internal-scope outputs
										</label>
										<p class="note" style={{ margin: 0 }}>
											Internal outputs are change. An Item for one links to
											every other change output of the account, and exposes the
											internal scope to a discrete-log adversary.
										</p>
									</div>
									<div class="btn-row" style={{ "margin-top": "0.9rem" }}>
										<button
											class="btn primary"
											disabled={busy() || selected().size === 0}
											onClick={() => void build()}
										>
											Create disclosure from {selected().size} output
											{selected().size === 1 ? "" : "s"}
										</button>
									</div>
								</section>
							</Show>

							<For each={r().warnings}>
								{(w) => (
									<p class="warning-line entry" style={{ margin: 0 }}>
										{w}
									</p>
								)}
							</For>

							<Show when={r().disclosure}>
								{(d) => (
									<section class="result entry">
										<div
											style={{
												display: "flex",
												"align-items": "center",
												"margin-bottom": "0.6rem",
											}}
										>
											<h3 style={{ margin: 0 }}>Disclosure</h3>
											<span style={{ "margin-left": "auto" }}>
												<CopyButton text={() => d()} />
											</span>
										</div>
										<p class="codeblock">{d()}</p>
									</section>
								)}
							</Show>
						</>
					)}
				</Show>
			</div>
		</main>
	);
}
