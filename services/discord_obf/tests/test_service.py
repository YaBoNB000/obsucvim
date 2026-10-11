import copy
from dataclasses import replace
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import time
from unittest.mock import Mock

from nacl.signing import SigningKey
import pytest
import requests

from services.discord_obf import app as service
from services.discord_obf import register_command

KEY = SigningKey.generate()
URL = "https://cdn.discordapp.com/attachments/123/456/source.lua?ex=abc&hm=def"


@pytest.fixture
def config():
    return service.Config("100", KEY.verify_key.encode().hex(), frozenset({"200"}), sys.executable)


@pytest.fixture
def payload():
    return {
        "type": 2, "id": "300", "application_id": "100", "token": "test-interaction-token",
        "member": {"user": {"id": "200"}},
        "data": {"name": "obf", "options": [{"name": "file", "type": 11, "value": "456"}],
                 "resolved": {"attachments": {"456": {"filename": "source.lua", "size": 19, "url": URL}}}},
    }


@pytest.fixture
def endpoint(config):
    manager = service.JobManager(config, start=False)
    app = service.create_app(config, manager)
    app.testing = True
    return app.test_client(), manager


def signed(client, payload, timestamp=None, key=KEY):
    body = json.dumps(payload).encode()
    timestamp = str(int(time.time())) if timestamp is None else timestamp
    signature = key.sign(timestamp.encode() + body).signature.hex()
    return client.post("/interactions", data=body, content_type="application/json",
                       headers={"X-Signature-Timestamp": timestamp, "X-Signature-Ed25519": signature})


def test_health_ping_and_missing_signature(endpoint):
    client, manager = endpoint
    assert client.get("/healthz").json == {"status": "ok"}
    assert signed(client, {"type": 1}).json == {"type": 1}
    assert client.post("/interactions", json={"type": 1}).status_code == 401
    assert manager.jobs.empty()


def test_bad_signature_and_stale_timestamp(endpoint, payload):
    client, _ = endpoint
    assert signed(client, payload, key=SigningKey.generate()).status_code == 401
    assert signed(client, payload, str(int(time.time()) - 301)).status_code == 401
    assert signed(client, payload, str(int(time.time()) + 301)).status_code == 401
    assert signed(client, payload, "bad").status_code == 401
    body = b'{"type": 1}'
    stamp = str(int(time.time()))
    sig = KEY.sign(stamp.encode() + body).signature.hex()
    assert client.post("/interactions", data=body + b" ", headers={
        "X-Signature-Timestamp": stamp, "X-Signature-Ed25519": sig}).status_code == 401


def test_defer_is_immediate_private_and_deduplicated(endpoint, payload):
    client, manager = endpoint
    before = time.monotonic()
    result = signed(client, payload)
    assert time.monotonic() - before < 1
    assert result.json == {"type": 5, "data": {"flags": 64}}
    assert signed(client, payload).json == result.json
    assert manager.jobs.qsize() == 1
    assert "test-interaction-token" not in repr(manager.jobs.queue[0])
    assert URL not in repr(manager.jobs.queue[0])


def test_authorization_and_wrong_app(endpoint, payload):
    client, manager = endpoint
    payload["member"]["user"]["id"] = "999"
    assert "白名单" in signed(client, payload).json["data"]["content"]
    payload["application_id"] = "999"
    assert signed(client, payload).status_code == 400
    assert manager.jobs.empty()


def test_dm_user(endpoint, payload):
    client, _ = endpoint
    payload["user"] = payload.pop("member")["user"]
    assert signed(client, payload).json["type"] == 5


@pytest.mark.parametrize("bad", [None, [], "hi", {"type": 2}, {"type": 2, "application_id": "100", "data": []}])
def test_malformed_payload_no_500(endpoint, bad):
    client, manager = endpoint
    assert signed(client, bad).status_code in {200, 400}
    assert manager.jobs.empty()


def test_request_size_limit(endpoint):
    client, _ = endpoint
    assert client.post("/interactions", data=b"x" * (128 * 1024 + 1)).status_code == 413


@pytest.mark.parametrize("field,value", [("size", 0), ("size", True), ("size", service.MAX_INPUT + 1),
                                         ("filename", "x.exe"), ("url", "http://127.0.0.1/secret")])
def test_invalid_attachment(endpoint, payload, field, value):
    client, manager = endpoint
    payload["data"]["resolved"]["attachments"]["456"][field] = value
    assert signed(client, payload).json["type"] == 4
    assert manager.jobs.empty()


