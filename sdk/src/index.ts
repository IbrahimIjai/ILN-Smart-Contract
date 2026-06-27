/**
 * @iln/sdk — Invoice Liquidity Network TypeScript SDK
 *
 * Public surface area re-exported from this entry point.
 */

export { fundInvoice, computeEffectiveYieldBps } from "./methods/fundInvoice.js";
export { getReputation } from "./methods/reputation.js";
export { getContractStats } from "./methods/stats.js";
export {
  getAllowance,
  buildApproveTransaction,
  isAllowanceSufficient,
} from "./utils/allowance.js";
export {
  validateGAddress,
  validateContractId,
  validateAmount,
  validateDiscountRate,
  validateDueDate,
} from "./utils/validate.js";
export { KeypairSigner } from "./signers/KeypairSigner.js";
export { FreighterSigner, ILNError, ILNErrorCode } from "./signers/FreighterSigner.js";
export { subscribe, parseContractEvent, matchesFilter } from "./events/subscribe.js";
export { ILNClient, iln } from "./client.js";
export type { ISigner } from "./signers/ISigner.js";
export type { ILNClientConfig } from "./client.js";
export type { ReputationProfile } from "./methods/reputation.js";
export type { ContractStats } from "./methods/stats.js";
export type {
  FundOptions,
  FundResult,
  InvoiceView,
  AllowanceParams,
  AllowanceResult,
} from "./types.js";
export type {
  ILNEvent,
  ILNEventType,
  EventFilter,
  Unsubscribe,
} from "./events/types.js";

export { getInvoice, listInvoicesBySubmitter, listInvoicesByLP } from "./methods/queries.js";
export { submitInvoice } from "./methods/submitInvoice.js";
export { transferLPPosition } from "./methods/transferLPPosition.js";
export { cancelInvoice } from "./methods/cancelInvoice.js";
export { markPaid } from "./methods/markPaid.js";
export {
  createProposal,
  castVote,
  executeProposal,
  getProposal,
  listProposals,
} from "./methods/governance.js";
export {
  ProposalAction,
  ProposalStatus,
} from "./types/governance.js";
export type {
  Proposal,
  ProposalFilter,
  CreateProposalResult,
} from "./types/governance.js";
export { ILNError } from "./errors.js";
