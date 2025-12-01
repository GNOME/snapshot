#pragma once

#include <glib-object.h>
#include <gio/gio.h>
#include <gtk/gtk.h>
#include <stdint.h>

G_BEGIN_DECLS

#define APERTURE_TYPE_LOADER (aperture_camera_get_type())
G_DECLARE_FINAL_TYPE(ApertureCamera, aperture_camera, APERTURE, CAMERA, GObject)

#define APERTURE_TYPE_DEVICE_PROVIDER (aperture_device_provider_get_type())
G_DECLARE_FINAL_TYPE(ApertureDeviceProvider, aperture_device_provider, APERTURE, DEVICE_PROVIDER, GObject)

#define APERTURE_TYPE_VIEWFINDER (aperture_viewfinder_get_type())
G_DECLARE_FINAL_TYPE(ApertureViewfinder, aperture_viewfinder, APERTURE, VIEWFINDER, GtkWidget)

void aperture_init (const *gchar app_id);

const *gchar aperture_camera_get_display_name (ApertureCamera *self);
void         aperture_camera_get_properties   (ApertureCamera *self);

ApertureDeviceProvider *aperture_device_provider_get_default (void);
gboolean                aperture_device_provider_start       (ApertureDeviceProvider *self);
ApertureCamera         *aperture_device_provider_get_camera  (ApertureDeviceProvider *self,
                                                              uint32_t                camera_id);

ApertureViewfinder *aperture_viewfinder_new             (void) G_GNUC_WARN_UNUSED_RESULT;
gboolean            aperture_viewfinder_take_picture    (ApertureViewfinder *self,
                                                         const *char location,
                                                         GError **error);
gboolean            aperture_viewfinder_start_recording (ApertureViewfinder *self,
                                                         const *char location,
                                                         GError **error);
gboolean            aperture_viewfinder_stop_recording  (ApertureViewfinder *self,
                                                         GError **error);

G_END_DECLS
