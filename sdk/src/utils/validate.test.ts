import {
  validateGAddress,
  validateContractId,
  validateAmount,
  validateDiscountRate,
  validateDueDate,
} from "./validate.js";
import { ILNError } from "../errors.js";

const G = "G".padEnd(56, "A");
const C = "C".padEnd(56, "A");

describe("validateGAddress", () => {
  it("accepts a well-formed G address", () => {
    expect(() => validateGAddress(G)).not.toThrow();
  });

  it("rejects an empty string", () => {
    expect(() => validateGAddress("")).toThrow(ILNError.InvalidAddress);
  });

  it("rejects an address not starting with G", () => {
    expect(() => validateGAddress(C)).toThrow(ILNError.InvalidAddress);
  });

  it("rejects an address of the wrong length", () => {
    expect(() => validateGAddress("GABC")).toThrow(ILNError.InvalidAddress);
  });
});

describe("validateContractId", () => {
  it("accepts a well-formed C address", () => {
    expect(() => validateContractId(C)).not.toThrow();
  });

  it("rejects a non-C address", () => {
    expect(() => validateContractId(G)).toThrow(ILNError.InvalidAddress);
  });

  it("rejects an empty string", () => {
    expect(() => validateContractId("")).toThrow(ILNError.InvalidAddress);
  });
});

describe("validateAmount", () => {
  it("accepts an amount at the minimum boundary", () => {
    expect(() => validateAmount(100n, 100n, "USDC")).not.toThrow();
  });

  it("rejects a zero or negative amount", () => {
    expect(() => validateAmount(0n, 1n, "USDC")).toThrow(ILNError.InvalidAmount);
    expect(() => validateAmount(-5n, 1n, "USDC")).toThrow(ILNError.InvalidAmount);
  });

  it("rejects an amount below the minimum", () => {
    expect(() => validateAmount(99n, 100n, "USDC")).toThrow(ILNError.InvalidAmount);
  });

  it("rejects an unknown non-contract token", () => {
    expect(() => validateAmount(100n, 1n, "DOGE")).toThrow(ILNError.InvalidAmount);
  });

  it("accepts a custom contract-address token", () => {
    expect(() => validateAmount(100n, 1n, C)).not.toThrow();
  });
});

describe("validateDiscountRate", () => {
  it("accepts boundary values 1 and 5000", () => {
    expect(() => validateDiscountRate(1)).not.toThrow();
    expect(() => validateDiscountRate(5000)).not.toThrow();
  });

  it("rejects values below 1 or above 5000", () => {
    expect(() => validateDiscountRate(0)).toThrow(ILNError.InvalidDiscountRate);
    expect(() => validateDiscountRate(5001)).toThrow(ILNError.InvalidDiscountRate);
  });

  it("rejects non-integer rates", () => {
    expect(() => validateDiscountRate(3.5)).toThrow(ILNError.InvalidDiscountRate);
  });
});

describe("validateDueDate", () => {
  it("accepts a date ~30 days out", () => {
    const d = new Date(Date.now() + 30 * 24 * 60 * 60 * 1000);
    expect(() => validateDueDate(d)).not.toThrow();
  });

  it("rejects an invalid Date", () => {
    expect(() => validateDueDate(new Date("not-a-date"))).toThrow(ILNError.InvalidDueDate);
  });

  it("rejects a date less than 24h away", () => {
    const d = new Date(Date.now() + 60 * 1000);
    expect(() => validateDueDate(d)).toThrow(ILNError.DueDateTooSoon);
  });

  it("rejects a date more than 365 days away", () => {
    const d = new Date(Date.now() + 400 * 24 * 60 * 60 * 1000);
    expect(() => validateDueDate(d)).toThrow(ILNError.DueDateTooFar);
  });
});
