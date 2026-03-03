import { json } from '@sveltejs/kit';
import { disposePreviewSession } from '$lib/server/steply-preview';

export const POST = async ({ request }) => {
  const body = await request.json().catch(() => ({}));
  const sessionId = typeof body?.sessionId === 'string' ? body.sessionId : '';
  if (!sessionId) {
    return json({ ok: false, error: 'Missing session id' }, { status: 400 });
  }
  return json({ ok: true, disposed: disposePreviewSession(sessionId) });
};
