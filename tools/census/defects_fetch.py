#!/usr/bin/env python3
"""Fetch public issue reports anonymously into a local cache, for `census.defects`.

Reads FreeCAD's and OCCT's public issues through GitHub's unauthenticated API. Comments are skipped, since each costs a
request. The cache keeps only what the survey needs: never the author, assignee or reactions. Nothing here reads the
text it stores; `defects_strip.py` does that, and only its output is ever read by a person or an agent.

Rules this file enforces, each with a test against a fake:

- anonymous, with the generic client name `occam-census` and no contact parameter of any kind;
- 100 issues per page, every page cached on disk, a run resumes from the cache;
- reads `X-RateLimit-Remaining` and stops at 20 until the reset time, because the anonymous 60 an hour is shared;
- spends at most a fixed budget of requests in any rolling hour, counted across restarts.

Exit status: 0 when every job finished, 75 when it stopped to wait (`resume_at` says until when), 1 on an error.
Python standard library only.
"""

import argparse
import json
import os
import re
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import asdict, dataclass
from pathlib import Path

API = "https://api.github.com"
USER_AGENT = "occam-census"
PER_PAGE = 100
RATE_FLOOR = 20
HOURLY_BUDGET = 40
TIMEOUT_SECONDS = 30
EXIT_WAIT = 75

DEFAULT_CACHE = Path.home() / ".cache" / "occam-census" / "issues"

KEPT_ISSUE_FIELDS = ("number", "title", "body", "state")


@dataclass(frozen=True)
class Job:
    """One walk over a repository's issues, optionally narrowed to a label."""

    repo: str
    label: str | None = None
    state: str = "all"
    sort: str = "created"
    max_pages: int = 5


@dataclass
class Response:
    status: int
    headers: dict
    body: bytes


@dataclass
class Outcome:
    requests: int = 0
    pages_cached: int = 0
    pages_reused: int = 0
    issues_seen: int = 0
    stopped: str | None = None
    resume_at: int | None = None
    detail: str = ""


def urllib_opener(url, headers):
    """The real transport: a GET that reports HTTP errors as responses instead of raising."""
    request = urllib.request.Request(url, headers=headers, method="GET")
    try:
        with urllib.request.urlopen(request, timeout=TIMEOUT_SECONDS) as reply:
            return Response(reply.status, dict(reply.headers.items()), reply.read())
    except urllib.error.HTTPError as error:
        return Response(error.code, dict(error.headers.items()), error.read())


def request_headers():
    """Every header this tool sends. Deliberately no token, contact address or `From`."""
    return {
        "User-Agent": USER_AGENT,
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
    }


def _slug(text):
    return re.sub(r"[^A-Za-z0-9]+", "-", text).strip("-").lower() or "x"


def page_path(cache_dir, job, page):
    repo = job.repo.replace("/", "__")
    label = _slug(job.label) if job.label else "all"
    return Path(cache_dir) / f"{repo}__{label}__{job.state}__{job.sort}__page-{page:04d}.json"


def page_url(job, page):
    query = {"state": job.state, "sort": job.sort, "direction": "desc", "per_page": str(PER_PAGE), "page": str(page)}
    if job.label:
        query["labels"] = job.label
    return f"{API}/repos/{job.repo}/issues?{urllib.parse.urlencode(query, quote_via=urllib.parse.quote)}"


def atomic_write(path, text):
    """Write through a temporary file in the same directory, so a page is either whole or absent."""
    handle, temporary = tempfile.mkstemp(dir=path.parent, suffix=".tmp", prefix=".partial-")
    try:
        with os.fdopen(handle, "w", encoding="utf-8") as stream:
            stream.write(text)
        os.replace(temporary, path)
    except BaseException:
        os.unlink(temporary)
        raise


class Ledger:
    """Timestamps of page requests, kept on disk so the hourly budget survives a restart."""

    WINDOW = 3600

    def __init__(self, path, budget, clock):
        self.path = Path(path)
        self.budget = budget
        self.clock = clock
        self.stamps = []
        if self.path.exists():
            self.stamps = [float(line) for line in self.path.read_text().split() if line.strip()]

    def _prune(self):
        horizon = self.clock() - self.WINDOW
        self.stamps = [stamp for stamp in self.stamps if stamp > horizon]

    def exhausted(self):
        self._prune()
        return len(self.stamps) >= self.budget

    def resume_at(self):
        self._prune()
        return int(self.stamps[0] + self.WINDOW)

    def record(self):
        self._prune()
        self.stamps.append(self.clock())
        self.path.write_text("".join(f"{stamp}\n" for stamp in self.stamps))


def _header(headers, name):
    for key, value in headers.items():
        if key.lower() == name.lower():
            return value
    return None


