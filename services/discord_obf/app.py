"""Single-process, bounded Discord interaction service (no Gateway/bot token)."""
from dataclasses import dataclass, field
import json
import logging
import os
from pathlib import Path
import queue
import re
import secrets
import subprocess
import sys
import tempfile
import threading
import time
from urllib.parse import urlsplit

from flask import Flask, jsonify, request
from nacl.exceptions import BadSignatureError
from nacl.signing import VerifyKey
import requests

LOG = logging.getLogger(__name__)
SNOWFLAKE = re.compile(r"[0-9]{1,20}\Z", re.ASCII)
MAX_INPUT = 256 * 1024
MAX_OUTPUT = 7 * 1024 * 1024
ENGINE_TIMEOUT = 90
MEMORY_MB = 256
QUEUE_SIZE = 3
COOLDOWN = 30
API = "https://discord.com/api/v10"


@dataclass(frozen=True)
class Config:
    application_id: str
    public_key: str
    allowed_users: frozenset[str]
    engine: str

    def __post_init__(self):
        if not SNOWFLAKE.fullmatch(self.application_id):
            raise ValueError("DISCORD_APPLICATION_ID must be a numeric application ID")
        if not re.fullmatch(r"[a-fA-F0-9]{64}", self.public_key):
            raise ValueError("DISCORD_PUBLIC_KEY must contain 64 hex characters")
        if not self.allowed_users or any(not SNOWFLAKE.fullmatch(u) for u in self.allowed_users):
            raise ValueError("ALLOWED_USER_IDS must be a nonempty comma-separated list of user IDs")
        if not Path(self.engine).is_absolute() or not os.access(self.engine, os.X_OK):
            raise ValueError("OBSUCVIM_BIN must be an absolute path to an executable engine")

    @classmethod
    def from_env(cls):
        return cls(
            os.environ.get("DISCORD_APPLICATION_ID", ""),
            os.environ.get("DISCORD_PUBLIC_KEY", ""),
            frozenset(x.strip() for x in os.environ.get("ALLOWED_USER_IDS", "").split(",") if x.strip()),
            os.environ.get("OBSUCVIM_BIN", "/usr/local/bin/obsucvim"),
        )


@dataclass(frozen=True)
class Job:
    id: str
    user_id: str
    token: str = field(repr=False)
    url: str = field(repr=False)
    size: int


class JobError(Exception):
    """Fixed, public-safe message; never include source, stderr or URLs."""


def attachment_url_allowed(url):
    if not isinstance(url, str) or len(url) > 4096:
        return False
    try:
        p = urlsplit(url)
        return (
            p.scheme == "https"
            and p.hostname in {"cdn.discordapp.com", "media.discordapp.net"}
            and p.port in {None, 443}
            and p.username is None and p.password is None and not p.fragment
            and re.fullmatch(r"/attachments/[0-9]+/[0-9]+/[^/]+", p.path) is not None
        )
    except ValueError:
        return False


def parse_job(payload, user_id):
    data = payload["data"]
    options = data.get("options", [])
    if not isinstance(options, list) or len(options) != 1:
        raise ValueError("expected exactly one attachment")
    option = options[0]
    if option.get("name") != "file" or option.get("type") != 11:
        raise ValueError("expected attachment option")
    attachment_id = option["value"]
    if not isinstance(attachment_id, str) or not SNOWFLAKE.fullmatch(attachment_id):
        raise ValueError("invalid attachment ID")
    attachment = data["resolved"]["attachments"][attachment_id]
    filename = attachment["filename"]
    if not isinstance(filename, str) or not filename.lower().endswith((".lua", ".luau")):
        raise ValueError("unsupported file extension")
    size = attachment["size"]
    if type(size) is not int or not 0 < size <= MAX_INPUT:
        raise ValueError("invalid size")
    url = attachment["url"]
    if not attachment_url_allowed(url):
        raise ValueError("invalid download URL")
    token = payload["token"]
    if not isinstance(token, str) or not re.fullmatch(r"[A-Za-z0-9._-]{1,512}", token):
        raise ValueError("invalid interaction token")
    job_id = payload["id"]
    if not isinstance(job_id, str) or not SNOWFLAKE.fullmatch(job_id):
        raise ValueError("invalid interaction ID")
    return Job(job_id, user_id, token, url, size)


