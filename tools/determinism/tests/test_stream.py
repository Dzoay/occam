"""The separate implementation reproduces the published generator, IEEE's special cases, and the record."""

import math
import unittest

import stream


class TheGeneratorIsSplitMix64(unittest.TestCase):
    def test_it_reproduces_the_published_sequence(self):
        generator = stream.SplitMix64(1_234_567)
        want = [
            6_457_827_717_110_365_317,
            3_203_168_211_198_807_973,
            9_817_491_932_198_370_423,
            4_593_380_528_125_082_431,
            16_408_922_859_458_223_821,
        ]
        self.assertEqual([generator.next_u64() for _ in want], want)


class ArithmeticIsIeee(unittest.TestCase):
    def test_division_by_zero_gives_signed_infinities_and_nan(self):
        self.assertEqual(stream.div(1.0, -0.0), -math.inf)
        self.assertEqual(stream.div(-1.0, -0.0), math.inf)
        self.assertTrue(math.isnan(stream.div(0.0, 0.0)))
        self.assertTrue(math.isnan(stream.div(math.nan, 0.0)))

    def test_the_square_root_of_a_negative_is_nan_and_of_minus_zero_is_minus_zero(self):
        self.assertTrue(math.isnan(stream.sqrt(-1.0)))
        self.assertEqual(stream.bits(stream.sqrt(-0.0)), stream.bits(-0.0))

    def test_every_nan_hashes_as_one_pattern_and_the_zeros_stay_apart(self):
        self.assertEqual(stream.canonical_bits(-math.nan), stream.CANONICAL_NAN)
        self.assertNotEqual(stream.canonical_bits(0.0), stream.canonical_bits(-0.0))

    def test_a_length_spans_the_exponents_it_claims(self):
        self.assertEqual(stream.bits(stream.length(0)), 0x3F00_0000_0000_0000)
        self.assertEqual(stream.bits(stream.length(0x01F0_0000_0000_0000)), 0x40F0_0000_0000_0000)

    def test_the_special_values_are_sixteen_distinct_patterns(self):
        self.assertEqual(len({stream.canonical_bits(x) for x in stream.SPECIAL}), 16)


class TheRecordComesFromHere(unittest.TestCase):
    def test_the_recorded_hashes_are_this_implementations(self):
        self.assertEqual(stream.recorded(), stream.run())


if __name__ == "__main__":
    unittest.main()
