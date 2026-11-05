package dev.ytui.android;

import android.media.AudioAttributes;
import android.media.AudioDeviceAttributes;
import android.media.AudioDeviceInfo;
import android.media.AudioManager;
import android.os.Build;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Media outputs the player can be routed to (speaker, wired, USB and Bluetooth), and a best
 * guess of the one in use. Call/SCO routes and virtual devices are left out so the picker only
 * shows places music can actually play.
 */
final class AudioOutputs {
  private AudioOutputs() {}

  static String kind(AudioDeviceInfo device) {
    switch (device.getType()) {
      case AudioDeviceInfo.TYPE_BLUETOOTH_A2DP:
      case AudioDeviceInfo.TYPE_HEARING_AID:
      case AudioDeviceInfo.TYPE_BLE_HEADSET:
      case AudioDeviceInfo.TYPE_BLE_SPEAKER:
        return "bluetooth";
      case AudioDeviceInfo.TYPE_WIRED_HEADSET:
      case AudioDeviceInfo.TYPE_WIRED_HEADPHONES:
        return "wired";
      case AudioDeviceInfo.TYPE_USB_HEADSET:
      case AudioDeviceInfo.TYPE_USB_DEVICE:
        return "usb";
      case AudioDeviceInfo.TYPE_BUILTIN_SPEAKER:
        return "speaker";
      default:
        return "";
    }
  }

  static String name(AudioDeviceInfo device) {
    String product = device.getProductName() == null ? "" : device.getProductName().toString().trim();
    switch (kind(device)) {
      case "speaker":
        return "This device";
      case "wired":
        return "Wired headphones";
      case "usb":
        return product.isEmpty() ? "USB audio" : product;
      default:
        return product.isEmpty() ? "Bluetooth device" : product;
    }
  }

  /** Connected outputs, one entry per physical device (Bluetooth can expose several types). */
  static List<AudioDeviceInfo> list(AudioManager audio) {
    List<AudioDeviceInfo> out = new ArrayList<>();
    if (audio == null) return out;
    Set<String> seen = new HashSet<>();
    for (AudioDeviceInfo device : audio.getDevices(AudioManager.GET_DEVICES_OUTPUTS)) {
      String kind = kind(device);
      if (kind.isEmpty() || !seen.add(kind + "\u0000" + name(device))) continue;
      out.add(device);
    }
    return out;
  }

  static AudioDeviceInfo find(AudioManager audio, int id) {
    for (AudioDeviceInfo device : list(audio)) if (device.getId() == id) return device;
    return null;
  }

  /** The chosen device while connected; otherwise what the system routes media to. */
  static AudioDeviceInfo active(AudioManager audio, AudioDeviceInfo preferred) {
    List<AudioDeviceInfo> devices = list(audio);
    if (preferred != null)
      for (AudioDeviceInfo device : devices) if (device.getId() == preferred.getId()) return device;
    if (Build.VERSION.SDK_INT >= 33) {
      AudioAttributes media =
          new AudioAttributes.Builder()
              .setUsage(AudioAttributes.USAGE_MEDIA)
              .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
              .build();
      for (AudioDeviceAttributes routed : audio.getAudioDevicesForAttributes(media))
        for (AudioDeviceInfo device : devices)
          if (device.getType() == routed.getType()
              && (routed.getAddress().isEmpty() || routed.getAddress().equals(device.getAddress())))
            return device;
    }
    // Older releases: media follows the newest accessory, which is nearly always one of these.
    for (String kind : new String[] {"bluetooth", "wired", "usb", "speaker"})
      for (AudioDeviceInfo device : devices) if (kind(device).equals(kind)) return device;
    return null;
  }

  static JSONObject describe(AudioManager audio, AudioDeviceInfo preferred) throws JSONException {
    JSONObject out = new JSONObject();
    AudioDeviceInfo active = audio == null ? null : active(audio, preferred);
    out.put("id", active == null ? 0 : active.getId())
        .put("kind", active == null ? "" : kind(active))
        .put("name", active == null ? "" : name(active))
        .put("automatic", preferred == null);
    JSONArray devices = new JSONArray();
    for (AudioDeviceInfo device : list(audio))
      devices.put(
          new JSONObject()
              .put("id", device.getId())
              .put("kind", kind(device))
              .put("name", name(device)));
    return out.put("devices", devices);
  }
}
