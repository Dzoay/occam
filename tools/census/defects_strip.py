#!/usr/bin/env python3
"""Strip code, patches and people from cached issue reports, for `census.defects`.

The provenance decision allows a script that fetches public bug reports and strips code blocks and linked patches, so
only the stripped text is read, for symptoms. This is that script. It removes fenced, indented and inline code, stack
traces, diff hunks, links to patches, commits, pull requests and diffs, attachments, images, `@mentions`, email
addresses, author names, user paths, and reduces any library identifier left in prose to plain words.

Only a person's or agent's reading of this script's output is permitted: the raw cache is never opened by hand.

Python standard library only.
"""

import argparse
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import defects_fetch  # noqa: E402

DEFAULT_OUT = Path.home() / ".cache" / "occam-census" / "issues-stripped" / "issues.jsonl"

# --- blocks -----------------------------------------------------------------------------------------------------------

FENCE_OPEN = re.compile(r"^ {0,3}(`{3,}|~{3,})(.*)$")
INDENTED = re.compile(r"^(?: {4}|\t)")
BLOCKQUOTE = re.compile(r"^\s{0,3}>")
ATTRIBUTION = re.compile(r"\bwrote:\s*$")
DIFF_HEADER = re.compile(r"^(?:diff --git |index [0-9a-f]+\.\.[0-9a-f]+|--- |\+\+\+ )")
HUNK_HEADER = re.compile(r"^@@ .*@@")
HUNK_LINE = re.compile(r"^[ +\-\\]")
CHANGED_LINE = re.compile(r"^[+-][^\s+\-]")
TRACEBACK = re.compile(r"^\s*Traceback \(most recent call last\)")
TRAILER = re.compile(
    r"^\s*(?:signed-off-by|co-authored-by|reviewed-by|acked-by|tested-by|reported-by|reporter|assignee|assigned to"
    r"|developer|tester|author|from|cc)\s*:.*$",
    re.IGNORECASE,
)
SIGN_OFF = re.compile(
    r"^\s*(?:best regards|kind regards|warm regards|regards|sincerely|cheers|thanks|thank you|many thanks|best)[,.!]*\s*$",
    re.IGNORECASE,
)

TRACE_LINES = [
    re.compile(pattern)
    for pattern in (
        r'^\s*File ".*", line \d+',
        r"^\s*#\d+\s+(?:0x[0-9a-fA-F]+|.*\bin\b.*\(|\S+\()",
        r"\b0x[0-9a-fA-F]{6,}\b",
        r"^\s*at\s+[\w.$<>:/\\-]+\s*\(.*\)\s*$",
        r"^\s*(?:in\s+)?[\w~<>$]+(?:::[\w~<>$]+)+\s*\(",
        r"\.(?:cpp|cxx|cc|c|hxx|hpp|h|py|java):\d+\b",
        r"^\s*\S*\.(?:so|dll|dylib|pyd)(?:\.\d+)*(?:[(!:+\[]|\s*$)",
    )
]

CODE_LINES = [
    re.compile(pattern)
    for pattern in (
        r"^\s*#\s*include\b",
        r"^\s*#!",
        r"^\s*import\s+[\w.]+(?:\s+as\s+\w+)?\s*$",
        r"^\s*from\s+[\w.]+\s+import\b",
        r"^\s*(?:def|class)\s+\w+.*:\s*$",
        r"^\s*(?:public|private|protected|static|virtual|const|void|int|float|double|bool|auto|char|struct|enum|template|namespace|using)\b.*[;{(]\s*$",
        r"^\s*(?:for|if|while|switch|return)\b.*[;{)]\s*$",
        r"^\s*[\w.\[\]]+\s*=\s*.+\)\s*$",
        r"^\s*[\w]+(?:\.[\w]+)+\(.*\)\s*;?\s*$",
        r"^.*;\s*$",
        r"^\s*[{}]\s*;?\s*$",
        r"^\s*}\s*else\b",
    )
]


def _closes(line, fence):
    match = re.match(r"^ {0,3}(`{3,}|~{3,})\s*$", line)
    return bool(match) and match.group(1)[0] == fence[0] and len(match.group(1)) >= len(fence)


def _is_changed_run(lines, index):
    if not CHANGED_LINE.match(lines[index]):
        return False
    before = index > 0 and CHANGED_LINE.match(lines[index - 1])
    after = index + 1 < len(lines) and CHANGED_LINE.match(lines[index + 1])
    return bool(before or after)


def _is_dropped_line(line):
    return (
        BLOCKQUOTE.match(line)
        or ATTRIBUTION.search(line)
        or TRAILER.match(line)
        or any(pattern.search(line) for pattern in TRACE_LINES)
        or any(pattern.search(line) for pattern in CODE_LINES)
    )


