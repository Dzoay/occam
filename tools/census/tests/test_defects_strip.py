"""The stripper removes code, patches and people, and leaves prose. Synthetic text only."""

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import defects_strip as strip  # noqa: E402

PROSE = "The fuse of two boxes returns an empty shape."
AFTER = "After that the model cannot be saved."


class Case(unittest.TestCase):
    def assertRemoved(self, source, *forbidden):
        out = strip.strip_body(source)
        for needle in forbidden:
            self.assertNotIn(needle, out)
        return out

    def assertKept(self, source, *wanted):
        out = strip.strip_body(source)
        for needle in wanted:
            self.assertIn(needle, out)
        return out


class FencedCode(Case):
    def test_backtick_fence(self):
        source = f"{PROSE}\n\n```python\nsecret_token = 1\n```\n\n{AFTER}"
        self.assertRemoved(source, "secret_token", "```")
        self.assertKept(source, PROSE, AFTER)

    def test_tilde_fence(self):
        source = f"{PROSE}\n\n~~~\nsecret_token = 1\n~~~\n\n{AFTER}"
        self.assertRemoved(source, "secret_token", "~~~")
        self.assertKept(source, PROSE, AFTER)

    def test_longer_fence_holds_a_shorter_one(self):
        source = f"{PROSE}\n\n````\n```\nsecret_token\n```\nmore_secret\n````\n\n{AFTER}"
        self.assertRemoved(source, "secret_token", "more_secret")
        self.assertKept(source, PROSE, AFTER)

    def test_unterminated_fence_removes_to_the_end(self):
        self.assertRemoved(f"{PROSE}\n\n```\nsecret_token = 1\n", "secret_token")
        self.assertKept(f"{PROSE}\n\n```\nsecret_token = 1\n", PROSE)

    def test_html_pre_and_code_blocks(self):
        source = f"{PROSE}\n<pre>secret_token</pre>\n<code>other_secret</code>\n{AFTER}"
        self.assertRemoved(source, "secret_token", "other_secret", "<pre>", "<code>")
        self.assertKept(source, PROSE, AFTER)

    def test_details_block_is_removed(self):
        source = f"{PROSE}\n<details><summary>Log</summary>\nsecret_token\n</details>\n{AFTER}"
        self.assertRemoved(source, "secret_token", "<details>", "Log")
        self.assertKept(source, PROSE, AFTER)

    def test_html_comments_are_removed(self):
        source = f"<!-- template secret_token -->\n{PROSE}"
        self.assertRemoved(source, "secret_token", "<!--")
        self.assertKept(source, PROSE)


class IndentedCode(Case):
    def test_indented_block_after_a_blank_line(self):
        source = f"{PROSE}\n\n    secret_token = 1\n    more_secret()\n\n{AFTER}"
        self.assertRemoved(source, "secret_token", "more_secret")
        self.assertKept(source, PROSE, AFTER)

    def test_tab_indented_block(self):
        source = f"{PROSE}\n\n\tsecret_token = 1\n\n{AFTER}"
        self.assertRemoved(source, "secret_token")
        self.assertKept(source, PROSE, AFTER)

    def test_blank_line_inside_an_indented_block_keeps_it_open(self):
        source = f"{PROSE}\n\n    secret_one\n\n    secret_two\n\n{AFTER}"
        self.assertRemoved(source, "secret_one", "secret_two")

    def test_indented_continuation_of_a_paragraph_is_kept(self):
        source = "First line of the paragraph,\n    continued here with more words."
        self.assertKept(source, "continued here")


