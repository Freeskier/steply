import { json } from '@sveltejs/kit';
import { sendPreviewSessionKey } from '$lib/server/steply-preview';

export const POST = async ({ request }) => {
  try {
    const body = await request.json();
    const sessionId = typeof body?.sessionId === 'string' ? body.sessionId : '';
    const keyEvent = body?.keyEvent;
    const renderRequest = body?.request;

    if (!sessionId) {
      return json({ ok: false, error: 'Missing session id' }, { status: 400 });
    }
    if (!keyEvent || typeof keyEvent !== 'object') {
      return json({ ok: false, error: 'Missing key event' }, { status: 400 });
    }
    if (!renderRequest || typeof renderRequest !== 'object') {
      return json({ ok: false, error: 'Missing render request' }, { status: 400 });
    }

    const rendered = sendPreviewSessionKey(sessionId, keyEvent, renderRequest);
    return json({ ok: true, rendered });
  } catch (error) {
    return json(
      { ok: false, error: error instanceof Error ? error.message : String(error) },
      { status: 400 }
    );
  }
};
