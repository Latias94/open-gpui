"""Fixture path checks: python -m unittest discover -s scripts -p test_compare_benchmarks.py."""

from pathlib import Path
import tempfile
import unittest

from compare_benchmarks import fixture_destination


class FixtureDestinationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="gpui-benchmark-fixtures-")
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name).resolve()
        self.source = self.directory / "source"
        self.source.mkdir()
        self.external = self.directory / "external"
        self.external.mkdir()
        self.user_file = self.external / "user.rs"
        self.user_file.write_bytes(b"uncommitted user changes")

    def symlink(self, link, target, *, directory=False):
        try:
            link.symlink_to(target, target_is_directory=directory)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"symlinks unavailable: {error}")

    def test_ordinary_fixture_can_be_created_and_updated(self):
        relative = Path("crates/gpui/benches/fixture.rs")
        destination = fixture_destination(self.source, relative)
        self.assertEqual(destination, self.source / relative)
        destination.parent.mkdir(parents=True)
        destination.write_bytes(b"original fixture")
        fixture_destination(self.source, relative).write_bytes(b"updated fixture")
        self.assertEqual(destination.read_bytes(), b"updated fixture")
        self.assertEqual(self.user_file.read_bytes(), b"uncommitted user changes")

    def test_symlink_parent_cannot_create_external_directories(self):
        self.symlink(self.source / "crates", self.external, directory=True)
        with self.assertRaisesRegex(RuntimeError, "refusing symlink"):
            fixture_destination(self.source, Path("crates/gpui/fixture.rs"))
        self.assertFalse((self.external / "gpui").exists())
        self.assertEqual(self.user_file.read_bytes(), b"uncommitted user changes")

    def test_symlink_file_cannot_overwrite_user_changes(self):
        self.symlink(self.source / "fixture.rs", self.user_file)
        with self.assertRaisesRegex(RuntimeError, "refusing symlink"):
            fixture_destination(self.source, Path("fixture.rs"))
        self.assertEqual(self.user_file.read_bytes(), b"uncommitted user changes")

    def test_dangling_symlink_is_rejected(self):
        absent = self.external / "absent.rs"
        self.symlink(self.source / "fixture.rs", absent)
        with self.assertRaisesRegex(RuntimeError, "refusing symlink"):
            fixture_destination(self.source, Path("fixture.rs"))
        self.assertFalse(absent.exists())

    def test_absolute_and_parent_traversal_paths_are_rejected(self):
        for relative in [self.user_file, Path("../external/user.rs"), Path(".")]:
            with self.subTest(relative=relative):
                with self.assertRaisesRegex(RuntimeError, "invalid fixture path"):
                    fixture_destination(self.source, relative)
        self.assertEqual(self.user_file.read_bytes(), b"uncommitted user changes")


if __name__ == "__main__":
    unittest.main()