class InlineCode(Case):
    def test_single_backtick_span(self):
        out = self.assertRemoved(f"Calling `secret_token()` fails. {PROSE}", "secret_token", "`")
        self.assertIn(PROSE, out)
        self.assertIn("Calling", out)

    def test_double_backtick_span_holding_a_backtick(self):
        self.assertRemoved("Using ``a ` secret_token`` fails badly.", "secret_token", "`")

    def test_stray_backtick_does_not_swallow_the_paragraph(self):
        out = strip.strip_body(f"Stray ` tick here. {PROSE}")
        self.assertIn(PROSE, out)
        self.assertIn("Stray", out)

    def test_code_span_does_not_cross_paragraphs(self):
        out = strip.strip_body(f"Stray ` tick.\n\n{PROSE}\n\nanother ` tick.")
        self.assertIn(PROSE, out)


class StackTraces(Case):
    def test_python_traceback(self):
        source = (
            f"{PROSE}\n\nTraceback (most recent call last):\n"
            '  File "/opt/thing/secret_module.py", line 12, in run\n'
            "    secret_call()\n"
            "ValueError: secret_value\n\n"
            f"{AFTER}"
        )
        self.assertRemoved(source, "secret_module", "secret_call", "Traceback", "secret_value")
        self.assertKept(source, PROSE, AFTER)

    def test_gdb_frames(self):
        source = (
            f"{PROSE}\n\n"
            "#0  0x00007ffff7a1b2c3 in secret_frame () from /usr/lib/libsecret.so.6\n"
            "#1  0x00007ffff7a1b2d4 in Acme::Widget::doSecret(int) const () at widget.cpp:42\n"
            f"\n{AFTER}"
        )
        self.assertRemoved(source, "secret_frame", "libsecret", "doSecret", "0x00007")
        self.assertKept(source, PROSE, AFTER)

    def test_cpp_frames_without_addresses(self):
        source = f"{PROSE}\n\nin Acme::Widget::doSecret(int) at src/secret_file.cpp:42\n\n{AFTER}"
        self.assertRemoved(source, "doSecret", "secret_file")
        self.assertKept(source, PROSE, AFTER)

    def test_shared_object_lines(self):
        source = f"{PROSE}\n\n/usr/lib/libsecret.so.6(+0x1234)[0x7f00]\nlibother.dll!SecretCall\n\n{AFTER}"
        self.assertRemoved(source, "libsecret", "libother", "SecretCall")
        self.assertKept(source, PROSE, AFTER)

    def test_java_style_frames(self):
        source = f"{PROSE}\n\n  at com.acme.SecretClass.secretMethod(SecretClass.java:12)\n\n{AFTER}"
        self.assertRemoved(source, "secretMethod", "SecretClass")
        self.assertKept(source, PROSE, AFTER)

    def test_prose_about_a_crash_is_kept(self):
        self.assertKept("The application crashes with a segmentation fault when I press Fuse.", "segmentation fault")


class DiffHunks(Case):
    def test_unfenced_unified_diff(self):
        source = (
            f"{PROSE}\n\n"
            "diff --git a/secret_a.cpp b/secret_a.cpp\n"
            "index 1a2b3c4..5d6e7f8 100644\n"
            "--- a/secret_a.cpp\n"
            "+++ b/secret_a.cpp\n"
            "@@ -1,3 +1,4 @@ void secret_function()\n"
            " context_line\n"
            "-removed_secret\n"
            "+added_secret\n"
            f"\n{AFTER}"
        )
        self.assertRemoved(source, "secret_a", "secret_function", "removed_secret", "added_secret", "context_line", "@@")
        self.assertKept(source, PROSE, AFTER)

    def test_run_of_changed_lines_without_a_header(self):
        source = f"{PROSE}\n\n-removed_secret();\n+added_secret();\n\n{AFTER}"
        self.assertRemoved(source, "removed_secret", "added_secret")
        self.assertKept(source, PROSE, AFTER)

    def test_markdown_lists_survive(self):
        out = self.assertKept("Steps:\n\n- open the part\n- add a fillet\n", "open the part", "add a fillet")
        self.assertNotIn("removed", out)