@pytest.mark.parametrize("url", [
    "http://cdn.discordapp.com/attachments/1/2/a.lua",
    "https://cdn.discordapp.com.evil.test/attachments/1/2/a.lua",
    "https://cdn.discordapp.com@127.0.0.1/attachments/1/2/a.lua",
    "https://user:pass@cdn.discordapp.com/attachments/1/2/a.lua",
    "https://cdn.discordapp.com:444/attachments/1/2/a.lua",
    "https://cdn.discordapp.com:bad/attachments/1/2/a.lua",
    "https://discord.com/api/v10/users/@me", "file:///etc/passwd", None,
])
def test_ssrf_urls_blocked(url):
    assert not service.attachment_url_allowed(url)


def test_cdn_signed_url_allowed():
    assert service.attachment_url_allowed(URL)
    assert service.attachment_url_allowed(URL.replace("cdn.discordapp.com", "media.discordapp.net"))


def test_no_extra_options_or_token_path_injection(endpoint, payload):
    client, manager = endpoint
    payload["data"]["options"].append({"name": "flags", "type": 3, "value": "; id"})
    assert signed(client, payload).json["type"] == 4
    payload["data"]["options"].pop()
    payload["token"] = "../../secret"
    assert signed(client, payload).json["type"] == 4
    assert manager.jobs.empty()


def test_queue_busy_cooldown_capacity_and_replay(config, payload):
    manager = service.JobManager(config, start=False)
    job = service.parse_job(payload, "200")
    assert manager.submit(job) is None
    assert "任务" in manager.submit(replace(job, id="301"))
    manager.active.remove("200")
    assert "频繁" in manager.submit(replace(job, id="301"))
    for i in range(2):
        assert manager.submit(replace(job, id=str(400 + i), user_id=str(500 + i))) is None
    assert "队列已满" in manager.submit(replace(job, id="999", user_id="999"))
    assert "999" not in manager.active
    assert manager.submit(job) is None
    assert manager.jobs.qsize() == 3


def test_worker_survives_failure_and_releases_user(config, payload, caplog):
    calls = []
    event = threading.Event()

    def processor(cfg, job):
        calls.append(job.id)
        if len(calls) == 1:
            raise RuntimeError("SECRET token source")
        event.set()

    manager = service.JobManager(config, processor=processor)
    job = service.parse_job(payload, "200")
    manager.submit(job)
    manager.submit(replace(job, id="301", user_id="201"))
    assert event.wait(2)
    manager.jobs.join()
    assert not manager.active
    assert calls == ["300", "301"]
    assert "SECRET" not in caplog.text


class Response:
    def __init__(self, content=b'local a = "bysuda"\n', status=200, headers=None):
        self.content = content
        self.status_code = status
        self.headers = headers or {}

    def iter_content(self, size):
        for i in range(0, len(self.content), size):
            yield self.content[i:i + size]

    def __enter__(self):
        return self

    def __exit__(self, *args):
        pass

    def close(self):
        pass

    def json(self):
        return {"retry_after": 0}


def test_stream_download_and_no_redirects(config, payload, tmp_path):
    session = Mock()
    session.get.return_value = Response()
    path = tmp_path / "in.lua"
    service.download_source(session, service.parse_job(payload, "200"), path)
    assert path.read_bytes() == b'local a = "bysuda"\n'
    assert session.get.call_args.kwargs["allow_redirects"] is False


@pytest.mark.parametrize("response,size", [
    (Response(status=302), 19), (Response(headers={"Content-Length": str(service.MAX_INPUT + 1)}), 19),
    (Response(headers={"Content-Length": "bad"}), 19),
    (Response(content=b"a" * (service.MAX_INPUT + 1)), 19),
    (Response(content=b"abc"), 19), (Response(content=b"\xff"), 1), (Response(content=b"\x00"), 1),
])
def test_download_rejections(response, size, payload, tmp_path):
    session = Mock()
    session.get.return_value = response
    job = replace(service.parse_job(payload, "200"), size=size)
    with pytest.raises(service.JobError):
        service.download_source(session, job, tmp_path / "in.lua")


def test_config_fails_closed(config):
    for changes in [{"allowed_users": frozenset()}, {"allowed_users": frozenset({"*"})},
                    {"public_key": "bad"}, {"application_id": "bad"}, {"engine": "relative"}]:
        with pytest.raises(ValueError):
            replace(config, **changes)


