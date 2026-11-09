"""The extractor on synthetic fixtures. No FreeCAD source is read here or needed.

Run with `just tools-test`.
"""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import freecad_occt_usage as usage  # noqa: E402

# What the pinned wheel exports, in miniature.
NAMES = {
    "gp_Pnt", "gp", "TopoDS", "TopoDS_Shape", "TopoDS_Edge", "TopoDS_Face",
    "BRepAlgoAPI_Fuse", "BRepAlgoAPI_Common", "BRepTools", "Geom_Curve", "Standard_Failure",
}

PART_CPP = """\
// A comment naming gp_Pnt and TopoDS_Face must not count.
/* Nor this block: BRepAlgoAPI_Fuse */
#include <TopoDS_Shape.hxx>
#include <gp_Pnt.hxx>
#include "BRepAlgoAPI_Fuse.hxx"
#include <SMESH_Mesh.hxx>
#include <vector>

const char *text = "gp_Pnt in a string must not count";
const char *raw = R"x(BRepAlgoAPI_Common in a raw string)x";

void build() {
    gp_Pnt a(0, 0, 0);
    gp_Pnt b(1, 0, 0);
    TopoDS_Shape shape = BRepAlgoAPI_Fuse(left, right).Shape();
    TopoDS_Edge edge = TopoDS::Edge(shape);
    BRepTools::Write(shape, "out.brep");
    Handle(Geom_Curve) curve;
    TopoDS_ShapePy wrapper;
    Standard_Real radius;
    boost::Geom::thing(1);
    Base::TopoDS::Other(2);
}
"""

DESIGN_CPP = """\
#include <gp_Pnt.hxx>
gp_Pnt centre();
gp_Pnt middle();
void tidy() { BRepTools::Clean(shape); }
"""

THIRD_PARTY_CPP = """\
#include <gp_Pnt.hxx>
gp_Pnt bundled;
"""

TEST_CPP = """\
#include <gp_Pnt.hxx>
gp_Pnt fixture;
gp_Pnt other;
"""

NOT_SOURCE = "gp_Pnt gp_Pnt gp_Pnt\n"


def write_tree(root):
    files = {
        "src/Mod/Part/App/Build.cpp": PART_CPP,
        "src/Mod/PartDesign/App/Feature.cpp": DESIGN_CPP,
        "src/3rdParty/bundled/Mesh.cpp": THIRD_PARTY_CPP,
        "tests/src/Mod/Part/Test.cpp": TEST_CPP,
        "src/Mod/Part/App/notes.txt": NOT_SOURCE,
        "src/Mod/Part/App/script.py": NOT_SOURCE,
    }
    for name, text in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")


class Fixture(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dir = tempfile.TemporaryDirectory()
        cls.root = Path(cls.dir.name)
        write_tree(cls.root)
        cls.census = usage.scan(cls.root, NAMES)

    @classmethod
    def tearDownClass(cls):
        cls.dir.cleanup()

    def test_only_source_files_are_scanned(self):
        self.assertEqual(self.census.files_scanned, 4)

    def test_a_name_counts_per_module(self):
        self.assertEqual(self.census.uses["gp_Pnt"], {"Part": 2, "PartDesign": 2, "3rdParty/bundled": 1, "tests": 2})

    def test_comments_and_strings_do_not_count(self):
        self.assertNotIn("TopoDS_Face", self.census.uses)
        self.assertNotIn("BRepAlgoAPI_Common", self.census.uses)
        self.assertEqual(self.census.uses["BRepAlgoAPI_Fuse"], {"Part": 1})

    def test_handles_and_includes(self):
        self.assertEqual(self.census.uses["Geom_Curve"], {"Part": 1})
        self.assertEqual(self.census.includes["BRepAlgoAPI_Fuse"], {"Part": 1})
        self.assertEqual(self.census.includes["gp_Pnt"]["PartDesign"], 1)

    def test_static_calls_where_the_class_is_named(self):
        self.assertEqual(self.census.static_calls["TopoDS::Edge"], {"Part": 1})
        self.assertEqual(self.census.static_calls["BRepTools::Write"], {"Part": 1})
        self.assertEqual(self.census.static_calls["BRepTools::Clean"], {"PartDesign": 1})

    def test_packages_outside_the_list_are_reported_not_counted(self):
        self.assertNotIn("SMESH_Mesh", self.census.uses)
        self.assertEqual(self.census.unrecognised_includes["SMESH"], 1)

    def test_names_the_wheel_does_not_export_are_set_aside(self):
        self.assertNotIn("TopoDS_ShapePy", self.census.uses)
        self.assertEqual(self.census.not_in_wheel["TopoDS_ShapePy"], {"Part": 1})
        self.assertEqual(self.census.not_in_wheel["Standard_Real"], {"Part": 1})

    def test_a_nested_namespace_is_not_a_package_call(self):
        self.assertNotIn("Geom::thing", self.census.static_calls)
        self.assertNotIn("TopoDS::Other", self.census.static_calls)

    def test_the_ranking_leaves_out_tests_and_third_party(self):
        ranked = usage.ranked(self.census.uses)
        self.assertEqual(ranked[0], ("gp_Pnt", 4))
        self.assertEqual(dict(ranked)["BRepAlgoAPI_Fuse"], 1)

    def test_the_report_states_the_commit_and_the_counts(self):
        text = usage.render(self.census, commit="abc123", tag="9.9")
        self.assertIn("abc123", text)
        self.assertIn("| `gp_Pnt` | 4 |", text)
        self.assertIn("`TopoDS::Edge`", text)
        self.assertIn("SMESH", text)
        self.assertIn("`TopoDS_ShapePy`", text)


if __name__ == "__main__":
    unittest.main()
