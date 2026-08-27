import assert from "node:assert/strict";
import test from "node:test";

import { IRON_VAULT_PROGRAM_ID, PDA_SEEDS } from "../dist/index.js";

test("exports the canonical program address", () => {
  assert.equal(
    IRON_VAULT_PROGRAM_ID,
    "2UWmTuefm4gqbfuZP36NSJMMSKLM4Rbop25jf1uBZAu1",
  );
});
test("exports every specified PDA namespace", () => {
  assert.deepEqual(Object.values(PDA_SEEDS), [
    "protocol",
    "escrow",
    "escrow_token",
    "vault",
    "vault_asset",
    "vault_token",
    "role",
    "withdrawal",
  ]);
  assert.ok(Object.isFrozen(PDA_SEEDS));
});
