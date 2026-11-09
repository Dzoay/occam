"""The fetch step: everything is checked against the lock, and no test touches the network."""

import hashlib
import io
import json
import pathlib
import tempfile
import unittest
import unittest.mock
import zipfile

import fetch


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def zip_of(members):
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as archive:
        for name, data in members.items():
            archive.writestr(name, data)
    return buffer.getvalue()


class Response(io.BytesIO):
    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()


def opener_for(payload):
    """A fake urlopen returning `payload`; it records every request."""
    requests = []

    def opener(request, timeout=None):
        requests.append(request)
        return Response(payload)

    opener.requests = requests
    return opener


def entry_for(payload, members=None):
    entry = {
        "name": "fake-source",
        "url": "https://example.invalid/fake.zip",
        "dest": "fake",
        "filename": "fake.zip",
        "sha256": sha256(payload),
        "size": len(payload),
    }
    if members is not None:
        entry["members"] = members
    return entry


class FetchTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.cache = pathlib.Path(self.tmp.name)
        patcher = unittest.mock.patch("urllib.request.urlopen", side_effect=AssertionError("network used in a test"))
        patcher.start()
        self.addCleanup(patcher.stop)

    def test_a_good_download_lands_in_its_own_directory(self):
        payload = b"locked bytes"
        opener = opener_for(payload)
        path = fetch.fetch_source(entry_for(payload), self.cache, opener=opener)
        self.assertEqual(path, self.cache / "fake" / "fake.zip")
        self.assertEqual(path.read_bytes(), payload)
        self.assertEqual(len(opener.requests), 1)

    def test_the_user_agent_is_generic(self):
        payload = b"locked bytes"
        opener = opener_for(payload)
        fetch.fetch_source(entry_for(payload), self.cache, opener=opener)
        self.assertEqual(opener.requests[0].get_header("User-agent"), "occam-census")

    def test_a_file_with_the_wrong_hash_is_rejected_and_not_kept(self):
        entry = entry_for(b"locked bytes")
        opener = opener_for(b"tampered bytes")
        with self.assertRaises(fetch.HashMismatch) as caught:
            fetch.fetch_source(entry, self.cache, opener=opener)
        self.assertIn(entry["sha256"], str(caught.exception))
        self.assertEqual(list((self.cache / "fake").iterdir()), [])

    def test_a_wrong_size_is_rejected(self):
        entry = entry_for(b"locked bytes")
        entry["size"] = 999
        with self.assertRaises(fetch.FetchError):
            fetch.fetch_source(entry, self.cache, opener=opener_for(b"locked bytes"))

    def test_an_already_correct_file_is_not_downloaded_again(self):
        payload = b"locked bytes"
        (self.cache / "fake").mkdir()
        (self.cache / "fake" / "fake.zip").write_bytes(payload)
        opener = opener_for(payload)
        fetch.fetch_source(entry_for(payload), self.cache, opener=opener)
        self.assertEqual(opener.requests, [])

    def test_a_corrupt_cached_file_is_replaced(self):
        payload = b"locked bytes"
        (self.cache / "fake").mkdir()
        (self.cache / "fake" / "fake.zip").write_bytes(b"something else")
        fetch.fetch_source(entry_for(payload), self.cache, opener=opener_for(payload))
        self.assertEqual((self.cache / "fake" / "fake.zip").read_bytes(), payload)

    def test_archive_members_are_checked_against_the_lock(self):
        payload = zip_of({"part.stp": b"geometry"})
        members = [{"name": "part.stp", "sha256": sha256(b"geometry"), "size": 8}]
        path = fetch.fetch_source(entry_for(payload, members), self.cache, opener=opener_for(payload))
        self.assertEqual(path.name, "fake.zip")

    def test_a_tampered_archive_member_is_rejected(self):
        payload = zip_of({"part.stp": b"geometry"})
        members = [{"name": "part.stp", "sha256": sha256(b"other"), "size": 8}]
        with self.assertRaises(fetch.HashMismatch):
            fetch.fetch_source(entry_for(payload, members), self.cache, opener=opener_for(payload))
        self.assertEqual(list((self.cache / "fake").iterdir()), [])

    def test_a_member_missing_from_the_archive_is_rejected(self):
        payload = zip_of({"part.stp": b"geometry"})
        members = [{"name": "gone.stp", "sha256": sha256(b"geometry"), "size": 8}]
        with self.assertRaises(fetch.FetchError):
            fetch.fetch_source(entry_for(payload, members), self.cache, opener=opener_for(payload))


class LockTests(unittest.TestCase):
    def test_the_committed_lock_is_well_formed(self):
        document = json.loads(fetch.LOCK.read_text())
        self.assertEqual(document["schema"], "occam-corpus-sources/1")
        self.assertTrue(document["sources"])
        for entry in document["sources"]:
            with self.subTest(source=entry["name"]):
                self.assertTrue(entry["url"].startswith("https://www.nist.gov/"))
                self.assertRegex(entry["sha256"], r"^[0-9a-f]{64}$")
                self.assertGreater(entry["size"], 0)
                self.assertIn("statement", entry["terms"])
                self.assertIn("page", entry["terms"])
                for member in entry["members"]:
                    self.assertRegex(member["sha256"], r"^[0-9a-f]{64}$")


if __name__ == "__main__":
    unittest.main()
