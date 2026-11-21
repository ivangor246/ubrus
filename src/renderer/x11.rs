// GTK4 exposes the XID through GDK's X11 ABI. Set the EWMH type before Mutter sees the initial map.
use gtk::{gdk, glib::translate::ToGlibPtr, prelude::*};
use std::ffi::{c_char, c_int, c_ulong, c_void};

#[link(name = "gtk-4")]
unsafe extern "C" {
    fn gdk_x11_display_get_xdisplay(display: *mut gdk::ffi::GdkDisplay) -> *mut c_void;
    fn gdk_x11_surface_get_xid(surface: *mut gdk::ffi::GdkSurface) -> c_ulong;
    fn gdk_x11_surface_set_skip_taskbar_hint(surface: *mut gdk::ffi::GdkSurface, skip: c_int);
    fn gdk_x11_surface_set_skip_pager_hint(surface: *mut gdk::ffi::GdkSurface, skip: c_int);
}
#[link(name = "X11")]
unsafe extern "C" {
    fn XInternAtom(display: *mut c_void, name: *const c_char, only_if_exists: c_int) -> c_ulong;
    fn XChangeProperty(
        display: *mut c_void,
        window: c_ulong,
        property: c_ulong,
        ty: c_ulong,
        format: c_int,
        mode: c_int,
        data: *const u8,
        elements: c_int,
    ) -> c_int;
}
pub fn desktop(surface: &gdk::Surface) {
    if surface.display().type_().name() != "GdkX11Display" {
        return;
    }
    // The main thread owns the realized surface and display for all calls in this block.
    unsafe {
        let s = surface.to_glib_none().0;
        let d = gdk_x11_display_get_xdisplay(surface.display().to_glib_none().0);
        let key = XInternAtom(d, c"_NET_WM_WINDOW_TYPE".as_ptr(), 0);
        let value = XInternAtom(d, c"_NET_WM_WINDOW_TYPE_DESKTOP".as_ptr(), 0);
        XChangeProperty(
            d,
            gdk_x11_surface_get_xid(s),
            key,
            4,
            32,
            0,
            (&raw const value).cast(),
            1,
        );
        gdk_x11_surface_set_skip_taskbar_hint(s, 1);
        gdk_x11_surface_set_skip_pager_hint(s, 1);
    }
}
