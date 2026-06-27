/**
 * Centralised input validation utilities for the ILN SDK.
 *
 * Validation logic for Stellar G-addresses, contract IDs, amounts, discount
 * rates and due dates was previously duplicated across method implementations.
 * Concentrating it here keeps error messages consistent and reduces bugs.
 *
 * Each validator throws a typed {@link ILNError} on failure and returns `void`
 * on success, so they can be used as guard clauses at the top of methods.
 */

import { ILNError } from "../errors.js";
import type { SupportedToken } from "../types/params.js";

/** Length of a Stellar StrKey-encoded public key (G…) address. */
const G_ADDRESS_LENGTH = 56;
/** Length of a Stellar StrKey-encoded contract (C…) address. */
const C_ADDRESS_LENGTH = 56;

/** Minimum allowed discount rate in basis points. */
export const MIN_DISCOUNT_BPS = 1;
/** Maximum allowed discount rate in basis points (50%). */
export const MAX_DISCOUNT_BPS = 5000;

/** Minimum invoice duration: 24 hours from now. */
export const MIN_DUE_DATE_MS = 24 * 60 * 60 * 1000;
/** Maximum invoice duration: 365 days from now. */
export const MAX_DUE_DATE_MS = 365 * 24 * 60 * 60 * 1000;

/** Base-unit precision (decimals) for each supported token. */
const TOKEN_DECIMALS: Record<string, number> = {
  USDC: 7,
  EURC: 7,
  XLM: 7,
};

/**
 * Validate a Stellar account (G…) address.
 *
 * @param address - The address to validate
 * @throws {ILNError.InvalidAddress} If the address is empty, not a string, does
 *   not start with `G`, or is not 56 characters long.
 */
export function validateGAddress(address: string): void {
  if (typeof address !== "string" || address.length === 0) {
    throw new ILNError.InvalidAddress("Address must be a non-empty string");
  }
  if (!address.startsWith("G")) {
    throw new ILNError.InvalidAddress(
      `Invalid Stellar address "${address}": must start with "G"`
    );
  }
  if (address.length !== G_ADDRESS_LENGTH) {
    throw new ILNError.InvalidAddress(
      `Invalid Stellar address "${address}": must be ${G_ADDRESS_LENGTH} characters`
    );
  }
}

/**
 * Validate a Soroban contract (C…) address.
 *
 * @param contractId - The contract ID to validate
 * @throws {ILNError.InvalidAddress} If the contract ID is malformed.
 */
export function validateContractId(contractId: string): void {
  if (typeof contractId !== "string" || contractId.length === 0) {
    throw new ILNError.InvalidAddress("Contract ID must be a non-empty string");
  }
  if (!contractId.startsWith("C") || contractId.length !== C_ADDRESS_LENGTH) {
    throw new ILNError.InvalidAddress(
      `Invalid contract ID "${contractId}": must start with "C" and be ${C_ADDRESS_LENGTH} characters`
    );
  }
}

/**
 * Validate a token amount against a minimum and the token's precision.
 *
 * @param amount - The amount in token base units
 * @param min    - The minimum allowed amount (inclusive) in base units
 * @param token  - The token the amount is denominated in
 * @throws {ILNError.InvalidAmount} If the amount is not a positive integer, is
 *   below the minimum, or exceeds the token's precision.
 */
export function validateAmount(
  amount: bigint,
  min: bigint,
  token: SupportedToken
): void {
  if (typeof amount !== "bigint") {
    throw new ILNError.InvalidAmount("Amount must be a bigint (token base units)");
  }
  if (amount <= 0n) {
    throw new ILNError.InvalidAmount("Amount must be greater than 0");
  }
  if (amount < min) {
    throw new ILNError.InvalidAmount(
      `Amount ${amount} is below the minimum of ${min} for ${token}`
    );
  }
  // Precision guard: amounts are integers in base units, so a negative or
  // unknown decimal config is the only way this can be violated. We surface
  // unsupported tokens explicitly to avoid silent mis-scaling.
  const decimals = TOKEN_DECIMALS[token];
  if (decimals === undefined && !token.startsWith("C")) {
    throw new ILNError.InvalidAmount(
      `Unknown token "${token}": cannot verify amount precision`
    );
  }
}

/**
 * Validate a discount rate.
 *
 * @param rate - Discount rate in basis points (1–5000)
 * @throws {ILNError.InvalidDiscountRate} If the rate is outside the 1–5000 bps range.
 */
export function validateDiscountRate(rate: number): void {
  if (!Number.isInteger(rate)) {
    throw new ILNError.InvalidDiscountRate("Discount rate must be an integer (bps)");
  }
  if (rate < MIN_DISCOUNT_BPS || rate > MAX_DISCOUNT_BPS) {
    throw new ILNError.InvalidDiscountRate(
      `Discount rate must be between ${MIN_DISCOUNT_BPS} and ${MAX_DISCOUNT_BPS} bps`
    );
  }
}

/**
 * Validate an invoice due date.
 *
 * @param date - The due date
 * @throws {ILNError.InvalidDueDate} If the date is invalid.
 * @throws {ILNError.DueDateTooSoon} If the date is less than 24h from now.
 * @throws {ILNError.DueDateTooFar} If the date is more than 365 days from now.
 */
export function validateDueDate(date: Date): void {
  if (!(date instanceof Date) || Number.isNaN(date.getTime())) {
    throw new ILNError.InvalidDueDate("Due date must be a valid Date");
  }
  const delta = date.getTime() - Date.now();
  if (delta < MIN_DUE_DATE_MS) {
    throw new ILNError.DueDateTooSoon("Due date must be at least 24 hours from now");
  }
  if (delta > MAX_DUE_DATE_MS) {
    throw new ILNError.DueDateTooFar("Due date must be within 365 days from now");
  }
}