def test_runner_uses_fixed_args_minimal_environment(config, tmp_path, monkeypatch):
    source, output = tmp_path / "input.lua", tmp_path / "output.luau"
    source.write_text("return 1")
    output.write_text("result")
    run = Mock(return_value=subprocess.CompletedProcess([], 0))
    monkeypatch.setattr(service.subprocess, "run", run)
    monkeypatch.setenv("DISCORD_BOT_TOKEN", "secret")
    service.run_engine(config, source, output)
    args, kwargs = run.call_args
    assert "--preset" in args[0] and "v15" in args[0]
    assert "--dialect" in args[0] and "luau" in args[0]
    assert "limited_exec.py" in args[0][1]
    assert "DISCORD_BOT_TOKEN" not in kwargs["env"]
    assert "shell" not in kwargs and "preexec_fn" not in kwargs
    assert kwargs["stderr"] == subprocess.DEVNULL


@pytest.mark.parametrize("mode", ["timeout", "error", "empty", "oversize"])
def test_runner_failure_modes(config, tmp_path, monkeypatch, mode):
    source, output = tmp_path / "in.lua", tmp_path / "out.lua"
    source.write_text("bad input")
    run = Mock(return_value=subprocess.CompletedProcess([], 1 if mode == "error" else 0))
    if mode == "timeout":
        run.side_effect = subprocess.TimeoutExpired("internal", 90)
    if mode == "oversize":
        with output.open("wb") as file:
            file.truncate(service.MAX_OUTPUT + 1)
    monkeypatch.setattr(service.subprocess, "run", run)
    with pytest.raises(service.JobError):
        service.run_engine(config, source, output)


def test_actual_launcher_limits(tmp_path):
    # Checks real OS limits after exec, independently of an engine installation.
    launcher = Path(service.__file__).with_name("limited_exec.py")
    code = "import resource,json; print(json.dumps([resource.getrlimit(x) for x in [resource.RLIMIT_AS,resource.RLIMIT_FSIZE,resource.RLIMIT_CPU,resource.RLIMIT_CORE]]))"
    result = subprocess.run([sys.executable, str(launcher), "256", "4096", "2", sys.executable, "-c", code],
                            capture_output=True, check=True, timeout=10)
    assert json.loads(result.stdout) == [[256 * 1024 * 1024] * 2, [4096] * 2, [2, 3], [0, 0]]


def test_upload_multipart_and_retries(config, payload, tmp_path, monkeypatch):
    path = tmp_path / "output.luau"
    path.write_text("result")
    session = Mock()
    contents = []

    def patch(url, **kwargs):
        assert url.endswith("/messages/@original")
        assert kwargs["allow_redirects"] is False
        assert json.loads(kwargs["data"]["payload_json"])["allowed_mentions"] == {"parse": []}
        contents.append(kwargs["files"]["files[0]"][1].read())
        return Response(status=429 if len(contents) == 1 else 200)

    session.patch.side_effect = patch
    monkeypatch.setattr(service.time, "sleep", lambda _: None)
    service.edit_reply(session, config, service.parse_job(payload, "200"), "ok", path)
    assert contents == [b"result", b"result"]


def test_reply_failure_is_bounded_and_secret_free(config, payload, monkeypatch):
    session = Mock()
    session.patch.side_effect = requests.RequestException("secret URL")
    monkeypatch.setattr(service.time, "sleep", lambda _: None)
    with pytest.raises(service.JobError) as error:
        service.edit_reply(session, config, service.parse_job(payload, "200"), "failed")
    assert "secret" not in str(error.value)
    assert session.patch.call_count == 3


@pytest.mark.parametrize("fail", [False, True])
def test_job_cleanup_success_and_error(config, payload, monkeypatch, fail):
    directories = []
    replies = []

    def download(session, job, path):
        directories.append(path.parent)
        path.write_text("source")

    def engine(cfg, source, output):
        if fail:
            raise service.JobError("expected failure")
        output.write_text("result")

    def reply(session, cfg, job, content, output=None):
        replies.append((content, output.read_text() if output else None))

    monkeypatch.setattr(service, "download_source", download)
    monkeypatch.setattr(service, "run_engine", engine)
    monkeypatch.setattr(service, "edit_reply", reply)
    service.process_job(config, service.parse_job(payload, "200"))
    assert not directories[0].exists()
    assert replies[0][1] == (None if fail else "result")