def download_source(session, job, target):
    # No redirects, credentials, arbitrary hosts, or unbounded response bodies.
    deadline = time.monotonic() + 30
    with session.get(job.url, stream=True, timeout=(5, 10), allow_redirects=False) as response:
        if response.status_code != 200:
            raise JobError("附件下载失败，请重新上传后再试。")
        if response.headers.get("Content-Length"):
            try:
                length = int(response.headers["Content-Length"])
            except ValueError:
                raise JobError("附件大小无效。") from None
            if not 0 < length <= MAX_INPUT:
                raise JobError("附件超过 256 KiB 限制。")
        total = 0
        with target.open("wb") as output:
            for chunk in response.iter_content(8192):
                total += len(chunk)
                if total > MAX_INPUT or time.monotonic() > deadline:
                    raise JobError("附件过大或下载超时。")
                output.write(chunk)
    if total != job.size:
        raise JobError("附件大小不匹配，请重新上传。")
    try:
        source = target.read_text(encoding="utf-8")
    except UnicodeError:
        raise JobError("请上传 UTF-8 编码的 Lua / Luau 源码。") from None
    if "\x00" in source:
        raise JobError("源码不能包含原始 NUL 字节。")


def run_engine(config, source, output):
    # Limits are installed by a separate launcher, NOT preexec_fn in this
    # multi-threaded web process. No shell, user paths, flags, or Lua execution.
    launcher = Path(__file__).with_name("limited_exec.py")
    command = [sys.executable, str(launcher), str(MEMORY_MB), str(MAX_OUTPUT),
               str(ENGINE_TIMEOUT), config.engine, "--preset", "v15", "--dialect", "luau",
               "--seed", str(secrets.randbits(64)), str(source), str(output)]
    try:
        result = subprocess.run(
            command, cwd=source.parent, env={"PATH": "/usr/local/bin:/usr/bin:/bin", "LANG": "C.UTF-8"},
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            timeout=ENGINE_TIMEOUT, check=False, close_fds=True,
        )
    except subprocess.TimeoutExpired:
        raise JobError("混淆超过 90 秒，任务已终止；请缩小脚本后重试。") from None
    if result.returncode != 0:
        raise JobError("混淆失败：请检查语法，或缩小脚本以满足资源限制。")
    if not output.is_file() or not 0 < output.stat().st_size <= MAX_OUTPUT:
        raise JobError("混淆结果为空或超过 7 MiB 限制。")


def edit_reply(session, config, job, content, output=None):
    url = f"{API}/webhooks/{config.application_id}/{job.token}/messages/@original"
    payload = {"content": content, "allowed_mentions": {"parse": []}, "attachments": []}
    if output:
        payload["attachments"] = [{"id": 0, "filename": "obfuscated.luau"}]
    # Editing @original is idempotent. Bounded retries handle transient errors
    # and the small possibility that the deferred acknowledgement is still in flight.
    for attempt in range(3):
        try:
            if output:
                with output.open("rb") as file:
                    response = session.patch(url, data={"payload_json": json.dumps(payload)},
                                             files={"files[0]": ("obfuscated.luau", file, "text/plain")},
                                             timeout=(5, 20), allow_redirects=False)
            else:
                response = session.patch(url, json=payload, timeout=(5, 20), allow_redirects=False)
        except requests.RequestException:
            if attempt == 2:
                raise JobError("Discord 回传失败。") from None
            time.sleep(1 + attempt)
            continue
        status = response.status_code
        delay = 1 + attempt
        if status == 429:
            try:
                delay = max(1, min(float(response.json().get("retry_after", 1)), 5))
            except (ValueError, TypeError, AttributeError):
                delay = 2
        response.close()
        if 200 <= status < 300:
            return
        if (status not in {404, 429} and status < 500) or attempt == 2:
            raise JobError("Discord 回传失败。")
        time.sleep(delay)


