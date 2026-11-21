import test from 'node:test';
import assert from 'node:assert/strict';
import { monitorIdentities } from '../../extension/desktop/monitors.js';

const output = (connector, serial) => [connector, 'vendor', 'model', serial];
const monitor = (x, ...outputs) => [x, 0, 1, 0, false, outputs, {}];

test('a unique serial follows the display to another connector', () => {
    const initial = monitorIdentities([monitor(0, output('DP-1', '42'))]);
    const moved = monitorIdentities([monitor(800, output('DP-2', '42'))]);
    assert.equal(initial.get('0,0').id, moved.get('800,0').id);
    assert.equal(moved.get('800,0').name, 'model · DP-2');
});

test('separate monitors with missing or duplicate serials use their connectors', () => {
    for (const serial of ['0', 'unknown', '', 'duplicate']) {
        const ids = monitorIdentities([
            monitor(0, output('DP-1', serial)),
            monitor(800, output('DP-2', serial)),
        ]);
        assert.deepEqual(JSON.parse(ids.get('0,0').id), ['vendor', 'model', 'DP-1']);
        assert.deepEqual(JSON.parse(ids.get('800,0').id), ['vendor', 'model', 'DP-2']);
    }
});

test('mirrored connectors are counted as one logical monitor', () => {
    const ids = monitorIdentities([monitor(0, output('DP-1', '42'), output('DP-2', '42'))]);
    assert.deepEqual(JSON.parse(ids.get('0,0').id), ['vendor', 'model', '42']);
});
