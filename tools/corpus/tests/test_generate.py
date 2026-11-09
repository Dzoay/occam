"""The generated corpus: deterministic, valid, attributed, and right on a hand-computed case per family."""

import json
import math
import pathlib
import tempfile
import unittest

import generate
from occam_oracle import caseformat as schema

GENERATED = pathlib.Path(generate.__file__).resolve().parent / "generated"
PI = math.pi


def tree(root):
    return {path.name: path.read_bytes() for path in sorted(pathlib.Path(root).glob("*.json"))}


class GeneratorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cases = generate.generate_all()

    def test_two_runs_write_identical_files(self):
        with tempfile.TemporaryDirectory() as first, tempfile.TemporaryDirectory() as second:
            generate.write_cases(generate.generate_all(), first)
            generate.write_cases(generate.generate_all(), second)
            self.assertTrue(tree(first))
            self.assertEqual(tree(first), tree(second))

    def test_a_rerun_removes_files_that_are_no_longer_generated(self):
        with tempfile.TemporaryDirectory() as out:
            stale = pathlib.Path(out) / "stale-0001.json"
            stale.write_text("{}\n")
            generate.write_cases(generate.generate_all(), out)
            self.assertFalse(stale.exists())

    def test_the_committed_files_match_the_generator(self):
        with tempfile.TemporaryDirectory() as out:
            generate.write_cases(generate.generate_all(), out)
            self.assertEqual(tree(out), tree(GENERATED))

    def test_at_least_200_cases(self):
        self.assertGreaterEqual(len(self.cases), 200)

    def test_ids_are_unique(self):
        ids = [case["id"] for case in self.cases]
        self.assertEqual(len(ids), len(set(ids)))

    def test_every_case_parses_against_the_schema(self):
        for case in self.cases:
            with self.subTest(case=case["id"]):
                schema.validate_case(case)

    def test_every_case_states_origin_and_licence(self):
        for case in self.cases:
            with self.subTest(case=case["id"]):
                self.assertEqual(case["licence"], "MIT OR Apache-2.0")
                self.assertRegex(case["origin"], r"tools/corpus/generate\.py .*family .+ seed \d+")

    def test_every_family_has_a_table_row_that_adds_up(self):
        counts = generate.family_counts(self.cases)
        self.assertEqual(sum(counts.values()), len(self.cases))
        families_named = {generate.family_of(case) for case in self.cases}
        self.assertEqual(set(counts), families_named)
        self.assertGreater(len(counts), 12)

    def test_the_cases_are_small(self):
        """Each case stays small so the corpus remains committable."""
        for case in self.cases:
            with self.subTest(case=case["id"]):
                self.assertLess(len(json.dumps(case)), 4096)

    def test_near_tangent_groups_carry_the_four_identity_roles_and_no_expect(self):
        groups = {}
        for case in self.cases:
            tag = case.get("volume_identity")
            if tag is not None:
                groups.setdefault(tag["group"], {})[tag["role"]] = case
        self.assertGreater(len(groups), 10)
        for name, members in groups.items():
            with self.subTest(group=name):
                self.assertEqual(sorted(members), ["a", "b", "intersection", "union"])
                for member in members.values():
                    self.assertNotIn("expect", member)

    def test_near_tangent_offsets_span_several_orders_of_magnitude(self):
        offsets = sorted({case["offset"] for case in generate.near_tangent_catalogue()})
        self.assertLess(offsets[0], 1e-8)
        self.assertGreater(offsets[-1], 1e-2)


class Answers:
    def assert_answer(self, body, volume=None, area=None, euler=None, bbox=None):
        expect = body.expect
        if volume is not None:
            self.assertAlmostEqual(expect["volume"]["value"], volume, delta=1e-9 * abs(volume) + 1e-12)
        if area is not None:
            self.assertAlmostEqual(expect["area"]["value"], area, delta=1e-9 * abs(area) + 1e-12)
        if euler is not None:
            self.assertEqual(expect["euler"]["value"], euler)
        if bbox is not None:
            for got, want in zip(expect["bbox"]["min"] + expect["bbox"]["max"], bbox[0] + bbox[1]):
                self.assertAlmostEqual(got, want, delta=1e-9)


