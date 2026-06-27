import { getInvoice, listInvoicesBySubmitter, listInvoicesByLP } from '../src/methods/queries.js';
import { ILNError } from '../src/errors.js';
import { Account, SorobanRpc } from '@stellar/stellar-sdk';

describe('queries', () => {
  const mockServer = { simulateTransaction: jest.fn() } as unknown as SorobanRpc.Server;
  const mockAccount = new Account('G123', '1');

  it('getInvoice throws InvoiceNotFound', async () => {
    // @ts-ignore
    mockServer.simulateTransaction.mockResolvedValue({ error: 'NotFound' });
    await expect(getInvoice(mockServer, 'C123', 1n, mockAccount, 'pass')).rejects.toThrow(ILNError.InvoiceNotFound);
  });
});
