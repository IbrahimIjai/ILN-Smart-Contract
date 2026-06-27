import { cancelInvoice } from '../src/methods/cancelInvoice.js';
import { ILNError } from '../src/errors.js';
import { Account, SorobanRpc } from '@stellar/stellar-sdk';
import * as queries from '../src/methods/queries.js';

jest.mock('../src/methods/queries.js');

describe('cancelInvoice', () => {
  const mockServer = { simulateTransaction: jest.fn() } as unknown as SorobanRpc.Server;
  const mockAccount = new Account('GAFREELANCER', '1');
  const mockSign = jest.fn((tx) => tx);

  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('throws if invoice is not in Pending state', async () => {
    // @ts-ignore
    queries.getInvoice.mockResolvedValue({ status: 'Funded', freelancer: 'GAFREELANCER' });
    await expect(cancelInvoice(mockServer, 'C123', 1n, mockAccount, mockSign, 'pass'))
      .rejects.toThrow(ILNError.InvoiceNotCancellable);
  });

  it('throws if caller is not the invoice submitter', async () => {
    // @ts-ignore
    queries.getInvoice.mockResolvedValue({ status: 'Pending', freelancer: 'GADIFFERENT' });
    await expect(cancelInvoice(mockServer, 'C123', 1n, mockAccount, mockSign, 'pass'))
      .rejects.toThrow(ILNError.Unauthorized);
  });
});
