# Prism HTTPS domain access

2026-09-10. Deployed and verified for the user's cross-device manual acceptance request.
This extends the previous deployment scope only for CPAR's existing domain and management origin.
The prior loopback-only default remains available when the option is omitted.

Current runtime: `c7cfd2c0d8771187775d2cd846cb49a8cf536046`; previous runtime: `18f29a3`.
Signed artifact, public HTTPS write/readback and browser evidence: [delivery report](../reports/prism-domain-delivery.md).
The current entry is the existing approved `cpar` domain, with `/` redirecting to `/admin-ui/`.

## Service and Caddy

Run the service with its existing arguments plus:

```sh
--management-origin https://cpar.example.com
```

Use one exact canonical HTTPS origin with no trailing slash, path, query or fragment.
The management listener still binds `127.0.0.1:18181`; Caddy remains the TLS entry point.
The declared origin replaces the loopback browser origin. Origin-free authenticated CLI requests
continue to work; browser writes from the old SSH-forwarded HTTP origin are rejected.

Apply only the approved hostname's routing from
[the Caddy example](../../deploy/caddy/prism-domain.Caddyfile), preserving existing site directives
and the existing data proxy configuration. Forward Origin unchanged. Do not embed keys or CSRF
values into Caddy. Routing uses mutually exclusive `handle` blocks as described by
[Caddy's official documentation](https://caddyserver.com/docs/caddyfile/directives/handle).

The base systemd template is unchanged. For an existing installation, an operator-owned drop-in
may replace ExecStart with its existing argv plus the explicit origin; keep all current proxy
arguments and the systemd `%d` credential directory. Back up and validate before restarting.

## Acceptance

The expected user entry is `https://<approved-cpar-host>/admin-ui/`; exact `/` redirects there.
Enter the existing Management Key and CSRF Token. Client Keys and Provider credentials cannot
unlock Prism. Secrets remain only in page memory and refresh locks the page.

Verify TLS/UI/four assets, authenticated read and a dedicated unpublished draft write/readback, wrong-origin and
missing-CSRF denial, anonymous denial, the existing public `/healthz` and authenticated `/v1/models`,
and unchanged other Caddy sites/Autoreg. These checks send no Provider inference request.
Actual access on the user's other physical devices remains their manual acceptance step.

## Recovery boundary

Restore this operation's previous binary, service drop-in state and only its Caddy site change.
Both old and new artifacts have schema 22, so do not drop that migration or its failure queue.
The 87 historical legacy Usage records and the previously observed admission 503s remain separate
project follow-ups; this domain-access change does not claim to resolve them.