def _rate_state(headers):
    remaining = _header(headers, "X-RateLimit-Remaining")
    reset = _header(headers, "X-RateLimit-Reset")
    if remaining is None or reset is None:
        return None
    return int(remaining), int(reset)


def preflight(opener):
    """Ask GitHub where the shared bucket stands. This call does not count against the limit."""
    response = opener(f"{API}/rate_limit", request_headers())
    if response.status != 200:
        return None
    core = json.loads(response.body)["resources"]["core"]
    return int(core["remaining"]), int(core["reset"])


def trim_issue(raw, repo):
    """Keep the survey's fields and nothing else. Author, assignee and the like never reach the disk."""
    item = {name: raw.get(name) for name in KEPT_ISSUE_FIELDS}
    item["repo"] = repo
    item["labels"] = [label["name"] for label in raw.get("labels") or []]
    item["created"] = (raw.get("created_at") or "")[:10]
    return item


def has_next_page(headers):
    return 'rel="next"' in (_header(headers, "Link") or "")


def _walk_job(job, cache_dir, opener, clock, floor, ledger, outcome, state):
    """One job's pages. Returns the updated rate state, or None when the run has to stop."""
    for page in range(1, job.max_pages + 1):
        path = page_path(cache_dir, job, page)
        if path.exists():
            cached = json.loads(path.read_text())
            outcome.pages_reused += 1
            outcome.issues_seen += len(cached["items"])
            if cached["has_next"]:
                continue
            return state
        if state[0] <= floor:
            outcome.stopped = "rate-floor"
            outcome.resume_at = state[1]
            return None
        if ledger.exhausted():
            outcome.stopped = "budget"
            outcome.resume_at = ledger.resume_at()
            return None
        ledger.record()
        response = opener(page_url(job, page), request_headers())
        outcome.requests += 1
        now = _rate_state(response.headers)
        if response.status in (403, 429):
            outcome.stopped = "rate-limited"
            outcome.resume_at = now[1] if now else None
            return None
        if response.status != 200:
            outcome.stopped = "error"
            outcome.detail = f"HTTP {response.status} for {job.repo} page {page}"
            return None
        if now is None:
            outcome.stopped = "no-rate-headers"
            outcome.detail = f"no rate-limit headers for {job.repo} page {page}"
            return None
        state = now
        raw_items = json.loads(response.body)
        items = [trim_issue(raw, job.repo) for raw in raw_items if "pull_request" not in raw]
        has_next = has_next_page(response.headers) and len(raw_items) > 0
        record = {
            "repo": job.repo,
            "label": job.label,
            "page": page,
            "has_next": has_next,
            "fetched": int(clock()),
            "pull_requests_dropped": len(raw_items) - len(items),
            "items": items,
        }
        atomic_write(path, json.dumps(record))
        outcome.pages_cached += 1
        outcome.issues_seen += len(items)
        if not has_next:
            return state
    return state


def run_jobs(jobs, cache_dir, opener, clock=time.time, floor=RATE_FLOOR, budget=HOURLY_BUDGET):
    """Walk every job's pages until done or until a limit says stop. Safe to run again: it resumes."""
    cache_dir = Path(cache_dir)
    cache_dir.mkdir(parents=True, exist_ok=True)
    outcome = Outcome()
    ledger = Ledger(cache_dir / "ledger.txt", budget, clock)
    state = preflight(opener)
    if state is None:
        outcome.stopped = "error"
        outcome.detail = "the rate check failed"
        return outcome
    for job in jobs:
        state = _walk_job(job, cache_dir, opener, clock, floor, ledger, outcome, state)
        if state is None:
            return outcome
    return outcome


def load_cached_items(cache_dir):
    """Every cached issue, for the stripper. Reading a page yields raw text, so only the stripper calls this."""
    for path in sorted(Path(cache_dir).glob("*__page-*.json")):
        yield from json.loads(path.read_text())["items"]


def cache_summary(cache_dir):
    """Per-job counts from the cached pages. Numbers and label names only; never any issue text."""
    jobs = {}
    for path in sorted(Path(cache_dir).glob("*__page-*.json")):
        record = json.loads(path.read_text())
        key = (record["repo"], record["label"])
        row = jobs.setdefault(key, {"repo": record["repo"], "label": record["label"], "pages": 0, "issues": 0, "pull_requests_dropped": 0, "more_available": False})
        row["pages"] += 1
        row["issues"] += len(record["items"])
        row["pull_requests_dropped"] += record["pull_requests_dropped"]
        row["more_available"] = record["has_next"]
    return [jobs[key] for key in sorted(jobs, key=lambda key: (key[0], key[1] or ""))]


