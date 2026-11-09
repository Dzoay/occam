"""The rule-based classifier on made-up issues: expected classes, determinism, ranking."""

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import defects_classify as classify  # noqa: E402


def issue(title, body="", labels=(), repo="acme/widgets", number=1):
    return {"repo": repo, "number": number, "labels": list(labels), "state": "open", "title": title, "body": body}


class Expected(unittest.TestCase):
    CASES = [
        ("Fuse of two cylinders returns an empty shape", "boolean"),
        ("Boolean cut gives wrong result on a near tangent pair", "boolean"),
        ("Common of two solids produces an invalid solid", "boolean"),
        ("Fillet fails on a simple edge", "fillet-chamfer"),
        ("Chamfer of this edge crashes", "fillet-chamfer"),
        ("Fails to fillet the edge after a pocket", "fillet-chamfer"),
        ("Gap between two faces after sewing, tolerance too large", "tolerance-gap"),
        ("Tolerance growth makes the wire not closed", "tolerance-gap"),
        ("Mesh of this surface has holes in the triangulation", "tessellation"),
        ("Tessellation produces degenerate triangles on a cone", "tessellation"),
        ("Sewing leaves the shell open and healing makes it worse", "shape-healing"),
        ("Fix shape operation removes a face", "shape-healing"),
        ("Topological naming breaks the pad after an edit", "topo-naming"),
        ("Toponaming problem: wrong face after recompute", "topo-naming"),
        ("Sketch attached to the wrong face after a feature is edited", "topo-naming"),
        ("STEP import loses colours and faces", "exchange"),
        ("Exporting an IGES file gives bad surfaces", "exchange"),
        ("Import of a step file fails", "exchange"),
        ("Sweep along a helix twists the section", "sweep-loft"),
        ("Loft between two profiles fails", "sweep-loft"),
        ("Pipe shell makes a self intersecting solid", "sweep-loft"),
        ("Offset of a face fails on a fillet", "offset-shell"),
        ("Thickness tool makes an empty result", "offset-shell"),
        ("Shell of a body with a fillet fails", "offset-shell"),
        ("Draft angle fails on a cylinder face", "draft"),
        ("Pad fails when the sketch has a slot", "extrude-revolve"),
        ("Revolve around an edge makes a bad solid", "extrude-revolve"),
        ("Very slow recompute of a large model", "performance"),
        ("Boolean takes hours compared to the old version", "performance"),
        ("Memory leak when recomputing a part", "memory"),
        ("Out of memory during meshing", "memory"),
        ("Crash when opening the file", "crash"),
        ("Segmentation fault on boolean", "crash"),
        ("Application hangs during fillet", "hang"),
        ("Freeze forever in an infinite loop", "hang"),
        ("Different result on every run with threads", "nondeterminism"),
        ("Non-deterministic result between runs", "nondeterminism"),
        ("Invalid shape after the operation", "invalid-shape"),
        ("Check geometry reports a self intersection", "invalid-shape"),
        ("Intersection of a curve and a surface misses a point", "intersection"),
        ("Section of a shape returns no edges", "intersection"),
        ("Project a curve on a surface gives wrong result", "intersection"),
        ("Bspline approximation is wildly wrong", "curves-nurbs"),
        ("NURBS surface evaluation is wrong", "curves-nurbs"),
        ("Wrong volume computed for a sphere", "measure"),
        ("Bounding box is too large", "measure"),
        ("Inertia matrix is wrong", "measure"),
        ("Sketch solver reports redundant constraints", "sketch"),
        ("Hidden line removal shows wrong edges in the drawing", "drawing"),
        ("Pattern of a pocket fails", "pattern-transform"),
        ("Mirror of a body gives a wrong shape", "pattern-transform"),
        ("Model is displayed wrongly in the 3D view", "visualization"),
        ("Dialog button does not respond", "gui"),
        ("Build fails with the newest compiler", "build"),
        ("Python binding is missing a method", "python-api"),
        ("Fuse returns an empty shape with no error message", "unhelpful-failure"),
        ("Cannot open the saved FCStd file", "persistence"),
        ("Exported STL is wrong", "exchange-other"),
        ("Wrong unit conversion for inches", "units"),
        ("Assembly joints are wrong", "out-assembly"),
        ("Finite element mesh fails", "out-fem"),
        ("Placement of the coordinate system is ignored when attaching", "placement-attachment"),
        ("Model failed to recompute in the new version", "recompute"),
        ("CAM path has wrong feed", "out-cam"),
    ]

    def test_each_case_gets_its_class(self):
        for title, expected in self.CASES:
            with self.subTest(title=title):
                self.assertIn(expected, classify.classes_of(title, ""))

    NOT_MATCHING = [
        ("Model threads are really slow", "", "nondeterminism"),
        ("Cosmetic threads appear in holes", "", "nondeterminism"),
        ("Parallel faces lose a fillet", "", "nondeterminism"),
        ("Something odd", "The failing example is built in memory first.", "memory"),
        ("Selection focus leaks to another widget", "", "memory"),
        ("Task dialog breaks with Building US units", "", "build"),
        ("Bug present in the latest AppImage", "", "build"),
        ("Attachment offset is forgotten on launch", "", "offset-shell"),
        ("Something odd", "The plane is offset from the original and does not meet the sketch.", "offset-shell"),
        ("Fails with US measurements", "Create a document that uses Building US measurements and it fails.", "measure"),
        ("Something odd", "The tool gets stuck in a compound and fails.", "hang"),
        ("The same frozen model gives an error", "", "hang"),
        ("Something odd", "Mismatched texture unit in the pair causes a failure.", "units"),
        ("The STEP file is fine here", "Everything looks fine in Fusion 360, but it is read wrongly by other programs.", "boolean"),
        ("Section cut fails on a body", "", "boolean"),
        ("Fillet looses edges after Pad editing", "", "extrude-revolve"),
        ("Pattern of holes has odd spacing", "", "pattern-transform"),
        ("UI: Combobox misbehaves in the Chamfer TaskPanel", "", "fillet-chamfer"),
        ("Something odd", "FreeCAD cannot open the exported STEP file.", "persistence"),
        ("Something odd", "I dumped these faces as .brep files and saw similar problems.", "exchange"),
        ("Crash when loading a step file with an invalid entity shell", "", "offset-shell"),
        ("Something odd", "When the file has no levels of solid and shell, the crash happens.", "offset-shell"),
    ]

    def test_near_miss_titles_and_sentences_do_not_match(self):
        for title, body, not_expected in self.NOT_MATCHING:
            with self.subTest(title=title, body=body):
                self.assertNotIn(not_expected, classify.classes_of(title, body))

    MATCHING_AFTER_TUNING = [
        ("Build fails with the newest compiler", "build"),
        ("Cannot build with the new toolchain", "build"),
        ("Linker error on the new toolchain", "build"),
        ("Offset of a face fails on a fillet", "offset-shell"),
        ("Negative offset impossible for a binder", "offset-shell"),
        ("2D Offset produces a bug on a spline", "offset-shell"),
        ("Memory overrun in the text conversion", "memory"),
        ("Fillet causes huge memory consumption", "memory"),
        ("Large memory footprint differences", "memory"),
        ("Quick measure gives wrong results", "measure"),
        ("Pad fails on a slot", "extrude-revolve"),
    ]

    def test_tightened_rules_still_match_the_real_symptom(self):
        for title, expected in self.MATCHING_AFTER_TUNING:
            with self.subTest(title=title):
                self.assertIn(expected, classify.classes_of(title, ""))

    def test_a_mere_mention_of_the_attached_file_is_not_a_persistence_failure(self):
        body = "The attached FCStd file does not show the problem."
        self.assertNotIn("persistence", classify.classes_of("Something odd", body))

    def test_body_needs_topic_and_failure_in_one_sentence(self):
        body = "The chamfer of the edge fails with no explanation."
        self.assertIn("fillet-chamfer", classify.classes_of("Something odd", body))

    def test_a_body_that_mentions_a_topic_without_failure_does_not_count(self):
        self.assertNotIn("fillet-chamfer", classify.classes_of("Something odd", "I made a fillet and a chamfer. Then I saved."))
        self.assertNotIn("boolean", classify.classes_of("Something odd", "I use a boolean sometimes."))

    def test_topic_and_failure_in_different_sentences_do_not_match(self):
        body = "I made a fillet. Later the screen went wrong."
        self.assertNotIn("fillet-chamfer", classify.classes_of("Something odd", body))

    def test_common_as_an_adjective_is_not_a_boolean(self):
        self.assertNotIn("boolean", classify.classes_of("A common problem with the toolbar", ""))

    def test_cut_as_in_clipboard_is_not_a_boolean(self):
        self.assertNotIn("boolean", classify.classes_of("Cut and paste of text fails", ""))

    def test_fuse_inside_another_word_is_not_a_boolean(self):
        self.assertNotIn("boolean", classify.classes_of("Confused by the fuselage", ""))

    def test_an_unrelated_title_has_no_class(self):
        self.assertEqual(classify.classes_of("Typo in the about box", ""), [])

    def test_classes_are_sorted_and_deterministic(self):
        title = "Crash in boolean fuse with a very slow fillet"
        first = classify.classes_of(title, "")
        self.assertEqual(first, sorted(first))
        self.assertEqual(first, classify.classes_of(title, ""))
        self.assertGreater(len(first), 1)


