use anchor_lang::prelude::*;

/// Errors shared by future IronVault instructions.
#[error_code]
pub enum IronVaultError {
    /// The supplied protocol version is unsupported.
    #[msg("Unsupported protocol version")]
    UnsupportedVersion,
    /// Protocol admin and guardian identities must be distinct and non-default.
    #[msg("Protocol authority configuration is invalid")]
    InvalidProtocolAuthority,
    /// Only the stored protocol admin or guardian may manage emergency flags.
    #[msg("Caller cannot manage protocol pause flags")]
    UnauthorizedProtocolPause,
    /// Guardians may add emergency flags but cannot remove any active flag.
    #[msg("Protocol guardian cannot clear pause flags")]
    GuardianCannotUnpause,
    /// Pause masks must contain only explicitly supported scopes.
    #[msg("Protocol pause mask contains unknown flags")]
    InvalidPauseFlags,
    /// The requested protocol operation is currently paused.
    #[msg("Protocol operation is paused")]
    ProtocolPaused,
    /// Escrow deposits must contain at least one base unit.
    #[msg("Escrow amount must be greater than zero")]
    InvalidAmount,
    /// The recipient must be non-default and different from the maker.
    #[msg("Escrow recipient is invalid")]
    InvalidRecipient,
    /// The escrow expiry must be strictly later than its creation time.
    #[msg("Escrow expiry must be in the future")]
    InvalidExpiry,
    /// The maker does not own the supplied source token account.
    #[msg("Maker does not own the source token account")]
    InvalidSourceOwner,
    /// The supplied source token account is for a different mint.
    #[msg("Source token account mint does not match")]
    InvalidSourceMint,
    /// The source token account cannot fund the requested amount.
    #[msg("Source token account has insufficient funds")]
    InsufficientFunds,
    /// The custody token account did not start empty or changed unexpectedly.
    #[msg("Escrow custody balance is invalid")]
    InvalidCustodyBalance,
    /// The source balance did not change by the exact requested amount.
    #[msg("Source token balance changed unexpectedly")]
    InvalidSourceBalance,
    /// Only a funded escrow can transition to a terminal state.
    #[msg("Escrow is not funded")]
    EscrowNotFunded,
    /// A maker release must occur strictly before expiry.
    #[msg("Escrow has expired")]
    EscrowExpired,
    /// Refunds become available at, but not before, the escrow expiry.
    #[msg("Escrow has not expired")]
    EscrowNotExpired,
    /// The release token account must be owned by the immutable recipient.
    #[msg("Release destination is not owned by the escrow recipient")]
    InvalidRecipientOwner,
    /// The release token account must use the immutable escrow mint.
    #[msg("Release destination mint does not match")]
    InvalidRecipientMint,
    /// The destination balance did not increase by the exact escrow amount.
    #[msg("Recipient token balance changed unexpectedly")]
    InvalidRecipientBalance,
    /// A refund destination must be controlled by the immutable maker.
    #[msg("Refund destination is not owned by the escrow maker")]
    InvalidMakerDestinationOwner,
    /// A refund destination must use the immutable escrow mint.
    #[msg("Refund destination mint does not match")]
    InvalidMakerDestinationMint,
    /// The maker destination balance did not increase by the exact escrow amount.
    #[msg("Maker destination balance changed unexpectedly")]
    InvalidMakerDestinationBalance,
    /// The vault guardian must be non-default and distinct from its authority.
    #[msg("Vault guardian is invalid")]
    InvalidVaultGuardian,
    /// Only the vault's stored authority may perform this core operation.
    #[msg("Caller is not the vault authority")]
    InvalidVaultAuthority,
    /// A replacement authority must be non-default and distinct from current intrinsic actors.
    #[msg("New vault authority is invalid")]
    InvalidNewVaultAuthority,
    /// The supplied asset is not enabled for vault operations.
    #[msg("Vault asset is disabled")]
    VaultAssetDisabled,
    /// Vault transfers must contain at least one base unit.
    #[msg("Vault transfer amount must be greater than zero")]
    InvalidVaultAmount,
    /// The depositor does not own the supplied source token account.
    #[msg("Depositor does not own the source token account")]
    InvalidDepositSourceOwner,
    /// A deposit source must use the registered asset mint.
    #[msg("Deposit source mint does not match the vault asset")]
    InvalidDepositSourceMint,
    /// A withdrawal destination must use the registered asset mint.
    #[msg("Withdrawal destination mint does not match the vault asset")]
    InvalidWithdrawalDestinationMint,
    /// Vault custody cannot fund the requested withdrawal.
    #[msg("Vault custody has insufficient funds")]
    InsufficientVaultFunds,
    /// The vault custody balance changed by an unexpected amount.
    #[msg("Vault custody balance changed unexpectedly")]
    InvalidVaultCustodyBalance,
    /// The inbound source balance changed by an unexpected amount.
    #[msg("Deposit source balance changed unexpectedly")]
    InvalidDepositSourceBalance,
    /// The withdrawal destination balance changed by an unexpected amount.
    #[msg("Withdrawal destination balance changed unexpectedly")]
    InvalidWithdrawalDestinationBalance,
    /// Paused vaults reject outflows.
    #[msg("Vault is paused")]
    VaultPaused,
    /// Only the stored authority or guardian may pause a vault.
    #[msg("Caller cannot pause this vault")]
    UnauthorizedVaultPause,
    /// Only the stored authority may unpause a vault.
    #[msg("Only the vault authority can unpause")]
    UnauthorizedVaultUnpause,
    /// Pause and unpause must change the current local state.
    #[msg("Vault pause state is unchanged")]
    VaultPauseUnchanged,
    /// Role principals must be non-default and distinct from intrinsic vault actors.
    #[msg("Role principal is invalid")]
    InvalidRolePrincipal,
    /// A role must contain only known bits and grant at least one capability.
    #[msg("Role permission mask is invalid")]
    InvalidPermissionMask,
    /// The supplied role is inactive or does not grant the required capability.
    #[msg("Caller lacks the required vault permission")]
    MissingVaultPermission,
    /// Only active roles can be revoked.
    #[msg("Role is not active")]
    RoleNotActive,
    /// Withdrawal accepts no extra accounts for authorities and exactly one role for operators.
    #[msg("Unexpected withdrawal accounts")]
    UnexpectedWithdrawalAccounts,
    /// Withdrawal limits must be positive and internally consistent.
    #[msg("Withdrawal policy is invalid")]
    InvalidWithdrawalPolicy,
    /// The requested amount exceeds the configured per-transaction maximum.
    #[msg("Per-transaction withdrawal limit exceeded")]
    PerTransactionLimitExceeded,
    /// The requested amount would exceed the configured rolling-window maximum.
    #[msg("Rolling-window withdrawal limit exceeded")]
    WindowLimitExceeded,
    /// Checked policy arithmetic failed.
    #[msg("Withdrawal policy arithmetic overflow")]
    WithdrawalPolicyOverflow,
    /// A live accounting window retains its original duration.
    #[msg("Cannot change duration while the withdrawal window is live")]
    LiveWindowDurationChange,
    /// Limit management accepts no extra accounts for authorities and one role for operators.
    #[msg("Unexpected limit-management accounts")]
    UnexpectedLimitAccounts,
    #[msg("Withdrawal amount requires a timelocked request")]
    TimelockRequired,
    #[msg("Withdrawal amount does not require a timelocked request")]
    TimelockNotRequired,
    #[msg("Withdrawal request timing overflow")]
    WithdrawalTimingOverflow,
    #[msg("Withdrawal request is not pending")]
    WithdrawalNotPending,
    #[msg("Withdrawal timelock has not elapsed")]
    WithdrawalTimelockActive,
    #[msg("Withdrawal request execution window has expired")]
    WithdrawalRequestExpired,
    #[msg("Withdrawal recipient account does not match the immutable request")]
    InvalidWithdrawalRecipient,
    #[msg("Withdrawal request mint does not match")]
    InvalidWithdrawalMint,
    #[msg("Caller cannot cancel this withdrawal request")]
    UnauthorizedWithdrawalCancellation,
    #[msg("Unexpected withdrawal-request authorization accounts")]
    UnexpectedRequestAccounts,
    #[msg("Unexpected withdrawal-cancellation authorization accounts")]
    UnexpectedCancellationAccounts,
    /// Mint and token accounts must be owned by the selected token program.
    #[msg("Token account or mint is not owned by the selected token program")]
    InvalidTokenProgram,
    /// V1 supports only extension-free Token-2022 mints.
    #[msg("Token-2022 mint extensions are not supported")]
    UnsupportedTokenExtension,
}
