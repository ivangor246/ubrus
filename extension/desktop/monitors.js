// A display with a unique hardware serial keeps its identity when moved to another connector.
function hardwareId([, vendor, product, serial]) {
    if (!serial || serial === '0' || serial === 'unknown') return null;
    return JSON.stringify([vendor, product, serial]);
}

export function monitorIdentities(logicalMonitors) {
    const occurrences = new Map();
    for (const [, , , , , outputs] of logicalMonitors) {
        // Mirrored outputs belong to the same logical monitor.
        for (const id of new Set(outputs.map(hardwareId))) {
            if (id !== null) occurrences.set(id, (occurrences.get(id) ?? 0) + 1);
        }
    }

    const identities = new Map();
    for (const [x, y, , , , outputs] of logicalMonitors) {
        const [connector, vendor, product] = outputs[0];
        const hardware = hardwareId(outputs[0]);
        const id = occurrences.get(hardware) === 1 ? hardware : JSON.stringify([vendor, product, connector]);
        identities.set(`${x},${y}`, { id, name: `${product || vendor} · ${connector}` });
    }
    return identities;
}
