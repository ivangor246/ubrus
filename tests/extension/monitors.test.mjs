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

test('assignments survive duplicate serials, undocking, and a Shell restart', () => {
    const left = monitor(0, output('DP-1', '42'));
    const right = monitor(800, output('DP-2', '42'));
    const history = new Map();
    const both = monitorIdentities([left, right], history);
    assert.equal(monitorIdentities([left], history).get('0,0').id, both.get('0,0').id);
    const restored = new Map(JSON.parse(JSON.stringify([...history])));
    assert.equal(monitorIdentities([right], restored).get('800,0').id, both.get('800,0').id);
    assert.deepEqual(monitorIdentities([left, right], restored), both);
});

test('a display configured before its duplicate arrives keeps its original assignment', () => {
    const history = new Map();
    const left = monitor(0, output('DP-1', '42'));
    const right = monitor(800, output('DP-2', '42'));
    const first = monitorIdentities([left], history).get('0,0').id;
    const pair = monitorIdentities([right, left], history);
    assert.equal(pair.get('0,0').id, first);
    assert.notEqual(pair.get('800,0').id, first);
});

test('a legacy connector assignment can arrive after Shell enumerates one remaining display', () => {
    const history = new Map();
    const remaining = [monitor(0, output('DP-1', '42'))];
    monitorIdentities(remaining, history);
    const port = JSON.stringify(['vendor', 'model', 'DP-1']);
    assert.equal(monitorIdentities(remaining, history, { [port]: {} }).get('0,0').id, port);
    assert.equal(monitorIdentities(remaining, history).get('0,0').id, port);
});