class PrimitiveClosedForms(Answers, unittest.TestCase):
    def test_box(self):
        self.assert_answer(generate.build_box((2, 3, 4), (1, 0, 0)), volume=24, area=52, euler=2, bbox=([1, 0, 0], [3, 3, 4]))

    def test_cylinder(self):
        body = generate.build_cylinder(2, 5, (0, 0, 0), (0, 0, 1))
        self.assert_answer(body, volume=20 * PI, area=28 * PI, euler=2, bbox=([-2, -2, 0], [2, 2, 5]))

    def test_frustum(self):
        # V = pi*h*(9+3+1)/3 = 13*pi; slant = sqrt(3^2+2^2) = sqrt(13); A = 10*pi + 4*sqrt(13)*pi.
        body = generate.build_frustum(3, 1, 3, (0, 0, 0), (0, 0, 1))
        self.assert_answer(body, volume=13 * PI, area=10 * PI + 4 * math.sqrt(13) * PI, euler=2, bbox=([-3, -3, 0], [3, 3, 3]))

    def test_sphere(self):
        self.assert_answer(generate.build_sphere(3, (1, 1, 1)), volume=36 * PI, area=36 * PI, euler=2, bbox=([-2, -2, -2], [4, 4, 4]))

    def test_torus(self):
        body = generate.build_torus(3, 1, (0, 0, 0), (0, 0, 1))
        self.assert_answer(body, volume=6 * PI * PI, area=12 * PI * PI, euler=0, bbox=([-4, -4, -1], [4, 4, 1]))