def _strip_lines(text):
    """Remove the block-level things: fences, indented code, quotes, diffs, stack traces, unfenced code."""
    lines = text.split("\n")
    kept = []
    fence = None
    indented = False
    in_hunk = False
    in_traceback = False
    previous_blank = True
    for index, line in enumerate(lines):
        if fence is not None:
            if _closes(line, fence):
                fence = None
                previous_blank = True
            continue
        opened = FENCE_OPEN.match(line)
        if opened:
            fence = opened.group(1)
            continue
        blank = not line.strip()
        if in_traceback:
            if blank:
                in_traceback = False
            elif not re.match(r"^\s", line):
                in_traceback = False
                previous_blank = True
                continue
            else:
                continue
        if indented:
            if blank:
                previous_blank = True
                continue
            if INDENTED.match(line):
                continue
            indented = False
        if blank:
            in_hunk = False
            kept.append("")
            previous_blank = True
            continue
        if in_hunk and HUNK_LINE.match(line):
            continue
        in_hunk = False
        if previous_blank and INDENTED.match(line):
            indented = True
            continue
        if TRACEBACK.search(line):
            in_traceback = True
            continue
        if HUNK_HEADER.match(line):
            in_hunk = True
            continue
        if DIFF_HEADER.match(line) or _is_changed_run(lines, index) or _is_dropped_line(line):
            previous_blank = False
            continue
        kept.append(line)
        previous_blank = False
    return kept


def _cut_sign_off(lines):
    """Drop a closing such as 'Best regards,' with the short name below it."""
    for index, line in enumerate(lines):
        if not SIGN_OFF.match(line):
            continue
        rest = [other for other in lines[index + 1 :] if other.strip()]
        if len(rest) <= 2:
            return lines[:index]
        return lines[:index] + lines[index + 1 :]
    return lines


# --- inline -----------------------------------------------------------------------------------------------------------

IMAGE_MD = re.compile(r"!\[[^\]]*\](?:\([^)]*\)|\[[^\]]*\])")
IMAGE_HTML = re.compile(r"<img\b[^>]*>", re.IGNORECASE)
CODE_SPAN = re.compile(r"(?<!`)(`+)(?!`)(.+?)(?<!`)\1(?!`)", re.DOTALL)
LINK_MD = re.compile(r"\[([^\]]*)\]\(\s*([^)\s]+)(?:\s+\"[^\"]*\")?\s*\)")
LINK_REF = re.compile(r"\[([^\]]+)\]\[[^\]]*\]")
LINK_DEF = re.compile(r"(?m)^\s*\[[^\]]+\]:\s*\S+.*$")
AUTOLINK = re.compile(r"<https?://[^>\s]+>", re.IGNORECASE)
BARE_URL = re.compile(r"\bhttps?://\S+|\bwww\.\S+", re.IGNORECASE)
HTML_TAG = re.compile(r"</?[a-zA-Z][^<>]*>")
PATCH_URL = re.compile(
    r"/commits?/|/pull/|/pulls/|/compare/|/blob/|\.patch\b|\.diff\b|/files/|/attachments?/|user-attachments", re.IGNORECASE
)

EMAIL = re.compile(r"[\w.+-]+@[\w-]+(?:\.[\w-]+)+")
MENTION = re.compile(r"(?<![\w@./])@[A-Za-z0-9][A-Za-z0-9_-]*(?:/[A-Za-z0-9._-]+)?")
NAME = r"(?:[A-Z][\w'.-]*\s*){1,4}"
ATTRIBUTED = re.compile(rf"\b(?:originally\s+)?(?:reported|submitted|authored|contributed|posted) by\s+{NAME}", re.IGNORECASE)
THANKED = re.compile(rf"\bthanks to\s+{NAME}", re.IGNORECASE)

SCOPED_NAME = re.compile(r"(?:\b\w+)?(?:::\w+)+(?:\([^)]*\))?")
INCLUDE_TOKEN = re.compile(r"#\s*include\b\S*")
CALL_EXPRESSION = re.compile(r"\b\w+(?:\.\w+)+\([^)]*\)|\b\w+\(\)")
SOURCE_FILE = re.compile(r"\b[\w.-]+\.(?:cpp|cxx|cc|c|hxx|hpp|hh|h|py|java|rs)\b", re.IGNORECASE)
WINDOWS_PATH = re.compile(r"\b[A-Za-z]:\\\S+")
BACKSLASH_PATH = re.compile(r"\b\w+(?:\\[\w.-]+)+\\?")
UNIX_PATH = re.compile(r"(?<![\w/.])(?:~|/)(?:[\w.+-]+/)+[\w.+-]*")
LIBRARY_NAME = re.compile(r"\b[A-Za-z][A-Za-z0-9]*(?:_[A-Za-z0-9]+)+\b")

ISSUE_REF = re.compile(r"\b[\w.-]+/[\w.-]+#\d+\b|(?<![\w#])#\d+\b|\bGH-\d+\b")
COMMIT_ID = re.compile(r"\b(?=[0-9a-f]*[a-f])(?=[0-9a-f]*[0-9])[0-9a-f]{7,40}\b")


def _replace_link(match):
    return "" if PATCH_URL.search(match.group(2)) else match.group(1)


