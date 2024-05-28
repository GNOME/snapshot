# Troubleshooting

## Flatpak
Snapshot relies on number of modern components which are under rapid development. Thus, in order to simplify debugging, please try the [Flatpack from Flathub](https://flathub.org/apps/org.gnome.Snapshot) before reporting issues.

## Pipewire
Snapshot exclusively uses [Pipewire](https://gitlab.freedesktop.org/pipewire/pipewire/) (from here on **PW**) to access camera devices.

Please restart PW to ensure all camera devices are found:

```
systemctl --user restart pipewire
```

A useful tool to look up information from PW is `pw-dump`. In order to check whether PW currently recognizes any camera devices, run:

```
pw-dump | grep default.video.source
```

If that is not the case, you may want to double-check that all required components for Pipewire camera support are installed, notably:

* [Wireplumber](https://gitlab.freedesktop.org/pipewire/wireplumber) (the PW "session-manager")
* potentially [libcamera](https://libcamera.org/) and the PW libcamera plugin

## XDG Desktop Portal
Snapshot uses the camera portal to request camera access. There are desktop environment specific implementations for it, thus ensure to have the matching one installed:

* [Gnome](https://gitlab.gnome.org/GNOME/xdg-desktop-portal-gnome)
* [KDE](https://github.com/KDE/xdg-desktop-portal-kde)
* [wlroots](https://github.com/emersion/xdg-desktop-portal-wlr) (Sway, Phosh, Hyprland etc.)

If Snapshot can't find any devices, you can check camera permissions in various ways, a simple one being [Flatseal](https://flathub.org/apps/com.github.tchx84.Flatseal).

## Gstreamer
Snapshot uses `GstPipeWire` components. In order to list available cameras and additional information about them, look for entries that contain `gst-launch-1.0 pipewiresrc` when running:

```
flatpak run --command=gst-device-monitor-1.0 org.gnome.Snapshot Video/Source
```

for the Flatpak or

```
gst-device-monitor-1.0 Video/Source
```

for non-Flatpak installations.

In the later case, make sure to have the Gstreamer Pipewire plugin installed.

## Logs
In case the issue persists you can get debug output for the application by
running:

```
RUST_LOG=snapshot=debug,aperture=debug flatpak run org.gnome.Snapshot
```

If you file an issue make sure to include the version info from the
"Troubleshooting" panel in the application's About dialog.