class Links(Case):
    def test_commit_pull_compare_and_patch_links_go_with_their_text(self):
        sources = {
            "/commit/": "[secret text](https://example.invalid/acme/widgets/commit/abc1234)",
            "/pull/": "[secret text](https://example.invalid/acme/widgets/pull/12)",
            "/compare/": "[secret text](https://example.invalid/acme/widgets/compare/a...b)",
            ".patch": "[secret text](https://example.invalid/acme/fix.patch)",
            ".diff": "[secret text](https://example.invalid/acme/fix.diff)",
        }
        for marker, link in sources.items():
            with self.subTest(marker=marker):
                out = self.assertRemoved(f"{PROSE} See {link}.", "secret text", "example.invalid", marker)
                self.assertIn(PROSE, out)

    def test_bare_urls_are_removed(self):
        out = self.assertRemoved(f"{PROSE} See https://example.invalid/acme/thing and www.example.invalid/x.", "example.invalid", "https://")
        self.assertIn(PROSE, out)

    def test_angle_bracket_autolinks_are_removed(self):
        self.assertRemoved(f"{PROSE} <https://example.invalid/acme/thing>", "example.invalid")

    def test_reference_style_definitions_are_removed(self):
        self.assertRemoved(f"{PROSE}\n\n[1]: https://example.invalid/acme/commit/abc\n", "example.invalid")

    def test_plain_page_links_keep_their_text(self):
        out = self.assertKept(f"{PROSE} See the [forum thread](https://example.invalid/t/1).", "forum thread")
        self.assertNotIn("example.invalid", out)

    def test_markdown_images(self):
        source = f"{PROSE} ![secret alt](https://example.invalid/a.png)"
        self.assertRemoved(source, "secret alt", "example.invalid", "a.png")
        self.assertKept(source, PROSE)

    def test_html_images(self):
        source = f'{PROSE} <img width="10" src="https://example.invalid/a.png" alt="secret alt">'
        self.assertRemoved(source, "secret alt", "example.invalid", "<img")
        self.assertKept(source, PROSE)

    def test_attachments(self):
        for link in (
            "[broken.FCStd](https://example.invalid/acme/widgets/files/1234/broken.FCStd)",
            "[model.zip](https://example.invalid/user-attachments/files/9/model.zip)",
            "[part.step](https://example.invalid/attachments/9/part.step)",
        ):
            with self.subTest(link=link):
                out = self.assertRemoved(f"{PROSE} Model: {link}", "example.invalid")
                self.assertNotIn("FCStd", out)
                self.assertNotIn("model.zip", out)
                self.assertNotIn("part.step", out)

    def test_issue_and_commit_references_are_removed(self):
        out = self.assertRemoved(
            f"{PROSE} Fixed by #1234 and acme/widgets#99 in abc1234def5, see GH-77.",
            "#1234",
            "#99",
            "abc1234def5",
            "GH-77",
        )
        self.assertIn(PROSE, out)

    def test_ordinary_hex_like_words_survive(self):
        self.assertKept("The face and the decade of cafe work fine.", "decade", "face")


