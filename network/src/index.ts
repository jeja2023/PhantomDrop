import PostalMime from 'postal-mime';

export interface Env {
  PHANTOM_HUB_URL: string;
  HUB_SECRET: string;
}

interface MailMessage {
  from: string;
  to: string;
  asRaw(): Promise<ArrayBuffer>;
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
  return env.PHANTOM_HUB_URL.replace(/\/+$/, '');
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
  return fetch(`${normalizedHubUrl(env)}/ingest`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-Hub-Secret': env.HUB_SECRET.trim(),
    },
    body: JSON.stringify(payload),
  });
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
      return json({
        status: 'ok',
        worker: 'phantom-drop-edge',
        hub_url: normalizedHubUrl(env),
      });
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
            forwarded_subject: payload.meta.subject,
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
      const rawEmail = await message.asRaw();
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
      if (!response.ok) {
        console.error('Failed to relay message to PhantomDrop hub:', await response.text());
      }
    } catch (error) {
      console.error('Email Worker processing failed:', error);
    }
  },
};
