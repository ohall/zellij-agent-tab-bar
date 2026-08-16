/**
 * Reports opencode session activity to the agent-aware Zellij tab bar
 * (zellij-agent-tab-bar) via the `zja` CLI, which publishes status events on
 * the `zja.events` named pipe.
 *
 * Maps opencode bus events to `zja status` states:
 *   - session busy / retry            -> running
 *   - session idle after success      -> complete
 *   - session.error / idle after fail -> error
 *
 * Only runs inside Zellij: it awaits the `ZELLIJ_PANE_ID` env var, which `zja`
 * uses to resolve the target pane. Outside Zellij it is a no-op.
 */
import { spawn } from "node:child_process";
import type { Plugin } from "@opencode-ai/plugin";

const AGENT_ID = "opencode";

let generation: number | null = null;
let sequences = new Map<number, number>();
let lastRunFailed = false;

function report(state: "running" | "complete" | "error") {
  if (!process.env.ZELLIJ_PANE_ID) {
    return;
  }

  if (state === "running") {
    generation = Date.now();
    sequences.set(generation, 0);
  }

  if (generation == null) {
    return;
  }

  const seq = (sequences.get(generation) ?? 0) + 1;
  sequences.set(generation, seq);

  const args = [
    "status",
    state,
    "--agent-id",
    AGENT_ID,
    "--generation",
    String(generation),
    "--sequence",
    String(seq),
  ];
  spawn("zja", args, { stdio: "ignore" }).on("error", () => {
    // zja missing or unusable; do not disturb the agent.
  });

  if (state !== "running") {
    generation = null;
  }
}

function onIdle() {
  report(lastRunFailed ? "error" : "complete");
  lastRunFailed = false;
}

export default (async () => {
  return {
    event: async ({ event }: { event: any }) => {
      switch (event.type) {
        case "session.status": {
          const kind = event.properties?.status?.type;
          if (kind === "busy" || kind === "retry") {
            lastRunFailed = false;
            report("running");
          } else if (kind === "idle") {
            onIdle();
          }
          break;
        }
        case "session.idle":
          onIdle();
          break;
        case "session.error":
          lastRunFailed = true;
          report("error");
          break;
        default:
          break;
      }
    },
  };
}) satisfies Plugin;