class PersonalData(Case):
    def test_email_addresses(self):
        out = self.assertRemoved(f"{PROSE} Contact made.up.name@example.invalid for details.", "made.up.name", "example.invalid", "@")
        self.assertIn(PROSE, out)

    def test_mentions(self):
        out = self.assertRemoved(f"Thanks @made-up-user. {PROSE} cc @other_one and @some-org/some-team", "made-up-user", "other_one", "some-team", "@")
        self.assertIn(PROSE, out)

    def test_at_sign_in_units_of_prose_is_left_alone(self):
        self.assertKept("It renders at 60 fps @ 1080 pixels.", "fps")

    def test_trailer_lines_are_removed(self):
        source = f"{PROSE}\n\nSigned-off-by: Made Up <m@example.invalid>\nCo-authored-by: Other Person\nReported-by: Some Body\n"
        self.assertRemoved(source, "Made Up", "Other Person", "Some Body", "example.invalid")
        self.assertKept(source, PROSE)

    def test_reporter_fields_are_removed(self):
        source = f"Reporter: Madeup Person\nAssigned to: Other Person\n{PROSE}"
        self.assertRemoved(source, "Madeup Person", "Other Person")
        self.assertKept(source, PROSE)

    def test_reported_by_phrases(self):
        out = self.assertRemoved(f"Originally reported by Madeup Person in the forum. {PROSE}", "Madeup Person")
        self.assertIn(PROSE, out)
        out = self.assertRemoved(f"Thanks to Madeup Person for the model. {PROSE}", "Madeup Person")
        self.assertIn(PROSE, out)

    def test_quoted_attribution_lines_are_removed(self):
        self.assertRemoved(f"Madeup Person wrote:\n> quoted secret\n\n{PROSE}", "Madeup Person", "quoted secret")

    def test_blockquotes_are_removed(self):
        source = f"{PROSE}\n\n> quoted secret\n> and more\n\n{AFTER}"
        self.assertRemoved(source, "quoted secret", "and more")
        self.assertKept(source, PROSE, AFTER)

    def test_sign_off_block_is_cut(self):
        source = f"{PROSE}\n\nBest regards,\nMadeup Person\n"
        self.assertRemoved(source, "Madeup Person", "Best regards")
        self.assertKept(source, PROSE)

    def test_user_paths_are_removed(self):
        source = f"{PROSE} Saved in /home/madeup/models/box.step and C:\\Users\\madeup\\box.step ok."
        self.assertRemoved(source, "madeup", "/home", "C:\\")
        self.assertKept(source, PROSE)


class Identifiers(Case):
    def test_namespaced_identifiers_reduce_to_words(self):
        out = strip.strip_body("Calling Acme_MakeFillet on the edge fails.")
        self.assertNotIn("Acme_MakeFillet", out)
        self.assertIn("make fillet", out)

    def test_scoped_cpp_names_are_removed(self):
        out = strip.strip_body("Then Acme::Widget::make fails on the face.")
        self.assertNotIn("Acme::", out)
        self.assertIn("fails on the face", out)

    def test_a_scoped_name_left_by_a_removed_code_span_goes_too(self):
        out = strip.strip_body("The check flags `Foo`::SecretCall here.")
        self.assertNotIn("SecretCall", out)
        self.assertIn("The check flags", out)

    def test_a_bare_include_token_is_removed(self):
        out = strip.strip_body("Then the line holds * #include and more prose.")
        self.assertNotIn("#include", out)

    def test_a_relative_backslash_path_is_removed(self):
        out = strip.strip_body("Affected in src\\SecretPackage\\ and more prose.")
        self.assertNotIn("SecretPackage", out)
        self.assertIn("and more prose", out)

    def test_call_expressions_are_removed(self):
        out = strip.strip_body("I run part.makeBox(1,2,3) and it fails.")
        self.assertNotIn("makeBox", out)
        self.assertIn("fails", out)

    def test_source_file_names_are_removed(self):
        out = strip.strip_body("It breaks in widget_thing.cpp around the loop.")
        self.assertNotIn("widget_thing", out)
        self.assertIn("around the loop", out)


class UnfencedCode(Case):
    def test_lines_that_end_a_statement(self):
        source = f"{PROSE}\n\nint secret_count = 3;\nfor (;;) {{\n}}\n\n{AFTER}"
        self.assertRemoved(source, "secret_count", "for (;;)")
        self.assertKept(source, PROSE, AFTER)

    def test_preprocessor_and_python_statements(self):
        source = (
            f"{PROSE}\n\n#include <secret_header.hxx>\n"
            "import secret_module\nfrom acme import secret_thing\ndef secret_fn(a):\n"
            "secret = App.ActiveDocument.addObject(1)\nPart.show(secret_shape)\n\n"
            f"{AFTER}"
        )
        self.assertRemoved(source, "secret_header", "secret_module", "secret_thing", "secret_fn", "addObject", "secret_shape")
        self.assertKept(source, PROSE, AFTER)

    def test_a_sentence_with_import_in_it_survives(self):
        self.assertKept("The import of the step file fails.", "import of the step file")


