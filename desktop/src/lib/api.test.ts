import { beforeEach, expect, it, vi } from "vitest";

import { invoke } from "@tauri-apps/api/core";
import { discoverCodex } from "./api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

beforeEach(() => vi.mocked(invoke).mockReset());

it("passes the selected Codex folder using the Tauri command argument name", async () => {
  await discoverCodex("F:\\isolated\\data");
  expect(invoke).toHaveBeenCalledWith("discover_codex", { codexHome: "F:\\isolated\\data" });
});

it("requests default discovery only when no Codex folder was selected", async () => {
  await discoverCodex();
  expect(invoke).toHaveBeenCalledWith("discover_codex", { codexHome: null });
});
