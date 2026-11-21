import test from 'node:test';
import assert from 'node:assert/strict';
import { covered, overviewCrop, pauseReason } from '../../extension/desktop/geometry.js';
const area = { x: -50, y: 24, width: 100, height: 70 };
const left = { x: -100, y: 0, width: 100, height: 100 };
const right = { x: 0, y: 0, width: 50, height: 100 };
test('two tiled windows hide the working area of only their monitor', () => {
    assert.equal(covered(area, [left, right]), true);
    assert.equal(covered({ ...area, x: 50 }, [left, right]), false);
});
test('a gap and overlapping windows preserve the visible strip', () => {
    assert.equal(covered(area, [left, left, { ...right, x: 1 }]), false);
    assert.equal(covered(area, []), false);
});
test('manual pause survives changes of system power and visibility', () => {
    assert.equal(pauseReason({ paused: true, power: true }, false, false), 'manual');
    assert.equal(pauseReason({ paused: false, power: false }, true, false), 'visible');
    assert.equal(pauseReason({ paused: false, power: true }, true, false), 'power-saver');
    assert.equal(pauseReason({ paused: false, power: true }, false, true), 'covered');
});

test('workspace previews keep the monitor aspect ratio throughout a resize', () => {
    for (const [sw, sh] of [[1920, 1080], [1080, 1920], [3440, 1440]]) {
        for (const [w, h] of [[sw, sh], [800, 400], [600, 480], [503.5, 270.25]]) {
            const { x1, y1, x2, y2 } = overviewCrop(sw, sh, w, h);
            assert.ok(Math.abs((x2 - x1) / sw - (y2 - y1) / sh) < 1e-9);
            assert.ok(x1 <= 1e-9 && y1 <= 1e-9 && x2 >= w - 1e-9 && y2 >= h - 1e-9);
            assert.ok(Math.abs(x1 + x2 - w) < 1e-9);
            assert.ok(Math.abs(y1 + y2 - h) < 1e-9);
        }
    }
    assert.deepEqual(overviewCrop(1920, 1080, 0, 0), { x1: 0, y1: 0, x2: 0, y2: 0 });
});

test('overview playback defaults to live and an overview-only pause ends on exit', () => {
    assert.equal(pauseReason({}, false, false, true), 'visible');
    const scene = { pause_in_overview: true };
    assert.equal(pauseReason(scene, false, false, true), 'overview');
    assert.equal(pauseReason(scene, false, false, false), 'visible');
    assert.equal(pauseReason(scene, false, true, false), 'covered');
    assert.equal(pauseReason({ ...scene, paused: true }, false, false, false), 'manual');
});
