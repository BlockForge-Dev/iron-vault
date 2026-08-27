import assert from "node:assert/strict";
import test from "node:test";

import {
  IRON_VAULT_PROGRAM_ID,
  PDA_SEEDS,
  PROTOCOL_PAUSE_FLAGS,
  TOKEN_2022_POLICY,
  TOKEN_PROGRAM_IDS,
  VAULT_PERMISSIONS,
} from "../dist/index.js";

test("exports the canonical program address", () => {
  assert.equal(
    IRON_VAULT_PROGRAM_ID,
    "2UWmTuefm4gqbfuZP36NSJMMSKLM4Rbop25jf1uBZAu1",
  );
});

test("exports the exact fail-closed token policy", () => {
  assert.deepEqual(TOKEN_PROGRAM_IDS, {
    legacy: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
    token2022: "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
  });
  assert.deepEqual(TOKEN_2022_POLICY, { allowMintExtensions: false });
  assert.ok(Object.isFrozen(TOKEN_PROGRAM_IDS));
  assert.ok(Object.isFrozen(TOKEN_2022_POLICY));
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

test("exports the exact reserved vault permission bits", () => {
  assert.deepEqual(Object.values(VAULT_PERMISSIONS), [1n, 2n, 4n, 8n, 16n, 32n]);
  assert.ok(Object.isFrozen(VAULT_PERMISSIONS));
});

test("exports the exact directional protocol pause bits", () => {
  assert.deepEqual(Object.values(PROTOCOL_PAUSE_FLAGS), [1, 2, 4, 8]);
  assert.ok(Object.isFrozen(PROTOCOL_PAUSE_FLAGS));
});