class Explain(unittest.TestCase):
    def test_a_title_hit_is_explained_by_the_title(self):
        self.assertEqual(classify.explain(issue("Fillet fails badly"), "fillet-chamfer"), ["title: Fillet fails badly"])

    def test_a_body_hit_is_explained_by_its_sentence(self):
        item = issue("Something odd", "First sentence is fine. The chamfer of the edge fails here. Other words.")
        self.assertEqual(classify.explain(item, "fillet-chamfer"), ["The chamfer of the edge fails here."])

    def test_a_non_member_explains_nothing(self):
        self.assertEqual(classify.explain(issue("Typo in the about box"), "fillet-chamfer"), [])


class BroadSample(unittest.TestCase):
    def test_broad_sample_labels_are_recognised(self):
        fc = "FreeCAD/FreeCAD"
        self.assertTrue(classify.in_broad_sample(issue("x", labels=["Mod: Part", "Type: Bug"], repo=fc)))
        self.assertTrue(classify.in_broad_sample(issue("x", labels=["Mod: Part Design", "Type: Bug"], repo=fc)))
        self.assertTrue(classify.in_broad_sample(issue("x", labels=["3rd party: OCC"], repo=fc)))
        self.assertTrue(classify.in_broad_sample(issue("x", labels=["2. Bug"], repo="Open-Cascade-SAS/OCCT")))

    def test_topical_pages_are_not_broad(self):
        fc = "FreeCAD/FreeCAD"
        for labels in (["Topic: Fillets"], ["Topic: Toponaming"], ["File format: STEP"], ["Topic: Performance"], []):
            with self.subTest(labels=labels):
                self.assertFalse(classify.in_broad_sample(issue("x", labels=labels, repo=fc)))


