"""Which corpus cases are in scope, and how they are grouped."""

import unittest

from occam_baseline import cases


def case(case_id, kinds):
    return {
        "id": case_id,
        "ops": [{"op": kind, "id": f"o{i}"} for i, kind in enumerate(kinds)],
        "result": f"o{len(kinds) - 1}",
    }


class TestClassify(unittest.TestCase):
    def test_a_boolean_family(self):
        chosen = cases.classify(case("box-corner-fuse-0001", ["box", "box", "fuse"]))
        self.assertEqual(chosen.group, "booleans-fuse")
        self.assertEqual(chosen.family, "box-corner-fuse")
        self.assertEqual(chosen.timed, {"fuse"})
        self.assertEqual(chosen.kind, "volume")

    def test_near_tangent_is_its_own_group(self):
        chosen = cases.classify(case("near-crossed-external-0001-union", ["cylinder", "cylinder", "fuse"]))
        self.assertEqual(chosen.group, "near-tangent-fuse")
        self.assertEqual(chosen.family, "near-crossed-external-fuse")

    def test_a_tessellation_family_times_only_the_mesher(self):
        chosen = cases.classify(case("tess-plate-0001", ["box", "cylinder", "cut", "tessellate"]))
        self.assertEqual(chosen.group, "tessellation")
        self.assertEqual(chosen.family, "tess-plate")
        self.assertEqual(chosen.timed, {"tessellate"})
        self.assertEqual(chosen.kind, "triangles")

    def test_a_fillet(self):
        chosen = cases.classify(case("fillet-box-all-0001", ["box", "fillet"]))
        self.assertEqual(chosen.group, "fillets")
        self.assertEqual(chosen.timed, {"fillet"})
        self.assertEqual(chosen.kind, "volume")

    def test_a_primitive_alone_is_out_of_scope(self):
        self.assertIsNone(cases.classify(case("prim-box-0001", ["box"])))
        self.assertIsNone(cases.classify(case("near-crossed-external-0001-a", ["cylinder"])))


if __name__ == "__main__":
    unittest.main()
