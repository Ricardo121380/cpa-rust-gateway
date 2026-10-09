"""Fixed-origin synthetic device server for the stock-serve acceptance process only."""
import json
import base64
import pathlib
import secrets
import threading
import urllib.parse


class DeviceFixture:
    hosts = ("auth.x.ai", "auth.kimi.com", "oidc.us-east-1.amazonaws.com")

    def __init__(self, out):
        self.out = pathlib.Path(out)
        self.sessions = {}
        self.events = []
        self.refresh_requests = 0
        self.access_tokens = set()
        self.refresh_tokens = set()
        self.lock = threading.Lock()

    def snapshot(self):
        with self.lock:
            # No tokens, device codes, authorization bodies or device identifiers in evidence.
            return {"events": list(self.events), "refresh_requests": self.refresh_requests, "sessions": [
                {key: row[key] for key in ("ordinal", "channel", "mode", "token_requests")}
                for row in self.sessions.values()
            ]}

    def build_grant(self):
        claims = base64.urlsafe_b64encode(json.dumps({"sub": "synthetic-build-owner", "nonce": secrets.token_hex(8)}).encode()).decode().rstrip("=")
        access, refresh = "header." + claims + ".signature", secrets.token_hex(12)
        self.access_tokens.add(access)
        self.refresh_tokens.add(refresh)
        return {"access_token": access, "refresh_token": refresh, "expires_in": 3600, "token_type": "Bearer"}

    def accepts_build_token(self, token):
        with self.lock:
            return token in self.access_tokens

    def user_info(self, handler):
        if handler.headers.get("Host", "").split(":")[0] != "auth.x.ai":
            return False
        assert handler.path == "/oauth2/userinfo"
        with self.lock:
            self.events.append({"host": "auth.x.ai", "path": handler.path, "method": "GET"})
        handler.reply(200, {"sub": "synthetic-build-owner", "email": "build.member@example.test"})
        return True

    def handle(self, handler):
        host = handler.headers.get("Host", "").split(":")[0]
        if host not in self.hosts:
            return False
        raw = handler.rfile.read(int(handler.headers.get("Content-Length", "0")))
        channel = "kiro" if host.startswith("oidc.") else "grok.build" if host == "auth.x.ai" else "kimi-coding"
        content_type = handler.headers.get("Content-Type", "")
        body = json.loads(raw) if channel == "kiro" else {
            key: values[0] for key, values in urllib.parse.parse_qs(raw.decode()).items()
        }
        with self.lock:
            self.events.append({"host": host, "path": handler.path, "channel": channel,
                                "content_type": content_type, "field_names": sorted(body)})
            if handler.path.endswith("/client/register"):
                handler.reply(200, {"clientId": "synthetic-client", "clientSecret": secrets.token_hex(12)})
                return True
            if handler.path.endswith("/device_authorization") or handler.path == "/oauth2/device/code":
                control = self.out / "device-control.json"
                mode = json.loads(control.read_text()).get("mode", "success") if control.exists() else "success"
                assert mode in {"success", "pending_then_success", "denied", "expired", "failed", "ttl", "transport"}
                code = secrets.token_hex(12)
                self.sessions[code] = {"ordinal": len(self.sessions) + 1, "channel": channel,
                                       "mode": mode, "token_requests": 0}
                expiry = 1 if mode == "ttl" else 120
                if channel == "kiro":
                    reply = {"deviceCode": code, "userCode": "CPAR-SYNTHETIC", "verificationUri": "https://view.awsapps.com/start",
                             "expiresIn": expiry, "interval": 5}
                else:
                    reply = {"device_code": code, "user_code": "CPAR-SYNTHETIC", "verification_uri": "https://auth.x.ai/activate" if channel == "grok.build" else "https://auth.kimi.com/activate",
                             "expires_in": expiry, "interval": 5}
                handler.reply(200, reply)
                return True
            if handler.path.endswith("/token"):
                if channel == "grok.build" and body.get("grant_type") == "refresh_token":
                    assert body.get("refresh_token") in self.refresh_tokens
                    self.refresh_requests += 1
                    handler.reply(200, self.build_grant())
                    return True
                row = self.sessions[body.get("deviceCode") or body.get("device_code")]
                assert row["channel"] == channel
                row["token_requests"] += 1
                mode = row["mode"]
                if mode == "transport":
                    handler.close_connection = True
                    return True
                error = {"denied": "access_denied", "expired": "expired_token", "failed": "invalid_grant"}.get(mode)
                if mode == "pending_then_success" and row["token_requests"] == 1:
                    error = "authorization_pending"
                if error:
                    handler.reply(400, {"error": error})
                elif channel == "kiro":
                    handler.reply(200, {"accessToken": secrets.token_hex(12), "refreshToken": secrets.token_hex(12), "expiresIn": 3600})
                elif channel == "grok.build":
                    handler.reply(200, self.build_grant())
                else:
                    handler.reply(200, {"access_token": secrets.token_hex(12), "refresh_token": secrets.token_hex(12), "expires_in": 3600})
                return True
            handler.reply(404, {"error": "unexpected_fixture_path"})
            return True