def _reduce_library_name(match):
    """`Acme_MakeFillet` becomes `make fillet`: the operation word survives, the identifier does not."""
    token = match.group(0)
    if token.islower():
        return token
    last = token.split("_")[-1]
    return re.sub(r"(?<=[a-z0-9])(?=[A-Z])", " ", last).lower()


def _strip_inline(text):
    text = IMAGE_MD.sub("", text)
    text = IMAGE_HTML.sub("", text)
    paragraphs = re.split(r"(\n\s*\n)", text)
    text = "".join(CODE_SPAN.sub("", paragraph) for paragraph in paragraphs)
    text = LINK_DEF.sub("", text)
    text = LINK_MD.sub(_replace_link, text)
    text = LINK_REF.sub(r"\1", text)
    text = AUTOLINK.sub("", text)
    text = BARE_URL.sub("", text)
    text = HTML_TAG.sub("", text)
    text = EMAIL.sub("", text)
    text = MENTION.sub("", text)
    text = ATTRIBUTED.sub("", text)
    text = THANKED.sub("", text)
    text = WINDOWS_PATH.sub("", text)
    text = BACKSLASH_PATH.sub("", text)
    text = UNIX_PATH.sub("", text)
    text = SOURCE_FILE.sub("", text)
    text = SCOPED_NAME.sub("", text)
    text = INCLUDE_TOKEN.sub("", text)
    text = CALL_EXPRESSION.sub("", text)
    text = LIBRARY_NAME.sub(_reduce_library_name, text)
    text = ISSUE_REF.sub("", text)
    text = COMMIT_ID.sub("", text)
    return text.replace("`", "")


def _tidy(text):
    lines = []
    for line in text.split("\n"):
        line = re.sub(r"[ \t]+", " ", line).strip()
        line = re.sub(r"\s+([.,;:!?])", r"\1", line)
        line = "" if not re.search(r"[A-Za-z]", line) else line
        lines.append(line)
    return re.sub(r"\n{3,}", "\n\n", "\n".join(lines)).strip()


INTERNALS_PROSE = re.compile(
    r"\breturns?\s+(?:true|false|null|nullptr)\b|\bcode\s*paths?\b|\bsource\s+code\b|\bin\s+the\s+code\b|\bthe\s+code\b"
    r"|\bimplementation\b|\b(?:the|this|that|which)\s+(?:function|method|routine|algorithm|algo)\b|\bbranch\b"
    r"|\b(?:is|are|gets?|being)\s+called\b",
    re.IGNORECASE,
)
SENTENCE_END = re.compile(r"(?<=[.!?])\s+")


def _drop_internals_sentences(text):
    """Drop sentences that describe how the code works. Prose can carry that too, and only symptoms may be read."""
    lines = []
    for line in text.split("\n"):
        kept = [sentence for sentence in SENTENCE_END.split(line) if not INTERNALS_PROSE.search(sentence)]
        lines.append(" ".join(kept))
    return "\n".join(lines)


def _remove_html_blocks(text):
    text = re.sub(r"(?s)<!--.*?(?:-->|$)", "", text)
    return re.sub(r"(?is)<(pre|code|samp|details|script|style)\b[^>]*>.*?(?:</\1\s*>|$)", "", text)


def strip_body(text):
    """The body with code, patches and people removed. Prose is left in place."""
    if not text:
        return ""
    text = text.replace("\r\n", "\n").replace("\r", "\n").lstrip("﻿")
    text = _remove_html_blocks(text)
    lines = _cut_sign_off(_strip_lines(text))
    return _tidy(_drop_internals_sentences(_strip_inline("\n".join(lines))))


def strip_title(text):
    """A title has no blocks, so only the inline pass runs."""
    if not text:
        return ""
    return _tidy(_strip_inline(text.replace("\n", " ")))


def strip_issue(item):
    """One record for the stripped file, holding the fields the classifier reads and no others."""
    return {
        "repo": item["repo"],
        "number": item["number"],
        "labels": item.get("labels") or [],
        "state": item.get("state"),
        "title": strip_title(item.get("title")),
        "body": strip_body(item.get("body")),
    }


def strip_cache(cache_dir, out_path):
    """Write the stripped file, one record per issue, sorted. Returns counts only, so nothing raw can be displayed."""
    out_path = Path(out_path)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    records = {}
    duplicates = 0
    for item in defects_fetch.load_cached_items(cache_dir):
        key = (item["repo"], item["number"])
        if key in records:
            duplicates += 1
            continue
        records[key] = strip_issue(item)
    with out_path.open("w", encoding="utf-8") as stream:
        for key in sorted(records):
            stream.write(json.dumps(records[key], ensure_ascii=False) + "\n")
    by_repo = {}
    for repo, _ in records:
        by_repo[repo] = by_repo.get(repo, 0) + 1
    return {"written": len(records), "duplicates": duplicates, "by_repo": by_repo}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--cache", type=Path, default=defects_fetch.DEFAULT_CACHE)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    options = parser.parse_args(argv)
    print(json.dumps(strip_cache(options.cache, options.out)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
