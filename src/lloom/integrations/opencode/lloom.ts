/**
 * lloom for OpenCode: proactive "mail is waiting" notices.
 *
 * Installed by `lloom skills install --agent opencode --with-push`. The
 * plugin runs `lloom watch --format jsonl` in the project directory: it
 * collects mail into the project's `.lloom/mail/new/` and reports counts
 * of what is waiting. On a waking signal it
 * prompts the current session with the watcher's fixed sentence, at once
 * when the session is idle and on `session.idle` when it is busy. A
 * non-waking signal goes in as context only (`noReply`). Every signal also
 * shows a toast.
 *
 * The injected text carries counts, never message bodies. The agent reads
 * the mail itself (`lloom poll` / `check_mailbox`, which read the maildir
 * while the watcher runs) under the lloom-receive
 * rules.
 *
 * Environment: LLOOM_BIN (default "lloom"), LLOOM_WATCH_WAKE
 * (private|all|none, default private), LLOOM_PUSH=0 to disable.
 */
import { spawn, type ChildProcess } from "node:child_process";
import { createInterface } from "node:readline";
import type { Plugin } from "@opencode-ai/plugin";

interface WatchEvent {
	v: number;
	type: "signal" | "status";
	pending?: number;
	wake?: boolean;
	state?: string;
	message?: string;
	text?: string;
}

export const LloomPlugin: Plugin = async ({ client, directory }) => {
	if (process.env.LLOOM_PUSH === "0") return {};

	let sessionID: string | undefined;
	let busy = false;
	let queued: WatchEvent | undefined; // newest waking signal, held while busy
	let child: ChildProcess | undefined;

	const toast = (message: string, variant: "info" | "warning" = "info") =>
		client.tui.showToast({ body: { title: "lloom", message, variant } }).catch(() => {});

	const deliver = async (ev: WatchEvent, wake: boolean) => {
		const text = ev.text!;
		if (!sessionID) {
			if (!wake) return;
			// no session seen yet: type it into whatever the TUI has open
			await client.tui.appendPrompt({ body: { text } }).catch(() => {});
			await client.tui.submitPrompt().catch(() => {});
			return;
		}
		await client.session
			.promptAsync({
				path: { id: sessionID },
				body: { noReply: !wake, parts: [{ type: "text", text }] },
			})
			.catch(() => {});
	};

	const onLine = async (line: string) => {
		let ev: WatchEvent;
		try {
			ev = JSON.parse(line);
		} catch {
			return;
		}
		if (ev.type === "status") {
			if (ev.state !== "ready") await toast(ev.message ?? "watch status changed", "warning");
			return;
		}
		if (ev.type !== "signal" || !ev.text) return;
		await toast(`${ev.pending ?? 0} message(s) waiting`);
		if (ev.wake && busy) {
			queued = ev; // a later signal supersedes: counts are cumulative
			return;
		}
		await deliver(ev, Boolean(ev.wake));
	};

	const bin = process.env.LLOOM_BIN || "lloom";
	const wake = process.env.LLOOM_WATCH_WAKE || "private";
	child = spawn(bin, ["watch", "--format", "jsonl", "--wake", wake], {
		cwd: directory,
		stdio: ["ignore", "pipe", "pipe"],
	});
	child.on("error", (err) => void toast(`lloom watch could not start: ${err.message}`, "warning"));
	createInterface({ input: child.stdout! }).on("line", (line) => void onLine(line));

	return {
		event: async ({ event }) => {
			if (event.type === "session.created" && !event.properties.info.parentID) {
				sessionID = event.properties.info.id;
			} else if (event.type === "message.updated") {
				sessionID ??= event.properties.info.sessionID;
			} else if (event.type === "session.status") {
				sessionID = event.properties.sessionID;
				busy = event.properties.status.type !== "idle";
			} else if (event.type === "session.idle") {
				busy = false;
				if (queued && event.properties.sessionID === sessionID) {
					const ev = queued;
					queued = undefined;
					await deliver(ev, true);
				}
			}
		},
		dispose: async () => {
			child?.kill();
			child = undefined;
		},
	};
};