def test_register_only_obf_not_bulk_overwrite(monkeypatch):
    monkeypatch.setenv("DISCORD_APPLICATION_ID", "100")
    monkeypatch.setenv("DISCORD_GUILD_ID", "200")
    monkeypatch.setenv("DISCORD_BOT_TOKEN", "secret")
    session = Mock()
    session.post.return_value = Response(status=201)
    context = Mock()
    context.__enter__ = Mock(return_value=session)
    context.__exit__ = Mock(return_value=False)
    monkeypatch.setattr(register_command.requests, "Session", lambda: context)
    assert register_command.main() == 0
    assert session.post.call_args.args[0].endswith("/applications/100/guilds/200/commands")
    assert session.post.call_args.kwargs["json"]["options"][0]["type"] == 11
    session.put.assert_not_called()


@pytest.mark.skipif(not os.environ.get("TEST_OBSUCVIM_BIN"), reason="set TEST_OBSUCVIM_BIN for real-engine integration")
def test_full_pipeline_real_engine_mock_discord(config, payload, monkeypatch):
    config = replace(config, engine=os.environ["TEST_OBSUCVIM_BIN"])
    artifacts = []
    paths = []
    original = service.run_engine

    def run(cfg, source, output):
        paths.append(source.parent)
        original(cfg, source, output)

    class DiscordSession:
        trust_env = True

        def __enter__(self):
            return self

        def __exit__(self, *args):
            pass

        def get(self, url, **kwargs):
            return Response()

        def patch(self, url, **kwargs):
            artifacts.append(kwargs["files"]["files[0]"][1].read())
            return Response()

    monkeypatch.setattr(service.requests, "Session", DiscordSession)
    monkeypatch.setattr(service, "run_engine", run)
    service.process_job(config, service.parse_job(payload, "200"))
    assert len(artifacts) == 1 and len(artifacts[0]) > 1000
    assert artifacts[0].startswith(b"-- This file was obfuscated by Obsucvim.\n")
    assert b"bysuda" not in artifacts[0]
    assert not paths[0].exists()


def test_concurrent_replay_admitted_once(config, payload):
    from concurrent.futures import ThreadPoolExecutor
    manager = service.JobManager(config, start=False)
    job = service.parse_job(payload, "200")
    with ThreadPoolExecutor(max_workers=8) as pool:
        assert all(result is None for result in pool.map(manager.submit, [job] * 32))
    assert manager.jobs.qsize() == 1


def test_real_subprocess_wall_timeout(config, tmp_path, monkeypatch):
    engine = tmp_path / "sleep-engine"
    engine.write_text(f"#!{sys.executable}\nimport time\ntime.sleep(10)\n")
    engine.chmod(0o700)
    config = replace(config, engine=str(engine))
    monkeypatch.setattr(service, "ENGINE_TIMEOUT", 1)
    source = tmp_path / "in.lua"
    source.write_text("return 1")
    before = time.monotonic()
    with pytest.raises(service.JobError, match="终止"):
        service.run_engine(config, source, tmp_path / "out.lua")
    assert time.monotonic() - before < 4


@pytest.mark.skipif(not os.environ.get("TEST_OBSUCVIM_BIN"), reason="set TEST_OBSUCVIM_BIN for real-engine integration")
@pytest.mark.parametrize("source,valid", [('error("must never execute")', True), ('local = broken(', False)])
def test_real_engine_compiles_without_executing_or_reports_error(config, source, valid, tmp_path):
    config = replace(config, engine=os.environ["TEST_OBSUCVIM_BIN"])
    path = tmp_path / "input.lua"
    path.write_text(source)
    output = tmp_path / "output.luau"
    if valid:
        service.run_engine(config, path, output)
        assert output.stat().st_size > 1000
    else:
        with pytest.raises(service.JobError, match="混淆失败"):
            service.run_engine(config, path, output)


def test_cleanup_even_when_upload_fails(config, payload, monkeypatch):
    paths = []

    def download(session, job, path):
        paths.append(path.parent)
        path.write_text("source")

    def engine(cfg, source, output):
        output.write_text("result")

    monkeypatch.setattr(service, "download_source", download)
    monkeypatch.setattr(service, "run_engine", engine)
    monkeypatch.setattr(service, "edit_reply", Mock(side_effect=service.JobError("upload failed")))
    with pytest.raises(service.JobError):
        service.process_job(config, service.parse_job(payload, "200"))
    assert not paths[0].exists()
