/**
 * lloom for Pi: proactive "mail is waiting" notices.
 *
 * Installed by `lloom skills install --agent pi --with-push`. On session
 * start it runs `lloom watch --format jsonl` in the project directory (it
 * collects mail into `.lloom/mail/new/` and reports counts of what is
 * waiting) and turns each signal into a
 * custom message. A waking signal (newly arrived private mail, within the
 * wake budget) starts a turn; anything else rides along with the next one.
 *
 * The injected text is the watcher's own fixed sentence. It carries counts,
 * never message bodies, so nothing another agent wrote reaches the model
 * this way. The agent reads the mail itself with `lloom poll` /
 * `check_mailbox` (both read the maildir while the watcher runs), under the
 * lloom-receive rules.
 *
 * Environment: LLOOM_BIN (default "lloom"), LLOOM_WATCH_WAKE
 * (private|all|none, default private), LLOOM_PUSH=0 to disable.
 */
import { spawn, type ChildProcess } from "node:child_process";
import { createInterface } from "node:readline";
import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";

interface WatchEvent {
	v: number;
	type: "signal" | "status";
	pending?: number;
	private?: number;
	broadcast?: number;
	wake?: boolean;
	reason?: string;
	state?: string;
	message?: string;
	text?: string;
}

const MAX_RESPAWNS = 3;

export default function (pi: ExtensionAPI) {
	let child: ChildProcess | undefined;
	let respawns = 0;
	let stopping = false;

	const status = (ctx: ExtensionContext, ev: WatchEvent) => {
		if (!ctx.hasUI) return;
		const n = ev.pending ?? 0;
		ctx.ui.setStatus("lloom", n > 0 ? `✉ lloom ${n}` : undefined);
	};

	const start = (ctx: ExtensionContext) => {
		if (process.env.LLOOM_PUSH === "0" || child) return;
		const bin = process.env.LLOOM_BIN || "lloom";
		const wake = process.env.LLOOM_WATCH_WAKE || "private";
		const proc = spawn(bin, ["watch", "--format", "jsonl", "--wake", wake], {
			cwd: ctx.cwd,
			stdio: ["ignore", "pipe", "pipe"],
		});
		child = proc;
		proc.on("error", (err) => {
			if (ctx.hasUI) ctx.ui.notify(`lloom watch could not start: ${err.message}`, "warning");
		});
		createInterface({ input: proc.stdout! }).on("line", (line) => {
			let ev: WatchEvent;
			try {
				ev = JSON.parse(line);
			} catch {
				return;
			}
			if (ev.type === "status") {
				if (ev.state !== "ready" && ctx.hasUI) ctx.ui.notify(`lloom: ${ev.message}`, "warning");
				return;
			}
			if (ev.type !== "signal" || !ev.text) return;
			status(ctx, ev);
			void pi.sendMessage(
				{ customType: "lloom", content: ev.text, display: true },
				ev.wake ? { triggerTurn: true, deliverAs: "followUp" } : { deliverAs: "nextTurn" },
			);
		});
		proc.on("exit", (code) => {
			child = undefined;
			if (stopping) return;
			// 2 = credentials: retrying cannot help
			if (code === 2 || respawns >= MAX_RESPAWNS) {
				if (ctx.hasUI) ctx.ui.notify(`lloom watch stopped (exit ${code}); notices are off`, "warning");
				return;
			}
			respawns += 1;
			setTimeout(() => start(ctx), 2000 * respawns);
		});
	};

	pi.on("session_start", async (_event, ctx) => {
		stopping = false;
		start(ctx);
	});

	pi.on("session_shutdown", async () => {
		stopping = true;
		child?.kill();
		child = undefined;
	});
}
