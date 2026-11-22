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

// Mutter tracks occlusion on the subsurface that actually presents the client buffer.
export function previewVisible(pid) {
    if (!pid) return null;
    const actor = global.get_window_actors().find(a =>
        a.meta_window.get_pid() === pid && a.meta_window.get_title() === 'Ubrus');
    if (!actor) return null;
    const window = actor.meta_window;
    const workspace = global.workspace_manager.get_active_workspace();
    if (window.minimized || !window.located_on_workspace(workspace) || !window.showing_on_its_workspace()) return false;
    const stack = [actor];
    while (stack.length) {
        const child = stack.pop();
        if (typeof child.is_obscured === 'boolean') return !child.is_obscured;
        stack.push(...child.get_children());
    }
    return true;
}