class BooleanClosedForms(Answers, unittest.TestCase):
    def test_box_corner_fuse_cut_common(self):
        a, b = ((0, 0, 0), (2, 2, 2)), ((1, 1, 1), (2, 2, 2))
        self.assert_answer(generate.build_box_pair("corner", "fuse", *a, *b), volume=15, area=42, euler=2, bbox=([0, 0, 0], [3, 3, 3]))
        self.assert_answer(generate.build_box_pair("corner", "cut", *a, *b), volume=7, area=24, euler=2, bbox=([0, 0, 0], [2, 2, 2]))
        self.assert_answer(generate.build_box_pair("corner", "common", *a, *b), volume=1, area=6, euler=2, bbox=([1, 1, 1], [2, 2, 2]))

    def test_box_slab_coplanar_sides(self):
        a, b = ((0, 0, 0), (2, 2, 2)), ((0, 0, 1), (2, 2, 2))
        self.assert_answer(generate.build_box_pair("slab", "fuse", *a, *b), volume=12, area=32, euler=2, bbox=([0, 0, 0], [2, 2, 3]))
        self.assert_answer(generate.build_box_pair("slab", "cut", *a, *b), volume=4, area=16, euler=2, bbox=([0, 0, 0], [2, 2, 1]))
        self.assert_answer(generate.build_box_pair("slab", "common", *a, *b), volume=4, area=16, euler=2, bbox=([0, 0, 1], [2, 2, 2]))

    def test_box_stacked_touching_faces(self):
        a, b = ((0, 0, 0), (2, 2, 2)), ((0.5, 0.5, 2), (1, 1, 1))
        self.assert_answer(generate.build_box_pair("stacked", "fuse", *a, *b), volume=9, area=28, euler=2, bbox=([0, 0, 0], [2, 2, 3]))
        self.assert_answer(generate.build_box_pair("stacked", "cut", *a, *b), volume=8, area=24, euler=2, bbox=([0, 0, 0], [2, 2, 2]))

    def test_plate_with_one_hole_has_euler_zero(self):
        body = generate.build_plate((10, 10, 2), (0, 0, 0), [(5, 5, 1)])
        self.assert_answer(body, volume=200 - 2 * PI, area=280 + 2 * PI, euler=0, bbox=([0, 0, 0], [10, 10, 2]))

    def test_plate_with_several_holes_has_euler_two_minus_two_genus(self):
        body = generate.build_plate((10, 10, 2), (0, 0, 0), [(3, 3, 1), (7, 7, 1)])
        self.assert_answer(body, volume=200 - 4 * PI, area=280 + 4 * PI, euler=-2, bbox=([0, 0, 0], [10, 10, 2]))

    def test_sphere_cap_cut_and_common(self):
        # R=5, plane at distance 3, h=2: cap volume = pi*4*(15-2)/3 = 52*pi/3, disc radius sqrt(25-9) = 4.
        cut = generate.build_sphere_cap("cut", 5, (0, 0, 0), 2, 1, 3)
        self.assert_answer(cut, volume=448 * PI / 3, area=96 * PI, euler=2, bbox=([-5, -5, -5], [5, 5, 3]))
        common = generate.build_sphere_cap("common", 5, (0, 0, 0), 2, 1, 3)
        self.assert_answer(common, volume=52 * PI / 3, area=36 * PI, euler=2, bbox=([-4, -4, 3], [4, 4, 5]))

    def test_sphere_cap_below_the_equator_keeps_the_small_part(self):
        # Plane at -3 (h=8): the cut keeps only the part below z=-3, whose widest circle is the disc of radius 4.
        cut = generate.build_sphere_cap("cut", 5, (0, 0, 0), 2, 1, -3)
        self.assert_answer(cut, volume=52 * PI / 3, area=36 * PI, euler=2, bbox=([-4, -4, -5], [4, 4, -3]))

    def test_steinmetz_common(self):
        body = generate.build_steinmetz("common", 3, 10)
        self.assert_answer(body, volume=144, area=144, euler=2, bbox=([-3, -3, -3], [3, 3, 3]))

    def test_steinmetz_fuse_and_cut(self):
        self.assert_answer(generate.build_steinmetz("fuse", 3, 10), volume=180 * PI - 144, area=156 * PI - 144, euler=2, bbox=([-5, -5, -3], [5, 5, 3]))
        self.assert_answer(generate.build_steinmetz("cut", 3, 10), volume=90 * PI - 144, area=78 * PI)

    def test_coaxial_equal_radius(self):
        self.assert_answer(generate.build_coaxial_equal("fuse", 1, 4, 2, 4, (0, 0, 0), (0, 0, 1)), volume=6 * PI, area=14 * PI, euler=2)
        self.assert_answer(generate.build_coaxial_equal("common", 1, 4, 2, 4, (0, 0, 0), (0, 0, 1)), volume=2 * PI, area=6 * PI, euler=2)
        self.assert_answer(generate.build_coaxial_equal("cut", 1, 4, 2, 4, (0, 0, 0), (0, 0, 1)), volume=2 * PI, area=6 * PI, euler=2)

    def test_coaxial_stepped(self):
        self.assert_answer(generate.build_coaxial_stepped("fuse", 2, 1, 4, 2, 4, (0, 0, 0), (0, 0, 1)), volume=18 * PI, area=28 * PI, euler=2)
        self.assert_answer(generate.build_coaxial_stepped("common", 2, 1, 4, 2, 4, (0, 0, 0), (0, 0, 1)), volume=2 * PI, area=6 * PI, euler=2)
        self.assert_answer(generate.build_coaxial_stepped("cut", 2, 1, 4, 2, 4, (0, 0, 0), (0, 0, 1)), volume=14 * PI, area=28 * PI, euler=2)

    def test_coaxial_tube_has_euler_zero(self):
        body = generate.build_tube(2, 1, 4, (0, 0, 0), (0, 0, 1))
        self.assert_answer(body, volume=12 * PI, area=30 * PI, euler=0, bbox=([-2, -2, 0], [2, 2, 4]))

    def test_sphere_lens(self):
        # R=r=d=3: plane at x=1.5, two caps of height 1.5, each of volume pi*2.25*7.5/3 = 5.625*pi.
        self.assert_answer(generate.build_sphere_lens("common", 3, 3, 3, (0, 0, 0), (1, 0, 0)), volume=11.25 * PI, area=18 * PI, euler=2)
        self.assert_answer(generate.build_sphere_lens("fuse", 3, 3, 3, (0, 0, 0), (1, 0, 0)), volume=60.75 * PI, area=54 * PI, euler=2)
        self.assert_answer(generate.build_sphere_lens("cut", 3, 3, 3, (0, 0, 0), (1, 0, 0)), volume=24.75 * PI, area=36 * PI, euler=2)

    def test_sphere_lens_matches_the_textbook_formula(self):
        # V = pi*(R+r-d)^2*(d^2+2d*r-3r^2+2d*R+6R*r-3R^2)/(12d) for R=4, r=3, d=5.
        R, r, d = 4, 3, 5
        lens = PI * (R + r - d) ** 2 * (d * d + 2 * d * r - 3 * r * r + 2 * d * R + 6 * R * r - 3 * R * R) / (12 * d)
        self.assert_answer(generate.build_sphere_lens("common", R, r, d, (0, 0, 0), (1, 0, 0)), volume=lens, euler=2)

    def test_fillet_one_box_edge(self):
        # abc - (1 - pi/4)*r^2*L = 1000 - 40 + 10*pi; A = 600 - 40 + 10*pi - 8 + 2*pi.
        body = generate.build_fillet_edge((10, 10, 10), (0, 0, 0), 2, (1, 1), 2)
        self.assert_answer(body, volume=960 + 10 * PI, area=552 + 12 * PI, euler=2, bbox=([0, 0, 0], [10, 10, 10]))

    def test_fillet_all_edges_of_a_box(self):
        # Inner box 8x8x8: 512 + 2*192 + 24*pi + 4*pi/3; area 384 + 48*pi + 4*pi.
        body = generate.build_fillet_all((10, 10, 10), (0, 0, 0), 1)
        self.assert_answer(body, volume=896 + 76 * PI / 3, area=384 + 52 * PI, euler=2, bbox=([0, 0, 0], [10, 10, 10]))

    def test_tessellated_sphere_keeps_the_sphere_answer(self):
        body = generate.build_tessellated("sphere", (5,), (0.1, 0.5, False))
        self.assert_answer(body, volume=500 * PI / 3, area=100 * PI, euler=2)
        self.assertEqual([op["op"] for op in body.ops], ["sphere", "tessellate"])
        self.assertEqual(body.expect["watertight"], {"value": True})


