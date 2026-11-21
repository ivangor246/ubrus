FROM ubuntu:26.04
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    dbus-daemon gnome-shell dbus-x11 python3-gi python3-gi-cairo gir1.2-gtk-4.0 gstreamer1.0-tools gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-base gstreamer1.0-gl gstreamer1.0-libav libegl-mesa0 libgl1-mesa-dri \
    webp-pixbuf-loader xdg-desktop-portal-gnome gsettings-desktop-schemas adwaita-icon-theme fonts-dejavu-core upower && rm -rf /var/lib/apt/lists/*
WORKDIR /src
CMD ["bash","tests/docker/container-session.sh"]
