// The frontend's single SpiritStream client instance.
//
// Every component and store imports `api` from here — they never reach into
// `@spiritstream/api-client` directly for resource calls. When the rewrite
// ships a Veilid transport, this is the only file that needs to
// flip from `transport: 'http'` to `transport: 'veilid'`.

import { makeApiClient } from '@spiritstream/api-client';

export const api = makeApiClient({ transport: 'http' });