class NearTangentGeometry(unittest.TestCase):
    def test_external_crossing_puts_the_axes_at_radius_sum_minus_offset(self):
        roles = generate.build_near_crossed("external", 2, 1, 0.001)
        cylinders = [op for op in roles["a"].ops + roles["b"].ops if op["op"] == "cylinder"]
        self.assertEqual({cylinder["radius"] for cylinder in cylinders}, {2, 1})
        self.assertAlmostEqual(cylinders[1]["origin"][2], 2.999, places=12)
        self.assertEqual(roles["union"].ops[-1]["op"], "fuse")
        self.assertEqual(roles["intersection"].ops[-1]["op"], "common")
        for role in ("a", "b", "union", "intersection"):
            self.assertIsNone(roles[role].expect)

    def test_near_steinmetz_offsets_equal_cylinders_by_the_offset(self):
        roles = generate.build_near_crossed("steinmetz", 3, 3, 1e-7)
        cylinders = [op for op in roles["b"].ops if op["op"] == "cylinder"]
        self.assertEqual(len(cylinders), 1)
        self.assertEqual(cylinders[0]["radius"], 3)
        self.assertAlmostEqual(cylinders[0]["origin"][2], 1e-7, places=15)

    def test_sphere_near_tangent_box_face_sits_inside_by_the_offset(self):
        roles = generate.build_near_sphere_box(5, 2, 1, 1e-6)
        box = next(op for op in roles["b"].ops if op["op"] == "box")
        self.assertAlmostEqual(box["origin"][2], 5 - 1e-6, places=12)
        self.assertEqual(roles["cut"].ops[-1]["op"], "cut")
        for role in ("a", "b", "union", "intersection", "cut"):
            self.assertIsNone(roles[role].expect)


if __name__ == "__main__":
    unittest.main()
