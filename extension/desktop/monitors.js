// A display with a unique hardware serial keeps its identity when moved to another connector.
function hardwareId([, vendor, product, serial]) {
    if (!serial || serial === '0' || serial === 'unknown') return null;
    return JSON.stringify([vendor, product, serial]);
}

export function monitorIdentities(logicalMonitors, history = new Map(), assignments = {}) {
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
        const port = JSON.stringify([vendor, product, connector]);
        let id = port;
        if (hardware) {
            const previous = history.get(hardware);
            // An older profile can already have connector-specific assignments.
            const hasPortAssignment = Object.hasOwn(assignments, port);
            const duplicate = previous?.duplicate || occurrences.get(hardware) > 1 || hasPortAssignment;
            // Keep the original serial-based assignment on its known connector.
            const anchor = hasPortAssignment && previous?.anchor === connector
                ? null
                : duplicate ? (previous?.anchor ?? null) : connector;
            history.set(hardware, { duplicate, anchor });
            if (!duplicate || anchor === connector) id = hardware;
        }
        identities.set(`${x},${y}`, { id, name: `${product || vendor} · ${connector}` });
    }
    return identities;
}
