import { beforeEach, expect, it, vi } from "vitest";

import { invoke } from "@tauri-apps/api/core";
import { discoverCodex, getMigrationJob, startMigrateAndConnect } from "./api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

beforeEach(() => vi.mocked(invoke).mockReset());

it("starts migration with only bounded selection fields", async () => {
  await startMigrateAndConnect({
    plan_id: "plan-1", codex_closed_confirmed: true, register_projects: true,
    probe_thread_id: "target-1", online_usage_confirmed: true,
  });
  expect(invoke).toHaveBeenCalledWith("start_migrate_and_connect", {
    selection: { plan_id: "plan-1", codex_closed_confirmed: true, register_projects: true,
      probe_thread_id: "target-1", online_usage_confirmed: true },
  });
});

it("polls the migration job using the Tauri camelCase parameter", async () => {
  await getMigrationJob("job-1");
  expect(invoke).toHaveBeenCalledWith("get_migration_job", { jobId: "job-1" });
});

it("passes the selected Codex folder using the Tauri command argument name", async () => {
  await discoverCodex("F:\\isolated\\data");
  expect(invoke).toHaveBeenCalledWith("discover_codex", { codexHome: "F:\\isolated\\data" });
});

it("requests default discovery only when no Codex folder was selected", async () => {
  await discoverCodex();
  expect(invoke).toHaveBeenCalledWith("discover_codex", { codexHome: null });
});
