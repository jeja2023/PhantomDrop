import PostalMime from 'postal-mime';

export interface Env {
  PHANTOM_HUB_URL: string;
  HUB_SECRET: string;
}

const DEFAULT_HUB_SECRET = 'local_dev_secret';

interface MailMessage {
  from: string;
  to: string;
  raw: ReadableStream;
}

interface ParsedAttachment {
  filename?: string | null;
  mimeType?: string | null;
}

interface ParsedMail {
  subject?: string | null;
  date?: string | null;
  text?: string | null;
  html?: string | null;
  attachments: ParsedAttachment[];
}

interface HubPayload {
  meta: {
    from: string;
    to: string;
    subject?: string | null;
    date?: string | null;
  };
  content: {
    text?: string | null;
    html?: string | null;
  };
  attachments?: Array<{
    filename?: string | null;
    mime_type?: string | null;
  }>;
}

function normalizedHubUrl(env: Env): string {
  const hubUrl = env.PHANTOM_HUB_URL?.trim();
  if (!hubUrl) {
    throw new Error('PHANTOM_HUB_URL is not configured');
  }

  return hubUrl.replace(/\/+$/, '');
}

function hubSecret(env: Env): string {
  const secret = env.HUB_SECRET?.trim();
  if (!secret) {
    throw new Error('HUB_SECRET is not configured. Run: npx wrangler secret put HUB_SECRET');
  }
  if (secret === DEFAULT_HUB_SECRET) {
    throw new Error('HUB_SECRET is still using the development default');
  }

  return secret;
}

function json(data: unknown, init?: ResponseInit): Response {
  return new Response(JSON.stringify(data, null, 2), {
    headers: {
      'content-type': 'application/json; charset=utf-8',
    },
    ...init,
  });
}

async function relayToHub(payload: HubPayload, env: Env): Promise<Response> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 10_000);
  const hubUrl = normalizedHubUrl(env);

  return fetch(`${hubUrl}/ingest`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-Hub-Secret': hubSecret(env),
    },
    body: JSON.stringify(payload),
    signal: controller.signal,
  }).finally(() => clearTimeout(timeout));
}

function buildRelayTestPayload(overridePayload?: Partial<HubPayload>): HubPayload {
  const now = new Date().toISOString();
  const subjectSuffix = now.replace(/[:.]/g, '-');

  return {
    meta: {
      from: overridePayload?.meta?.from ?? 'worker-probe@phantomdrop.local',
      to: overridePayload?.meta?.to ?? 'probe@phantomdrop.local',
      subject: overridePayload?.meta?.subject ?? `PhantomDrop Worker Probe ${subjectSuffix}`,
      date: overridePayload?.meta?.date ?? now,
    },
    content: {
      text: overridePayload?.content?.text ?? `Worker relay probe at ${now}. Verification code 246810.`,
      html:
        overridePayload?.content?.html ??
        `<html><body><p>Worker relay probe at <strong>${now}</strong>.</p><p>Verification code <strong>246810</strong>.</p></body></html>`,
    },
    attachments: overridePayload?.attachments ?? [],
  };
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    if (request.method === 'GET' && url.pathname === '/health') {
      try {
        return json({
          status: 'ok',
          worker: 'phantom-drop-edge',
          hub_url: normalizedHubUrl(env),
          secret_configured: Boolean(hubSecret(env)),
        });
      } catch (error) {
        return json(
          {
            status: 'error',
            worker: 'phantom-drop-edge',
            message: error instanceof Error ? error.message : String(error),
          },
          { status: 500 },
        );
      }
    }

    if (request.method === 'POST' && url.pathname === '/relay-test') {
      try {
        const body = (await request.json().catch(() => ({}))) as Partial<HubPayload>;
        const payload = buildRelayTestPayload(body);
        const response = await relayToHub(payload, env);
        const responseText = await response.text();

        return json(
          {
            status: response.ok ? 'success' : 'error',
            hub_status: response.status,
            hub_response: responseText,
            hub_url: normalizedHubUrl(env),
            forwarded_subject: payload.meta.subject,
            forwarded_to: payload.meta.to,
          },
          { status: response.ok ? 200 : 502 },
        );
      } catch (error) {
        return json(
          {
            status: 'error',
            message: error instanceof Error ? error.message : String(error),
          },
          { status: 500 },
        );
      }
    }

    return json({
      status: 'ok',
      service: 'phantom-drop-email-worker',
      supported_paths: ['/health', '/relay-test'],
    });
  },

  async email(message: MailMessage, env: Env) {
    try {
      const rawEmail = message.raw;
      const parser = new PostalMime();
      const parsed = (await parser.parse(rawEmail)) as ParsedMail;

      const payload: HubPayload = {
        meta: {
          from: message.from,
          to: message.to,
          subject: parsed.subject,
          date: parsed.date,
        },
        content: {
          text: parsed.text,
          html: parsed.html,
        },
        attachments: parsed.attachments.map((attachment) => ({
          filename: attachment.filename ?? null,
          mime_type: attachment.mimeType ?? null,
        })),
      };

      const response = await relayToHub(payload, env);
      const responseText = await response.text();
      if (!response.ok) {
        console.error(
          'Failed to relay message to PhantomDrop hub:',
          JSON.stringify({
            hub_status: response.status,
            hub_response: responseText,
            from: payload.meta.from,
            to: payload.meta.to,
            subject: payload.meta.subject,
          }),
        );
        throw new Error(`PhantomDrop hub relay failed with status ${response.status}`);
      }
      console.log(
        'Relayed message to PhantomDrop hub:',
        JSON.stringify({
          from: payload.meta.from,
          to: payload.meta.to,
          subject: payload.meta.subject,
        }),
      );
    } catch (error) {
      console.error('Email Worker processing failed:', error);
      throw error;
    }
  },
};