class Defects(unittest.TestCase):
    def test_feature_labels_are_not_defects(self):
        self.assertFalse(classify.is_defect(issue("Add a thing", labels=["Type: Feature"])))
        self.assertFalse(classify.is_defect(issue("Add a thing", labels=["enhancement"])))

    def test_question_and_feature_titles_are_not_defects(self):
        self.assertFalse(classify.is_defect(issue("[Feature Request] Add a thing")))
        self.assertFalse(classify.is_defect(issue("Feature request: new tool")))

    def test_bug_and_unlabelled_issues_are_defects(self):
        self.assertTrue(classify.is_defect(issue("Fillet fails", labels=["Type: Bug"])))
        self.assertTrue(classify.is_defect(issue("Fillet fails")))


class Ranking(unittest.TestCase):
    def issues(self):
        return [
            issue("Fuse fails", repo="acme/a", number=1),
            issue("Cut fails with a crash", repo="acme/a", number=2),
            issue("Fuse is wrong", repo="acme/b", number=3),
            issue("Fillet fails", repo="acme/b", number=4),
            issue("Add a thing", labels=["Type: Feature"], repo="acme/a", number=5),
            issue("Typo in the about box", repo="acme/a", number=6),
        ]

    def test_counts_per_source_and_totals(self):
        table = classify.tally(self.issues())
        self.assertEqual(table["sources"], {"acme/a": 3, "acme/b": 2})
        self.assertEqual(table["excluded_not_defects"], 1)
        self.assertEqual(table["unclassified"], 1)
        rows = {row["class"]: row for row in table["rows"]}
        self.assertEqual(rows["boolean"]["total"], 3)
        self.assertEqual(rows["boolean"]["per_source"], {"acme/a": 2, "acme/b": 1})
        self.assertEqual(rows["fillet-chamfer"]["total"], 1)
        self.assertEqual(rows["crash"]["total"], 1)

    def test_rows_rank_by_total_then_name(self):
        table = classify.tally(self.issues())
        self.assertEqual(table["rows"][0]["class"], "boolean")
        names = [row["class"] for row in table["rows"]]
        self.assertEqual(names[1:], sorted(names[1:]))

    def test_sampling_is_deterministic_and_bounded(self):
        issues = [issue(f"Fuse fails {n}", number=n) for n in range(20)]
        first = classify.sample(issues, "boolean", 5, seed="x")
        self.assertEqual(len(first), 5)
        self.assertEqual([item["number"] for item in first], [item["number"] for item in classify.sample(issues, "boolean", 5, seed="x")])
        self.assertNotEqual([item["number"] for item in first], [item["number"] for item in classify.sample(issues, "boolean", 5, seed="y")])

    def test_load_reads_json_lines(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "in.jsonl"
            path.write_text(json.dumps(issue("Fuse fails")) + "\n")
            self.assertEqual(classify.load_issues(path)[0]["title"], "Fuse fails")


if __name__ == "__main__":
    unittest.main()