def process_job(config, job):
    with requests.Session() as session:
        session.trust_env = False
        # Temp source/results deleted on success, engine error, or upload error.
        with tempfile.TemporaryDirectory(prefix="obsucvim-") as directory:
            source, output = Path(directory) / "input.lua", Path(directory) / "output.luau"
            try:
                download_source(session, job, source)
                run_engine(config, source, output)
            except JobError as exc:
                edit_reply(session, config, job, str(exc))
                return
            except (requests.RequestException, OSError):
                edit_reply(session, config, job, "处理失败，请稍后重新上传。")
                return
            edit_reply(session, config, job, "混淆完成（v15 / Luau）。结果仅对你可见。", output)


class JobManager:
    def __init__(self, config, processor=process_job, start=True):
        self.config = config
        self.processor = processor
        self.jobs = queue.Queue(maxsize=QUEUE_SIZE)
        self.lock = threading.Lock()
        self.active = set()
        self.last = {}
        self.seen = {}
        if start:
            threading.Thread(target=self._work, name="obfuscation-worker", daemon=True).start()

    def submit(self, job):
        now = time.monotonic()
        with self.lock:
            self.seen = {key: ts for key, ts in self.seen.items() if now - ts < 900}
            if job.id in self.seen:
                return None  # Replay gets the same defer ACK, never a second job.
            if job.user_id in self.active:
                return "你已有一个任务正在排队或处理中。"
            if now - self.last.get(job.user_id, -COOLDOWN) < COOLDOWN:
                return "请求太频繁，请等 30 秒后再试。"
            if len(self.seen) >= 4096:
                return "服务繁忙，请稍后重试。"
            try:
                self.jobs.put_nowait(job)
            except queue.Full:
                return "队列已满，请稍后重试。"
            self.active.add(job.user_id)
            self.last[job.user_id] = now
            self.seen[job.id] = now
        return None

    def _work(self):
        while True:
            job = self.jobs.get()
            try:
                self.processor(self.config, job)
            except Exception:
                # Deliberately exclude exception/traceback: HTTP errors can
                # include signed attachment URLs or interaction webhook tokens.
                LOG.error("Job %s failed; details suppressed to protect source and tokens", job.id)
            finally:
                with self.lock:
                    self.active.discard(job.user_id)
                self.jobs.task_done()


def create_app(config=None, manager=None):
    config = config or Config.from_env()
    verify_key = VerifyKey(bytes.fromhex(config.public_key))
    manager = manager or JobManager(config)
    app = Flask(__name__)
    app.config["MAX_CONTENT_LENGTH"] = 128 * 1024

    def message(text):
        return jsonify({"type": 4, "data": {"content": text, "flags": 64,
                                           "allowed_mentions": {"parse": []}}})

    @app.get("/healthz")
    def health():
        return jsonify({"status": "ok"})

    @app.post("/interactions")
    def interactions():
        body = request.get_data()
        timestamp = request.headers.get("X-Signature-Timestamp", "")
        signature = request.headers.get("X-Signature-Ed25519", "")
        try:
            if not re.fullmatch(r"[0-9]{1,12}", timestamp) or abs(time.time() - int(timestamp)) > 300:
                raise ValueError("expired timestamp")
            if not re.fullmatch(r"[a-fA-F0-9]{128}", signature):
                raise ValueError("bad signature format")
            verify_key.verify(timestamp.encode("ascii") + body, bytes.fromhex(signature))
        except (ValueError, BadSignatureError):
            return jsonify({"error": "invalid signature"}), 401
        try:
            payload = json.loads(body)
            if not isinstance(payload, dict):
                raise ValueError("expected object")
            if payload.get("type") == 1:
                return jsonify({"type": 1})
            if payload.get("application_id") != config.application_id:
                return jsonify({"error": "wrong application"}), 400
            if payload.get("type") != 2 or payload["data"].get("name") != "obf":
                return message("仅支持 /obf file:<附件>。")
            user = payload.get("member", {}).get("user") or payload.get("user", {})
            user_id = user.get("id")
            if not isinstance(user_id, str) or user_id not in config.allowed_users:
                return message("你不在此机器人的使用白名单中。")
            job = parse_job(payload, user_id)
        except (ValueError, KeyError, TypeError, AttributeError):
            return message("请用 /obf 上传一个不超过 256 KiB 的 .lua 或 .luau 文件。")
        error = manager.submit(job)
        if error:
            return message(error)
        return jsonify({"type": 5, "data": {"flags": 64}})

    return app
