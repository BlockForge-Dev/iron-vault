/** Canonical program address shared with `declare_id!` and Anchor.toml. */
export const IRON_VAULT_PROGRAM_ID =
  "2UWmTuefm4gqbfuZP36NSJMMSKLM4Rbop25jf1uBZAu1" as const;

/** Stable ASCII PDA namespaces from the Milestone 0 account model. */
export const PDA_SEEDS = Object.freeze({
  protocol: "protocol",
  escrow: "escrow",
  escrowToken: "escrow_token",
  vault: "vault",
  vaultAsset: "vault_asset",
  vaultToken: "vault_token",
  role: "role",
  withdrawal: "withdrawal",
} as const);

export type PdaSeedName = keyof typeof PDA_SEEDS;
