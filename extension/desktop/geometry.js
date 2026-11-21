// Subtract each opaque rectangle from the pieces of the desktop that are still visible.
export function covered(area, windows) {
    let visible = [area];
    for (const win of windows) {
        const remaining = [];
        for (const r of visible) {
            const right = r.x + r.width,
                bottom = r.y + r.height;
            const x = Math.max(r.x, win.x),
                y = Math.max(r.y, win.y);
            const x2 = Math.min(right, win.x + win.width),
                y2 = Math.min(bottom, win.y + win.height);
            if (x >= x2 || y >= y2) {
                remaining.push(r);
                continue;
            }
            if (r.y < y) remaining.push({ x: r.x, y: r.y, width: r.width, height: y - r.y });
            if (bottom > y2) remaining.push({ x: r.x, y: y2, width: r.width, height: bottom - y2 });
            if (r.x < x) remaining.push({ x: r.x, y, width: x - r.x, height: y2 - y });
            if (right > x2) remaining.push({ x: x2, y, width: right - x2, height: y2 - y });
        }
        visible = remaining;
        if (!visible.length) return true;
    }
    return false;
}
export function pauseReason(scene, saving, occluded, overview) {
    if (scene.paused) return 'manual';
    if (scene.power && saving) return 'power-saver';
    if (overview && scene.pause_in_overview) return 'overview';
    return occluded ? 'covered' : 'visible';
}

// The source already contains the monitor's Fit/Fill composition.
export function overviewCrop(sourceWidth, sourceHeight, width, height) {
    if (Math.min(sourceWidth, sourceHeight, width, height) <= 0) return { x1: 0, y1: 0, x2: 0, y2: 0 };
    const scale = Math.max(width / sourceWidth, height / sourceHeight);
    const x = (width - sourceWidth * scale) / 2;
    const y = (height - sourceHeight * scale) / 2;
    return { x1: x, y1: y, x2: width - x, y2: height - y };
}

export function frameBounds(selection, width, height) {
    const [sw, sh] = selection.dimensions;
    const frame = selection.frame ?? {};
    const scale =
        ((selection.fit ? Math.min : Math.max)(width / sw, height / sh) * (frame.zoom ?? 100)) / 100;
    const w = sw * scale,
        h = sh * scale;
    return [((width - w) * (frame.x ?? 50)) / 100, ((height - h) * (frame.y ?? 50)) / 100, w, h];
}
