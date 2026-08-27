import assert from "node:assert/strict";
import test from "node:test";
import { parseInvocation, USAGE } from "../dist/index.js";

test("parses a documented escrow create command", () => {
  assert.deepEqual(parseInvocation([
    "escrow", "create", "--recipient", "recipient", "--mint", "mint",
    "--amount", "100", "--expires-at", "2030-01-01T00:00:00Z", "--json",
  ]), {
    group: "escrow",
    action: "create",
    options: {
      recipient: "recipient", mint: "mint", amount: "100",
      "expires-at": "2030-01-01T00:00:00Z", json: true,
    },
  });
});

test("rejects missing and duplicate option values", () => {
  assert.throws(() => parseInvocation(["vault", "deposit", "--amount"]), /missing value/);
  assert.throws(
    () => parseInvocation(["vault", "deposit", "--amount", "1", "--amount", "2"]),
    /duplicate option/,
  );
});

test("help includes every milestone command", () => {
  for (const command of [
    "escrow create", "escrow release", "escrow refund", "vault create",
    "vault deposit", "withdrawal request",
  ]) assert.match(USAGE, new RegExp(command.replace(" ", "\\s+")));
});
