export type InvoiceStatus =
  | 'Pending'
  | 'Funded'
  | 'PartiallyFunded'
  | 'Paid'
  | 'Defaulted'
  | 'Appealed'
  | 'Disputed'
  | 'Expired'
  | 'Cancelled';

export interface Invoice {
  id: bigint;
  freelancer: string;
  payer: string;
  token: string;
  amount: bigint;
  dueDate: number;
  discountRate: number;
  status: InvoiceStatus;
  funder?: string;
  fundedAt?: number;
  amountFunded: bigint;
  amountPaid: bigint;
  referralCode?: string;
  submitterReputation: number;
  effectiveYieldBps: number;
}
