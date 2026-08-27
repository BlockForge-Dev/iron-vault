# Token-2022 security policy

## V1 decision

IronVault accepts exactly two token programs:

- legacy SPL Token;
- Token-2022 with an extension-free mint.

All token-bearing accounts use Anchor's `Mint`, `TokenAccount`, and
`TokenInterface` interface types. Every operation separately proves that its
mint, source, custody, and destination accounts are owned by the selected or
immutably stored token program. The program continues to use checked transfers
and verifies exact pre/post balances.

For Token-2022, IronVault parses the mint with `StateWithExtensions<Mint>` and
requires `get_extension_types()` to be empty. The policy is an allowlist, not a
blocklist: an extension unknown to this release is rejected automatically.

## Explicitly rejected semantics

The empty-list rule rejects, among all other extensions:

- **Permanent Delegate:** another authority could transfer or burn tokens from
  custody without the IronVault PDA signing.
- **Transfer Hook:** execution depends on another program and caller-supplied
  accounts outside the reviewed fixed CPI surface.
- **Transfer Fee:** the recipient may receive less than the requested amount,
  violating exact-balance invariants and policy accounting.
- **Non-transferable:** custody and eventual release/refund semantics are
  incompatible with a token that cannot transfer.

Account extensions are not separately allowlisted because v1 admits no mint
extension that requires them. Custody accounts are created by the selected token
program with the canonical IronVault PDA authority.

## Verification evidence and boundaries

LiteSVM tests construct serialized Token-2022 mints using the official TLV state
API. They prove successful escrow funding for an extension-free Token-2022 mint,
continued legacy SPL support, and rejection/rollback for Permanent Delegate,
Transfer Hook, Transfer Fee, and Non-transferable mints.

This policy does not audit either SPL Token implementation, promise support for
every Token-2022 mint, or make a mint authority trustworthy. Adding any
extension requires a specification change, adversarial integration fixtures,
review of all additional CPI accounts, and revalidation of exact-transfer and
custody invariants.