def fetch_labels(repo, cache_dir, opener, clock=time.time, floor=RATE_FLOOR, budget=HOURLY_BUDGET):
    """A repository's label names, cached. Labels are names, not issue text, so they may be printed."""
    cache_dir = Path(cache_dir)
    cache_dir.mkdir(parents=True, exist_ok=True)
    path = cache_dir / f"labels__{repo.replace('/', '__')}.json"
    if path.exists():
        return json.loads(path.read_text()), Outcome()
    ledger = Ledger(cache_dir / "ledger.txt", budget, clock)
    outcome = Outcome()
    state = preflight(opener)
    if state is None:
        outcome.stopped = "error"
        return [], outcome
    names = []
    for page in range(1, 6):
        if state[0] <= floor:
            outcome.stopped = "rate-floor"
            outcome.resume_at = state[1]
            return names, outcome
        if ledger.exhausted():
            outcome.stopped = "budget"
            outcome.resume_at = ledger.resume_at()
            return names, outcome
        ledger.record()
        url = f"{API}/repos/{repo}/labels?per_page={PER_PAGE}&page={page}"
        response = opener(url, request_headers())
        outcome.requests += 1
        if response.status != 200:
            outcome.stopped = "error"
            outcome.detail = f"HTTP {response.status}"
            return names, outcome
        state = _rate_state(response.headers) or state
        names.extend(label["name"] for label in json.loads(response.body))
        if not has_next_page(response.headers):
            break
    atomic_write(path, json.dumps(names))
    return names, outcome


def probe_url(url):
    """Whether a page answers anonymously, reporting status and URL only, never the page's text."""
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT}, method="GET")
    try:
        with urllib.request.urlopen(request, timeout=TIMEOUT_SECONDS) as reply:
            return {"url": url, "status": reply.status, "final_url": reply.geturl(), "content_type": reply.headers.get("Content-Type", "")}
    except urllib.error.HTTPError as error:
        return {"url": url, "status": error.code}
    except OSError as error:
        return {"url": url, "error": type(error).__name__, "reason": str(getattr(error, "reason", error))}


# The pages this survey asks for, in priority order, newest first. Chosen from the labels endpoint for what a kernel
# survey needs: reports its maintainers have tied to the modelling layer, and OCCT's own bug label. A comma means both
# labels at once. The evidence file states the bias this choice introduces.
OCCT = "Open-Cascade-SAS/OCCT"
FREECAD = "FreeCAD/FreeCAD"
JOBS = [
    Job(OCCT, "2. Bug", max_pages=5),
    Job(FREECAD, "Mod: Part,Type: Bug", max_pages=4),
    Job(FREECAD, "Mod: Part Design,Type: Bug", max_pages=4),
    Job(FREECAD, "3rd party: OCC", max_pages=3),
    Job(FREECAD, "Topic: Toponaming", max_pages=2),
    Job(FREECAD, "Topic: Fillets", max_pages=2),
    Job(FREECAD, "File format: STEP", max_pages=2),
    Job(FREECAD, "Topic: Performance", max_pages=2),
]


def exit_status(outcome):
    if outcome.stopped is None:
        return 0
    if outcome.stopped in ("rate-floor", "budget", "rate-limited"):
        return EXIT_WAIT
    return 1


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--cache", type=Path, default=DEFAULT_CACHE)
    commands = parser.add_subparsers(dest="command", required=True)
    labels = commands.add_parser("labels", help="print a repository's label names")
    labels.add_argument("repo")
    commands.add_parser("run", help="fetch the configured jobs, resuming from the cache")
    commands.add_parser("status", help="print the cache's page and request counts")
    probe = commands.add_parser("probe", help="report whether URLs answer anonymously, printing no page text")
    probe.add_argument("urls", nargs="+")
    options = parser.parse_args(argv)
    if options.command == "labels":
        names, outcome = fetch_labels(options.repo, options.cache, urllib_opener)
        print(json.dumps({"labels": names, "outcome": asdict(outcome)}, indent=1))
        return exit_status(outcome)
    if options.command == "run":
        outcome = run_jobs(JOBS, options.cache, urllib_opener)
        print(json.dumps(asdict(outcome), indent=1))
        return exit_status(outcome)
    if options.command == "status":
        pages = sorted(options.cache.glob("*__page-*.json"))
        ledger = options.cache / "ledger.txt"
        stamps = ledger.read_text().split() if ledger.exists() else []
        print(json.dumps({"pages_cached": len(pages), "requests_in_ledger": len(stamps), "jobs": cache_summary(options.cache)}, indent=1))
        return 0
    if options.command == "probe":
        for url in options.urls:
            print(json.dumps(probe_url(url)))
        return 0
    return 1


if __name__ == "__main__":
    sys.exit(main())
