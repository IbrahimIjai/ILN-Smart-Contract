# Contract Error Codes

This reference documents the current `ContractError` enum used by `invoice_liquidity`.
Source of truth: [`contracts/invoice_liquidity/src/errors.rs`](../contracts/invoice_liquidity/src/errors.rs)

If you change the enum, update this page at the same time so integrators can keep their error handling in sync.

> Note: the current source assigns code `33` to two variants: `FeeOnTransferToken` and `PayerUnverified`.
> This document mirrors the enum exactly so callers can map the runtime value back to the right failure mode.

| Code | Variant | Description | Common cause | Recommended remediation |
|------|---------|-------------|--------------|--------------------------|
| 1 | `InvoiceNotFound` | The requested invoice ID does not exist in contract storage. | Caller supplied an invalid or deleted invoice ID. | Check the invoice ID before calling, or load it from a prior successful `submit_invoice` response. |
| 2 | `AlreadyFunded` | The invoice is already funded and cannot be funded again. | A second LP attempted to fund an invoice that already reached the funded state. | Read the invoice status first and stop funding once the invoice is funded. |
| 3 | `AlreadyPaid` | The invoice has already been paid. | A payer or LP retried a payment flow after settlement completed. | Treat the invoice as terminal and skip any additional payment, funding, or collection attempts. |
| 4 | `NotFunded` | The invoice has not been funded yet. | A caller tried to settle, claim, or resolve a flow that requires an active funded invoice. | Fund the invoice first, or wait until the correct state transition has occurred. |
| 5 | `Unauthorized` | The caller does not have the required role or authorization for the action. | Wrong account signed the transaction, or the contract has not been configured with the expected admin/role mapping. | Verify the signing account and required role, then retry with the correct address or permissions. |
| 6 | `InvalidAmount` | The provided amount is not acceptable to the contract. | Zero, negative, or otherwise malformed payment/funding amount. | Send a positive amount that matches the invoice rules and token decimals. |
| 7 | `InvalidDiscountRate` | The discount rate is outside the allowed range. | Admin or caller supplied a rate above the contract maximum or in the wrong units. | Use the documented basis-point format and keep the value within the configured bounds. |
| 8 | `InvalidDueDate` | The due date is not valid for invoice creation or update. | Due date is in the past, malformed, or violates contract invariants. | Provide a future due date that satisfies the contract’s validation rules. |
| 9 | `InvoiceDefaulted` | The invoice has already defaulted. | Caller tried to fund, pay, cancel, or otherwise act on an invoice that is already in default. | Use the default/appeal flows instead of settlement or funding flows. |
| 10 | `NothingToClaim` | There is no yield or claimable amount available. | LP tried to claim before yield accrued or before funds became claimable. | Wait until the invoice has generated claimable yield, then retry the claim. |
| 11 | `NotYetDefaulted` | The invoice has not reached the default threshold yet. | A default-claim or default-handling function was called too early. | Wait until the invoice is actually defaulted before using the default recovery flow. |
| 12 | `OverfundingRejected` | The funding attempt would exceed the invoice’s remaining amount. | LP sent more than the unpaid principal or attempted to top up beyond the cap. | Fund only the remaining unpaid amount, or read the remaining balance first. |
| 13 | `InvoiceExpired` | The invoice has expired and cannot proceed through normal settlement. | Caller tried to fund or pay after the invoice passed its allowed lifecycle window. | Create a fresh invoice or use the appropriate default/closure flow if supported. |
| 14 | `BatchTooLarge` | The submitted batch exceeds the contract’s maximum batch size. | Bulk action included too many invoices in one call. | Split the request into smaller batches and retry. |
| 15 | `AlreadyCancelled` | The invoice was already cancelled. | A caller retried a cancel flow or attempted another action after cancellation. | Treat the invoice as terminal and stop sending state-changing actions for it. |
| 16 | `AlreadyInitialized` | The contract was initialized more than once. | A deployment or setup script ran initialization again after state already existed. | Run initialization only once per deployment and guard scripts against duplicate setup. |
| 17 | `AlreadyAppealed` | An appeal already exists for this invoice. | The payer submitted a second appeal for the same defaulted invoice. | Check whether an appeal is already open before creating another one. |
| 18 | `AppealWindowClosed` | The appeal deadline has passed. | The appeal was submitted after the configured appeal window elapsed. | Submit the appeal before the deadline, or update the contract configuration if the window needs to change. |
| 19 | `NotDefaulted` | The invoice is not currently in the defaulted state required by this action. | A caller attempted to appeal or resolve a default-specific flow before default existed. | Wait until the invoice is defaulted, then retry the default-specific action. |
| 20 | `AlreadyInQueue` | The LP has already joined the funding queue for this invoice. | Duplicate queue enrollment request from the same LP. | Skip re-joining if the LP is already queued, or remove the existing queue entry first. |
| 21 | `NotApprovedFunder` | The LP is not the funder approved by the priority queue. | A different LP attempted to fund before queue resolution selected them. | Wait for queue resolution and fund only when the contract assigns that LP as the approved funder. |
| 22 | `InvoiceAppealed` | The invoice is currently in the appealed state. | Another action was attempted while appeal review is still in progress. | Wait for the appeal to resolve before retrying settlement or closure flows. |
| 23 | `AlreadyDisputed` | The invoice is already disputed. | A caller attempted to open a second dispute on the same invoice. | Check dispute status before filing and avoid re-opening an active dispute. |
| 24 | `NotDisputed` | The invoice is not in a disputed state. | A dispute-resolution function was called before a dispute existed. | Open a dispute first, or call the correct function for the current invoice state. |
| 25 | `InvoiceDisputed` | The invoice is under dispute and cannot proceed through normal settlement. | A user attempted to fund, pay, or finalize an invoice while a dispute is active. | Resolve or dismiss the dispute before retrying normal invoice actions. |
| 26 | `ContractPaused` | The contract is currently paused. | An admin paused the protocol for maintenance, incident response, or governance action. | Wait until the contract is unpaused, or ask the admin/governance process to resume it. |
| 27 | `DueDateTooSoon` | The due date is earlier than the minimum allowed horizon. | Invoice due date was set too close to the current ledger time. | Choose a later due date that satisfies the contract’s minimum lead time. |
| 28 | `DueDateTooFar` | The due date is later than the maximum allowed horizon. | Invoice due date was set too far in the future. | Reduce the due date to fall within the contract’s configured maximum range. |
| 29 | `SelfInvoice` | The payer and invoice creator are the same address. | A caller attempted to create an invoice against themselves. | Use distinct payer and submitter addresses, or fix the invoice data before resubmitting. |
| 30 | `OverpaymentRejected` | The payment amount exceeds the remaining amount due. | Payer attempted to pay more than the invoice balance. | Pay exactly the remaining amount or query the outstanding balance first. |
| 31 | `PayerReputationTooLow` | The payer’s reputation is below the configured minimum threshold. | Reputation gate is enabled and the payer score does not meet the contract requirement. | Improve the payer’s reputation score, or adjust the minimum threshold through the approved governance/admin path. |
| 32 | `ArithmeticOverflow` | A checked arithmetic operation overflowed. | Large amounts, counters, or computed values exceeded `u64`/`i128` limits during processing. | Re-check inputs for unreasonable values and investigate the caller data or contract math path. |
| 33 | `FeeOnTransferToken` | The token charges a transfer fee, so the received amount differs from the amount sent. | An unsupported fee-on-transfer asset was added or used for settlement. | Use a standard token that transfers the full amount, or remove the fee-on-transfer asset from configuration. |
| 33 | `PayerUnverified` | The oracle did not verify the payer when verification was required. | Oracle verification is enabled, but the payer is not present or not verified in the oracle response. | Use a verified payer account, or disable payer verification if that policy is not required. |
| 34 | `OracleDataStale` | The oracle response is older than the configured freshness window. | The payer-verification oracle data has exceeded `max_oracle_age_ledgers`. | Refresh oracle data and retry, or increase the freshness window only if that tradeoff is acceptable. |

## Keeping This Doc Current

When you add, remove, or renumber variants in [`contracts/invoice_liquidity/src/errors.rs`](../contracts/invoice_liquidity/src/errors.rs):

1. Update this table.
2. Update any client-side error mapping in SDKs or examples.
3. Keep the README link below pointing here so the reference remains easy to find.
