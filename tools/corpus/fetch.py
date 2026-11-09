#!/usr/bin/env python3
"""Downloads the locked corpus sources into the user's cache, verifying every byte against sources.lock.json.

Nothing fetched here is committed; `tools/corpus/sources.lock.json` is the only trace in the repository. Sends no
identifying header: the User-Agent is the generic `occam-census`. `just corpus-fetch` runs this.
"""

import argparse
import hashlib
import json
import os
import pathlib
import sys
import urllib.request
import zipfile

LOCK = pathlib.Path(__file__).resolve().parent / "sources.lock.json"
USER_AGENT = "occam-census"
TIMEOUT_SECONDS = 300
CHUNK = 1024 * 1024


class FetchError(Exception):
    """A source could not be fetched or does not match its lock entry."""


class HashMismatch(FetchError):
    """The bytes that arrived are not the bytes the lock names."""


def default_cache():
    return pathlib.Path(os.environ.get("XDG_CACHE_HOME", pathlib.Path.home() / ".cache")) / "occam-corpus"


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as stream:
        for chunk in iter(lambda: stream.read(CHUNK), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _verify_members(path, members):
    """Checks every locked member of an archive, by reading it in memory. Nothing is extracted to disk."""
    with zipfile.ZipFile(path) as archive:
        present = set(archive.namelist())
        for member in members:
            name = member["name"]
            if name not in present:
                raise FetchError(f"{path}: locked member {name!r} is missing")
            data = archive.read(name)
            actual = hashlib.sha256(data).hexdigest()
            if actual != member["sha256"] or len(data) != member["size"]:
                raise HashMismatch(f"{path}: member {name!r} is {actual}, locked as {member['sha256']}")


def _verify(path, entry):
    actual = sha256_file(path)
    if actual != entry["sha256"]:
        raise HashMismatch(f"{path}: sha256 {actual}, locked as {entry['sha256']}")
    size = path.stat().st_size
    if size != entry["size"]:
        raise FetchError(f"{path}: {size} bytes, locked as {entry['size']}")
    if entry.get("members"):
        _verify_members(path, entry["members"])


def _download(url, temporary, opener):
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with opener(request, timeout=TIMEOUT_SECONDS) as response, open(temporary, "wb") as out:
        for chunk in iter(lambda: response.read(CHUNK), b""):
            out.write(chunk)


def fetch_source(entry, cache, opener=None):
    """Makes `cache/<dest>/<filename>` hold exactly the locked bytes, and returns its path.

    A partial or wrong download is removed before the error is raised, so nothing that fails verification stays behind.
    """
    opener = opener or urllib.request.urlopen
    directory = pathlib.Path(cache) / entry["dest"]
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / entry["filename"]
    if path.exists():
        try:
            _verify(path, entry)
            return path
        except FetchError:
            path.unlink()
    temporary = directory / f".{entry['filename']}.partial"
    try:
        _download(entry["url"], temporary, opener)
        _verify(temporary, entry)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise
    temporary.replace(path)
    return path


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--cache", type=pathlib.Path, default=default_cache(), help="cache root (default: ~/.cache/occam-corpus)")
    args = parser.parse_args(argv)
    document = json.loads(LOCK.read_text(encoding="utf-8"))
    failed = 0
    for entry in document["sources"]:
        try:
            path = fetch_source(entry, args.cache)
        except (FetchError, OSError) as error:
            print(f"FAILED {entry['name']}: {error}", file=sys.stderr)
            failed += 1
            continue
        print(f"ok {entry['name']}: {path} ({entry['size']} bytes, {len(entry.get('members', []))} members verified)")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
