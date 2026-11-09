"""The fetcher against a fake GitHub: anonymous, polite, resumable. Synthetic data only."""

import json
import sys
import tempfile
import unittest
import urllib.parse
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import defects_fetch as fetch  # noqa: E402


def make_issue(number, **extra):
    issue = {
        "number": number,
        "title": f"Made-up issue {number}",
        "body": "Made-up body.",
        "state": "open",
        "created_at": "2025-01-02T03:04:05Z",
        "labels": [{"name": "kind: bug", "color": "ff0000"}],
        "user": {"login": "made-up-author", "id": 42},
        "assignee": {"login": "made-up-assignee"},
    }
    issue.update(extra)
    return issue


class FakeGitHub:
    """Serves numbered pages of made-up issues and records every request."""

    def __init__(self, pages, remaining=50, limit=60, reset=1_000_000, fail_page=None, fail_status=500, omit_headers=False):
        self.pages = pages
        self.remaining = remaining
        self.limit = limit
        self.reset = reset
        self.fail_page = fail_page
        self.fail_status = fail_status
        self.omit_headers = omit_headers
        self.calls = []

    def page_calls(self):
        return [url for url, _ in self.calls if "/rate_limit" not in url]

    def _rate_headers(self):
        if self.omit_headers:
            return {}
        return {
            "X-RateLimit-Limit": str(self.limit),
            "X-RateLimit-Remaining": str(self.remaining),
            "X-RateLimit-Reset": str(self.reset),
        }

    def __call__(self, url, headers):
        self.calls.append((url, dict(headers)))
        parsed = urllib.parse.urlparse(url)
        if parsed.path == "/rate_limit":
            body = {"resources": {"core": {"limit": self.limit, "remaining": self.remaining, "reset": self.reset}}}
            return fetch.Response(200, {}, json.dumps(body).encode())
        query = urllib.parse.parse_qs(parsed.query)
        page = int(query.get("page", ["1"])[0])
        self.remaining = max(0, self.remaining - 1)
        if self.fail_page == page:
            return fetch.Response(self.fail_status, self._rate_headers(), b'{"message": "made-up failure"}')
        items = self.pages[page - 1] if page <= len(self.pages) else []
        headers = self._rate_headers()
        if page < len(self.pages):
            headers["Link"] = f'<{url}&page={page + 1}>; rel="next"'
        return fetch.Response(200, headers, json.dumps(items).encode())


class Clock:
    def __init__(self, now=1_000.0):
        self.now = now

    def __call__(self):
        return self.now


JOB = fetch.Job(repo="acme/widgets", label="kind: bug", max_pages=5)
THREE_PAGES = [[make_issue(1), make_issue(2)], [make_issue(3)], [make_issue(4)]]


