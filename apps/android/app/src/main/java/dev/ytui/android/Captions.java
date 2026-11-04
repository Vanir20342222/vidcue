package dev.ytui.android;

import androidx.media3.common.C;
import androidx.media3.common.Format;
import androidx.media3.common.MediaItem;
import androidx.media3.common.MimeTypes;
import androidx.media3.common.Tracks;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Caption choices for one video. YouTube offers the uploader's captions, an automatically
 * generated track in the spoken language and machine translations into well over a hundred
 * languages. Attaching them in catalogue order buried the useful ones behind an endless
 * alphabetical list, so they are ordered by use and given readable names.
 */
final class Captions {
  /** The uploader's own captions. */
  static final String UPLOADED = "uploaded";
  /** YouTube's automatic captions in the spoken language. */
  static final String GENERATED = "generated";
  /** Machine translations of the automatic captions. */
  static final String TRANSLATED = "translated";
  /** Captions carried inside the stream itself. */
  static final String EMBEDDED = "embedded";

  /** Media3 attaches each sideloaded track as its own source; keep that bounded. */
  static final int LIMIT = 40;

  private Captions() {}

  /** Lowercase base language codes the user reads, most preferred first, plus English. */
  static List<String> deviceLanguages() {
    List<String> languages = new ArrayList<>();
    android.os.LocaleList locales = android.os.LocaleList.getDefault();
    for (int i = 0; i < locales.size(); i++) addBase(languages, locales.get(i).getLanguage());
    addBase(languages, "en");
    return languages;
  }

  private static void addBase(List<String> languages, String code) {
    String base = base(code);
    if (!base.isEmpty() && !languages.contains(base)) languages.add(base);
  }

  /** "en-orig" and "en-US" both read as "en". */
  static String base(String code) {
    if (code == null) return "";
    String trimmed = code.trim().toLowerCase(Locale.ROOT);
    int cut = trimmed.indexOf('-');
    return cut < 0 ? trimmed : trimmed.substring(0, cut);
  }

  /** A language name in the user's own language, for example "German" or "Deutsch". */
  static String displayName(String code) {
    String tag = code == null ? "" : code.replace("-orig", "");
    Locale locale = Locale.forLanguageTag(tag);
    String name = locale.getDisplayName(Locale.getDefault());
    if (name.isEmpty() || name.equalsIgnoreCase(tag)) return tag.isEmpty() ? "Unknown" : tag;
    return name.substring(0, 1).toUpperCase(Locale.getDefault()) + name.substring(1);
  }

  /** Readable label for a choice of the given kind. */
  static String label(String code, String kind) {
    String name = displayName(code);
    switch (kind) {
      case GENERATED:
        return name + " (auto-generated)";
      case TRANSLATED:
        return name + " (auto-translated)";
      default:
        return name;
    }
  }

  private static final class Choice {
    final String url, code, kind, label;
    final int rank;

    Choice(String url, String code, String kind, String label, int rank) {
      this.url = url;
      this.code = code;
      this.kind = kind;
      this.label = label;
      this.rank = rank;
    }
  }

  /**
   * Build the sideloaded caption tracks from the engine's catalogue. Each catalogue label reads
   * "{code} · captions {name}" or "{code} · automatic {name}". [kinds] receives the kind of
   * every attached label so the published track list can tell them apart.
   */
  static List<MediaItem.SubtitleConfiguration> configurations(
      JSONArray catalogue, Map<String, String> kinds) {
    kinds.clear();
    List<MediaItem.SubtitleConfiguration> out = new ArrayList<>();
    if (catalogue == null) return out;
    List<String> device = deviceLanguages();
    String spoken = "";
    List<Choice> choices = new ArrayList<>();
    Set<String> labels = new HashSet<>();
    for (int i = 0; i < catalogue.length(); i++) {
      JSONObject row = catalogue.optJSONObject(i);
      if (row == null || row.optString("url").isEmpty()) continue;
      String raw = row.optString("label");
      String code = raw.split(" ", 2)[0].trim();
      boolean automatic = raw.contains("· automatic");
      String kind =
          !automatic ? UPLOADED : code.toLowerCase(Locale.ROOT).endsWith("-orig") ? GENERATED : TRANSLATED;
      if (GENERATED.equals(kind) && spoken.isEmpty()) spoken = base(code);
      String label = label(code, kind);
      if (!labels.add(label)) continue;
      int preference = device.indexOf(base(code));
      int rank;
      switch (kind) {
        case UPLOADED:
          rank = preference >= 0 ? preference : 100;
          break;
        case GENERATED:
          rank = 200;
          break;
        default:
          rank = preference >= 0 ? 300 + preference : 1000;
      }
      choices.add(new Choice(row.optString("url"), code, kind, label, rank));
    }
    // A translation into the spoken language duplicates the generated track.
    final String original = spoken;
    choices.removeIf(c -> TRANSLATED.equals(c.kind) && !original.isEmpty() && base(c.code).equals(original));
    choices.sort(
        Comparator.<Choice>comparingInt(c -> c.rank)
            .thenComparing(c -> c.label, String.CASE_INSENSITIVE_ORDER));
    for (Choice choice : choices) {
      if (out.size() >= LIMIT) break;
      kinds.put(choice.label, choice.kind);
      out.add(
          new MediaItem.SubtitleConfiguration.Builder(android.net.Uri.parse(choice.url))
              .setMimeType(MimeTypes.TEXT_VTT)
              .setLabel(choice.label)
              .setLanguage(base(choice.code))
              .build());
    }
    return out;
  }

  /** The kind of a published text track; captions inside the stream are "embedded". */
  static String kind(Format format, Map<String, String> kinds) {
    String kind = format.label == null ? null : kinds.get(format.label);
    return kind == null ? EMBEDDED : kind;
  }

  /**
   * The track to show when captions are switched on without a choice: the uploader's captions
   * in a language the user reads, then the automatic captions in the spoken language, then a
   * translation into a language the user reads, then any track. Returns {group, index}.
   */
  static int[] best(Tracks tracks, Map<String, String> kinds) {
    List<String> device = deviceLanguages();
    int[] best = null;
    int bestScore = Integer.MAX_VALUE;
    List<Tracks.Group> groups = tracks.getGroups();
    for (int g = 0; g < groups.size(); g++) {
      Tracks.Group group = groups.get(g);
      if (group.getType() != C.TRACK_TYPE_TEXT) continue;
      for (int i = 0; i < group.length; i++) {
        if (!group.isTrackSupported(i)) continue;
        Format format = group.getTrackFormat(i);
        int preference = device.indexOf(base(format.language));
        String kind = kind(format, kinds);
        int score;
        if ((UPLOADED.equals(kind) || EMBEDDED.equals(kind)) && preference >= 0) score = preference;
        else if (GENERATED.equals(kind)) score = 100;
        else if (TRANSLATED.equals(kind) && preference >= 0) score = 200 + preference;
        else if (UPLOADED.equals(kind) || EMBEDDED.equals(kind)) score = 300;
        else score = 1000;
        if (score < bestScore) {
          bestScore = score;
          best = new int[] {g, i};
        }
      }
    }
    return best;
  }
}
