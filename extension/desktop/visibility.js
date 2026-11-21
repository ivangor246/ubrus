import Cogl from 'gi://Cogl';
import Meta from 'gi://Meta';
import { covered } from './geometry.js';

// Sample the workspace once per refresh, then reuse it for every monitor.
export function workspaceCoverage(enabled) {
    const rects = [],
        opaque = [],
        fullscreen = [];
    if (enabled) {
        const workspace = global.workspace_manager.get_active_workspace();
        for (const w of workspace.list_windows()) {
            const actor = w.get_compositor_private();
            if (
                !actor ||
                actor.opacity !== 255 ||
                w.minimized ||
                !w.showing_on_its_workspace() ||
                ![Meta.WindowType.NORMAL, Meta.WindowType.DIALOG, Meta.WindowType.MODAL_DIALOG].includes(
                    w.get_window_type(),
                )
            )
                continue;
            const r = w.get_frame_rect();
            rects.push(r);
            if (w.is_fullscreen()) fullscreen.push(r);
            const multi = actor.get_texture?.()?.get_texture?.();
            const texture = multi?.is_simple?.() ? multi.get_plane(0) : null;
            if (
                texture?.get_components &&
                Cogl.TextureComponents &&
                texture.get_components() === Cogl.TextureComponents.RGB
            )
                opaque.push(r);
        }
    }
    return (area, surfaceObscured) =>
        covered(area, fullscreen) || covered(area, opaque) || (surfaceObscured && covered(area, rects));
}