class Base(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.cache = Path(self._tmp.name)
        self.clock = Clock()

    def run_jobs(self, fake, jobs=(JOB,), **kwargs):
        return fetch.run_jobs(list(jobs), self.cache, fake, clock=self.clock, **kwargs)

    def cached_text(self):
        return "".join(p.read_text() for p in sorted(self.cache.glob("*.json")))


class Anonymity(Base):
    def test_every_request_carries_only_the_generic_client_name(self):
        fake = FakeGitHub(THREE_PAGES)
        self.run_jobs(fake)
        self.assertTrue(fake.calls)
        for url, headers in fake.calls:
            self.assertEqual(headers.get("User-Agent"), "occam-census")
            lowered = {name.lower(): value for name, value in headers.items()}
            self.assertNotIn("authorization", lowered)
            self.assertNotIn("from", lowered)
            self.assertNotIn("@", " ".join(lowered.values()))
            self.assertNotIn("@", url)

    def test_page_urls_ask_for_one_hundred_issues_and_all_states(self):
        fake = FakeGitHub(THREE_PAGES)
        self.run_jobs(fake)
        first = urllib.parse.parse_qs(urllib.parse.urlparse(fake.page_calls()[0]).query)
        self.assertEqual(first["per_page"], ["100"])
        self.assertEqual(first["state"], ["all"])
        self.assertEqual(first["labels"], ["kind: bug"])
        self.assertEqual(first["page"], ["1"])


class RateLimit(Base):
    def test_stops_when_the_header_says_twenty_remain(self):
        fake = FakeGitHub(THREE_PAGES, remaining=22)
        outcome = self.run_jobs(fake)
        self.assertEqual(len(fake.page_calls()), 2)
        self.assertEqual(outcome.stopped, "rate-floor")
        self.assertEqual(outcome.resume_at, fake.reset)

    def test_requests_nothing_when_the_preflight_is_already_at_the_floor(self):
        fake = FakeGitHub(THREE_PAGES, remaining=20)
        outcome = self.run_jobs(fake)
        self.assertEqual(fake.page_calls(), [])
        self.assertEqual(outcome.stopped, "rate-floor")
        self.assertEqual(outcome.resume_at, fake.reset)
        self.assertEqual(outcome.requests, 0)

    def test_the_rate_check_does_not_count_against_the_budget(self):
        fake = FakeGitHub(THREE_PAGES)
        outcome = self.run_jobs(fake)
        self.assertEqual(outcome.requests, len(fake.page_calls()))
        self.assertGreater(len(fake.calls), len(fake.page_calls()))

    def test_a_response_without_rate_headers_stops_the_run(self):
        fake = FakeGitHub(THREE_PAGES, omit_headers=True)
        outcome = self.run_jobs(fake)
        self.assertEqual(len(fake.page_calls()), 1)
        self.assertEqual(outcome.stopped, "no-rate-headers")

    def test_a_403_is_a_rate_limit_stop_and_is_not_cached(self):
        fake = FakeGitHub(THREE_PAGES, fail_page=2, fail_status=403)
        outcome = self.run_jobs(fake)
        self.assertEqual(outcome.stopped, "rate-limited")
        self.assertEqual(outcome.resume_at, fake.reset)
        self.assertNotIn("made-up failure", self.cached_text())
        self.assertEqual(outcome.pages_cached, 1)

    def test_a_429_is_a_rate_limit_stop_too(self):
        fake = FakeGitHub(THREE_PAGES, fail_page=1, fail_status=429)
        outcome = self.run_jobs(fake)
        self.assertEqual(outcome.stopped, "rate-limited")


class Budget(Base):
    def test_stops_at_the_hourly_budget(self):
        fake = FakeGitHub([[make_issue(n)] for n in range(1, 10)])
        job = fetch.Job(repo="acme/widgets", label=None, max_pages=9)
        outcome = self.run_jobs(fake, jobs=[job], budget=2)
        self.assertEqual(outcome.requests, 2)
        self.assertEqual(len(fake.page_calls()), 2)
        self.assertEqual(outcome.stopped, "budget")
        self.assertEqual(outcome.resume_at, int(self.clock.now + 3600))

    def test_the_budget_survives_a_restart_within_the_hour(self):
        pages = [[make_issue(n)] for n in range(1, 10)]
        job = fetch.Job(repo="acme/widgets", label=None, max_pages=9)
        self.run_jobs(FakeGitHub(pages), jobs=[job], budget=2)
        self.clock.now += 60
        second = FakeGitHub(pages)
        outcome = self.run_jobs(second, jobs=[job], budget=2)
        self.assertEqual(second.page_calls(), [])
        self.assertEqual(outcome.stopped, "budget")

    def test_the_budget_refills_after_an_hour(self):
        pages = [[make_issue(n)] for n in range(1, 10)]
        job = fetch.Job(repo="acme/widgets", label=None, max_pages=9)
        self.run_jobs(FakeGitHub(pages), jobs=[job], budget=2)
        self.clock.now += 3601
        second = FakeGitHub(pages)
        outcome = self.run_jobs(second, jobs=[job], budget=2)
        self.assertEqual(len(second.page_calls()), 2)
        self.assertEqual(outcome.stopped, "budget")


class CacheAndResume(Base):
    def test_a_second_run_requests_nothing_for_cached_pages(self):
        self.run_jobs(FakeGitHub(THREE_PAGES))
        fake = FakeGitHub(THREE_PAGES)
        outcome = self.run_jobs(fake)
        self.assertEqual(fake.page_calls(), [])
        self.assertEqual(outcome.pages_reused, 3)
        self.assertIsNone(outcome.stopped)

    def test_resume_fetches_only_the_pages_still_missing(self):
        self.run_jobs(FakeGitHub(THREE_PAGES, remaining=22))
        fake = FakeGitHub(THREE_PAGES, remaining=50)
        outcome = self.run_jobs(fake)
        self.assertEqual(len(fake.page_calls()), 1)
        self.assertEqual(outcome.pages_cached, 1)
        self.assertEqual(outcome.pages_reused, 2)

    def test_stops_paging_when_there_is_no_next_link(self):
        fake = FakeGitHub([[make_issue(1)]])
        self.run_jobs(fake)
        self.assertEqual(len(fake.page_calls()), 1)

    def test_a_completed_job_does_not_ask_for_a_next_page_on_resume(self):
        self.run_jobs(FakeGitHub([[make_issue(1)]]))
        fake = FakeGitHub([[make_issue(1)], [make_issue(2)]])
        self.run_jobs(fake)
        self.assertEqual(fake.page_calls(), [])

    def test_max_pages_limits_the_walk(self):
        job = fetch.Job(repo="acme/widgets", label=None, max_pages=2)
        fake = FakeGitHub(THREE_PAGES)
        self.run_jobs(fake, jobs=[job])
        self.assertEqual(len(fake.page_calls()), 2)

    def test_a_failed_page_keeps_earlier_pages_and_leaves_no_partial_file(self):
        fake = FakeGitHub(THREE_PAGES, fail_page=2)
        outcome = self.run_jobs(fake)
        self.assertEqual(outcome.stopped, "error")
        self.assertEqual(outcome.pages_cached, 1)
        self.assertEqual(len(list(self.cache.glob("*page-0001*.json"))), 1)
        self.assertEqual(list(self.cache.glob("*page-0002*")), [])
        self.assertEqual(list(self.cache.glob("*.tmp")), [])

    def test_distinct_jobs_on_one_repo_have_distinct_cache_files(self):
        first = fetch.Job(repo="acme/widgets", label="kind: bug", max_pages=1)
        second = fetch.Job(repo="acme/widgets", label="area: part", max_pages=1)
        self.run_jobs(FakeGitHub([[make_issue(1)]]), jobs=[first, second])
        self.assertEqual(len([p for p in self.cache.glob("*page-*.json")]), 2)


class Summary(Base):
    def test_the_summary_prints_counts_and_no_issue_text(self):
        pull = make_issue(9, pull_request={"url": "https://example.invalid/pull/9"})
        job = fetch.Job(repo="acme/widgets", label="kind: bug", max_pages=2)
        self.run_jobs(FakeGitHub([[make_issue(1), pull], [make_issue(2)]]), jobs=[job])
        rows = fetch.cache_summary(self.cache)
        self.assertEqual(
            rows,
            [{"repo": "acme/widgets", "label": "kind: bug", "pages": 2, "issues": 2, "pull_requests_dropped": 1, "more_available": False}],
        )
        self.assertNotIn("Made-up", json.dumps(rows))


class WhatIsStored(Base):
    def test_only_the_fields_the_survey_needs_are_written(self):
        self.run_jobs(FakeGitHub([[make_issue(7)]]))
        items = list(fetch.load_cached_items(self.cache))
        self.assertEqual(len(items), 1)
        self.assertEqual(set(items[0]), {"repo", "number", "title", "body", "state", "labels", "created"})
        self.assertEqual(items[0]["repo"], "acme/widgets")
        self.assertEqual(items[0]["labels"], ["kind: bug"])
        self.assertEqual(items[0]["created"], "2025-01-02")

    def test_author_fields_are_never_written_to_disk(self):
        self.run_jobs(FakeGitHub([[make_issue(7)]]))
        self.assertNotIn("made-up-author", self.cached_text())
        self.assertNotIn("made-up-assignee", self.cached_text())

    def test_pull_requests_are_dropped_from_the_stored_page(self):
        pull = make_issue(9, pull_request={"url": "https://example.invalid/pull/9"})
        self.run_jobs(FakeGitHub([[make_issue(8), pull]]))
        numbers = [item["number"] for item in fetch.load_cached_items(self.cache)]
        self.assertEqual(numbers, [8])
        self.assertNotIn("/pull/9", self.cached_text())


if __name__ == "__main__":
    unittest.main()