class InternalsProse(Case):
    def test_sentences_about_code_paths_are_dropped(self):
        source = (
            "The fillet fails on this edge. The function returns false when the check fails. "
            "As read at one, this takes the secret branch. In the implementation the loop is wrong. "
            "The result is an empty solid."
        )
        out = strip.strip_body(source)
        self.assertIn("The fillet fails on this edge.", out)
        self.assertIn("The result is an empty solid.", out)
        for needle in ("returns false", "secret branch", "implementation"):
            self.assertNotIn(needle, out)

    def test_a_sentence_about_a_tool_function_survives(self):
        self.assertIn("The Sweep function fails on this profile.", strip.strip_body("The Sweep function fails on this profile."))

    def test_the_sentence_filter_applies_to_titles_too(self):
        self.assertEqual(strip.strip_title("Fuse fails on a box"), "Fuse fails on a box")


class Tidy(Case):
    def test_only_whitespace_lines_are_collapsed(self):
        out = strip.strip_body(f"{PROSE}\n\n\n\n{AFTER}\n\n")
        self.assertEqual(out, f"{PROSE}\n\n{AFTER}")

    def test_empty_and_none_bodies(self):
        self.assertEqual(strip.strip_body(""), "")
        self.assertEqual(strip.strip_body(None), "")


class Titles(unittest.TestCase):
    def test_title_loses_inline_code_and_mentions(self):
        out = strip.strip_title("Fuse fails: `secret_token` for @made-up-user")
        self.assertNotIn("secret_token", out)
        self.assertNotIn("made-up-user", out)
        self.assertIn("Fuse fails", out)

    def test_title_keeps_a_plain_title_unchanged(self):
        self.assertEqual(strip.strip_title("Fillet of an edge gives a bad solid"), "Fillet of an edge gives a bad solid")

    def test_title_keeps_its_bracket_tag_for_the_defect_filter(self):
        self.assertEqual(strip.strip_title("[Feature Request] New tool"), "[Feature Request] New tool")


class Output(unittest.TestCase):
    ITEM = {
        "repo": "acme/widgets",
        "number": 12,
        "title": "Fuse of `secret_token` fails",
        "body": "It fails.\n\n```\nsecret_code\n```\n",
        "state": "open",
        "labels": ["kind: bug"],
        "created": "2025-01-02",
    }

    def test_strip_issue_keeps_only_the_stripped_fields(self):
        out = strip.strip_issue(self.ITEM)
        self.assertEqual(set(out), {"repo", "number", "labels", "state", "title", "body"})
        self.assertNotIn("secret_token", out["title"])
        self.assertNotIn("secret_code", out["body"])
        self.assertEqual(out["number"], 12)

    def test_strip_cache_writes_one_record_per_issue(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            raw = root / "raw"
            raw.mkdir()
            duplicate = {"number": 12, "title": "x", "body": "y", "state": "open", "labels": [], "created": "2025-01-02"}
            for page, items in (("1", [dict(self.ITEM), dict(duplicate)]), ("2", [dict(self.ITEM, number=13)])):
                (raw / f"acme__widgets__x__page-000{page}.json").write_text(
                    json.dumps({"repo": "acme/widgets", "items": [dict(item, repo="acme/widgets") for item in items], "has_next": False})
                )
            out_path = root / "out.jsonl"
            counts = strip.strip_cache(raw, out_path)
            lines = [json.loads(line) for line in out_path.read_text().splitlines() if line]
        self.assertEqual(sorted(item["number"] for item in lines), [12, 13])
        self.assertEqual(counts["written"], 2)
        self.assertNotIn("secret_code", json.dumps(lines))


if __name__ == "__main__":
    unittest.main()
