import { json } from '@sveltejs/kit';
import { createPreviewSession, renderPreviewSession } from '$lib/server/steply-preview';

export const POST = async ({ request }) => {
  try {
    const body = await request.json();
    const yaml = typeof body?.yaml === 'string' ? body.yaml : '';
    const renderRequest = body?.request;

    if (!yaml.trim()) {
      return json({ ok: false, error: 'YAML is empty' }, { status: 400 });
    }
    if (!renderRequest || typeof renderRequest !== 'object') {
      return json({ ok: false, error: 'Missing render request' }, { status: 400 });
    }

    const sessionId = createPreviewSession(yaml);
    const rendered = renderPreviewSession(sessionId, renderRequest);
    return json({ ok: true, sessionId, rendered });
  } catch (error) {
    return json(
      { ok: false, error: error instanceof Error ? error.message : String(error) },
      { status: 400 }
    );
  }
};
