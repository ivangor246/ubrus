"""Replay X11 resize and repaint events in a fixed order."""

import os
import unittest
from tempfile import TemporaryDirectory
from unittest.mock import Mock, call, patch

from PIL import Image
from scenarios.x11_settings import SettingsWindow, check_responsive, wait_for_editor


def editor_frame(width, height):
    frame = Image.new('RGB', (width, height))
    center = width // 2 if width == 620 else width - 175
    half = round(height * 0.16)
    frame.paste((37, 91, 156), (center - 100, 120, center + 100, 120 + half))
    frame.paste((32, 132, 61), (center - 100, 120 + half, center + 100, 120 + 2 * half))
    return frame


class ResizeTest(unittest.TestCase):
    def test_waits_for_narrow_editor_after_resize(self):
        wide = editor_frame(1440, 900)
        narrow = editor_frame(620, 430)
        # A clipped wide frame can still show the gallery tile on the left.
        wide.paste((37, 91, 156), (40, 170, 140, 210))
        stale = wide.crop((0, 0, 620, 430))
        gallery = Image.new('RGB', narrow.size)
        gallery.paste((37, 91, 156), (40, 200, 140, 240))
        incomplete = Image.new('RGB', narrow.size)
        incomplete.putpixel((310, 120), (37, 91, 156))
        incomplete.putpixel((310, 257), (32, 132, 61))
        window = Mock(spec=SettingsWindow)
        window.id = 'editor'
        window.capture.side_effect = [
            wide, wide, stale, Image.new('RGB', narrow.size), incomplete, narrow,
            gallery, narrow, editor_frame(900, 600),
        ]
        with TemporaryDirectory() as logs, patch.dict(os.environ, UBRUS_TEST_LOG_DIR=logs):
            with patch('scenarios.x11_settings.xdotool'):
                check_responsive(Mock(), window, 'resize')
        window.key.assert_called_once_with('Escape')
        window.click.assert_called_once()
        x, y = window.click.call_args.args
        self.assertEqual(gallery.getpixel((x, y)), (37, 91, 156))
        window.assert_has_calls([call.key('Escape'), call.capture(), call.click(x, y)])

    def test_rejects_preview_overlapping_footer(self):
        frame = editor_frame(620, 430)
        frame.putpixel((310, 418), (32, 132, 61))
        window = Mock(spec=SettingsWindow)
        window.capture.return_value = frame
        with self.assertRaisesRegex(AssertionError, 'overlaps the editor footer'):
            wait_for_editor(window, 620, 430)

    def test_interpolated_color_boundary_keeps_its_height(self):
        frame = editor_frame(1440, 900)
        frame.paste((35, 111, 108), (1165, 259, 1365, 269))
        window = Mock(spec=SettingsWindow)
        window.capture.return_value = frame
        self.assertIs(wait_for_editor(window, 1440, 900), frame)


if __name__ == '__main__':
    unittest.main()
