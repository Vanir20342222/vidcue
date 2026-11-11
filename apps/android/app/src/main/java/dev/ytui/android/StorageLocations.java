package dev.ytui.android;

import android.content.*;
import android.net.Uri;
import android.provider.DocumentsContract;
import java.io.*;
import java.util.*;
import org.json.*;

/** User-granted document trees and app-produced files only. Never stores stream leases here. */
final class StorageLocations {
  static SharedPreferences prefs() {
    return MediaBridge.context.getSharedPreferences("offline-folders", Context.MODE_PRIVATE);
  }

  static Uri folder(boolean audio) {
    String raw = prefs().getString(audio ? "music" : "video", "");
    return raw.isEmpty() ? null : Uri.parse(raw);
  }

  /** DocumentsUI expects a document URI here, not the persisted tree-grant URI. */
  static Uri initialDocument(boolean audio) {
    Uri tree = folder(audio);
    if (tree == null) return null;
    try {
      return DocumentsContract.buildDocumentUri(
          tree.getAuthority(), DocumentsContract.getTreeDocumentId(tree));
    } catch (IllegalArgumentException invalid) {
      return null;
    }
  }

  static String label(boolean audio) {
    Uri folder = folder(audio);
    if (folder == null) return "App storage";
    String name = DocumentsContract.getTreeDocumentId(folder);
    return name.substring(name.lastIndexOf(':') + 1).replace("/", " › ");
  }

  static boolean granted(Uri tree) {
    for (UriPermission permission :
        MediaBridge.context.getContentResolver().getPersistedUriPermissions())
      if (permission.getUri().equals(tree)
          && permission.isReadPermission()
          && permission.isWritePermission()) return true;
    return false;
  }

  static void select(boolean audio, Uri tree, int flags) throws Exception {
    if (!DocumentsContract.isTreeUri(tree)) throw new IOException("Choose a folder");
    int grant =
        flags & (Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
    MediaBridge.context.getContentResolver().takePersistableUriPermission(tree, grant);
    if (!granted(tree)) throw new IOException("The folder must allow reading and writing");
    if (!prefs().edit().putString(audio ? "music" : "video", tree.toString()).commit())
      throw new IOException("Could not save folder");
    OfflineStore.exportPending();
    MediaBridge.event("storage_updated", "");
  }

  static synchronized JSONObject records() throws JSONException {
    return new JSONObject(prefs().getString("files", "{}"));
  }

  static synchronized JSONObject record(String id) throws JSONException {
    return records().optJSONObject(id);
  }

  static synchronized void remember(
      String id, Uri document, Uri playlistFolder, String playlistTitle, JSONObject request)
      throws Exception {
    if (prefs().getBoolean("removed:" + id, false))
      throw new InterruptedIOException("Offline copy removed");
    JSONObject files = records();
    JSONObject media = new JSONObject(request.getJSONObject("media").toString());
    media.put("offline_id", id);
    String folder = label(request.optBoolean("audio"));
    if (playlistFolder != null) folder = folder + " › " + playlistTitle;
    JSONObject file =
        new JSONObject()
            .put("id", id)
            .put("uri", document.toString())
            .put("media", media)
            .put("audio", request.optBoolean("audio"))
            .put("folder", folder);
    if (playlistFolder != null) file.put("playlistFolder", playlistFolder.toString());
    files.put(id, file);
    String encoded = files.toString();
    if (files.length() > 2000 || encoded.length() > 4 * 1024 * 1024)
      throw new IOException("Offline folder index is full");
    if (!prefs().edit().putString("files", encoded).remove("error:" + id).commit())
      throw new IOException("Could not save offline file index");
    OfflineStore.dirty = true;
  }

  static JSONObject request(String id) throws Exception {
    JSONObject file = record(id);
    if (file == null) throw new IOException("Offline file is unavailable");
    Uri uri = Uri.parse(file.getString("uri"));
    try (android.content.res.AssetFileDescriptor descriptor =
        MediaBridge.context.getContentResolver().openAssetFileDescriptor(uri, "r")) {
      if (descriptor == null) throw new IOException("Offline folder cannot be read");
    }
    return new JSONObject()
        .put("media", file.getJSONObject("media"))
        .put("audio", file.optBoolean("audio"))
        .put("stream", new JSONObject().put("url", uri.toString()))
        .put("local", true);
  }

  static synchronized void remove(String id) throws Exception {
    JSONObject files = records(), file = files.optJSONObject(id);
    if (file != null) {
      try {
        if (!DocumentsContract.deleteDocument(
            MediaBridge.context.getContentResolver(), Uri.parse(file.getString("uri"))))
          throw new IOException("Cannot remove the offline file; reconnect its folder");
      } catch (FileNotFoundException alreadyRemoved) {
        /* remove its stale index only */
      }
      // Best-effort cleanup of the playlist folder this file was packed into; it stays
      // while it still holds other files.
      String playlistFolder = file.optString("playlistFolder", "");
      if (!playlistFolder.isEmpty()) deleteIfEmpty(Uri.parse(playlistFolder));
      files.remove(id);
    }
    if (!prefs().edit().putString("files", files.toString()).remove("error:" + id).commit())
      throw new IOException("Could not update offline library");
    OfflineStore.dirty = true;
  }

  static JSONArray rows() throws Exception {
    JSONObject files = records();
    JSONArray rows = new JSONArray();
    for (Iterator<String> ids = files.keys(); ids.hasNext(); ) {
      String id = ids.next();
      JSONObject item = files.getJSONObject(id);
      rows.put(
          new JSONObject()
              .put("id", id)
              .put("title", item.getJSONObject("media").optString("title"))
              .put("state", "Complete")
              .put("media", item.getJSONObject("media"))
              .put("audio", item.optBoolean("audio"))
              .put("folder", item.optString("folder"))
              .put("progress", 1));
    }
    return rows;
  }

  /**
   * The folder that the first downloaded member of an offline playlist was packed into, or null
   * when none of its downloads has one yet.
   */
  static Uri playlistFolder(String playlistId) throws Exception {
    JSONArray playlists =
        new JSONArray(
            NativeBridge.request(new JSONObject().put("op", "offline_playlists").toString()));
    for (int i = 0; i < playlists.length(); i++) {
      JSONObject playlist = playlists.getJSONObject(i);
      if (!playlistId.equals(playlist.optString("id"))) continue;
      JSONArray items = playlist.optJSONArray("items");
      for (int j = 0; items != null && j < items.length(); j++) {
        JSONObject file = record(items.getJSONObject(j).optString("offline_id"));
        String folder = file == null ? "" : file.optString("playlistFolder", "");
        if (!folder.isEmpty()) return Uri.parse(folder);
      }
    }
    return null;
  }

  /**
   * The folder named [name] directly inside the granted [tree], created when it does not exist
   * yet. Every member of a playlist packs into this one folder; creating it per download made
   * the provider add "Name (1)", "Name (2)", ... folders. Synchronized so two saves of the same
   * playlist cannot both create it.
   */
  static synchronized Uri childFolder(Uri tree, String name) throws IOException {
    ContentResolver resolver = MediaBridge.context.getContentResolver();
    String root = DocumentsContract.getTreeDocumentId(tree);
    Uri children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, root);
    try (android.database.Cursor cursor =
        resolver.query(
            children,
            new String[] {
              DocumentsContract.Document.COLUMN_DOCUMENT_ID,
              DocumentsContract.Document.COLUMN_DISPLAY_NAME,
              DocumentsContract.Document.COLUMN_MIME_TYPE
            },
            null,
            null,
            null)) {
      while (cursor != null && cursor.moveToNext()) {
        if (name.equals(cursor.getString(1))
            && DocumentsContract.Document.MIME_TYPE_DIR.equals(cursor.getString(2)))
          return DocumentsContract.buildDocumentUriUsingTree(tree, cursor.getString(0));
      }
    }
    Uri created =
        DocumentsContract.createDocument(
            resolver,
            DocumentsContract.buildDocumentUriUsingTree(tree, root),
            DocumentsContract.Document.MIME_TYPE_DIR,
            name);
    if (created == null) throw new IOException("Could not create the playlist folder");
    return created;
  }

  /**
   * Deletes [folder] only when nothing is left in it. Providers delete a folder with everything
   * inside, so a shared playlist folder must never be removed while it holds other files.
   */
  static void deleteIfEmpty(Uri folder) {
    ContentResolver resolver = MediaBridge.context.getContentResolver();
    try {
      Uri children =
          DocumentsContract.buildChildDocumentsUriUsingTree(
              folder, DocumentsContract.getDocumentId(folder));
      try (android.database.Cursor cursor =
          resolver.query(
              children,
              new String[] {DocumentsContract.Document.COLUMN_DOCUMENT_ID},
              null,
              null,
              null)) {
        if (cursor == null || cursor.getCount() != 0) return;
      }
      DocumentsContract.deleteDocument(resolver, folder);
    } catch (Exception staleOrUnreadable) {
      // Leave it: an unreadable folder is never deleted blindly.
    }
  }

  static void failed(String id) {
    prefs()
        .edit()
        .putString(
            "error:" + id,
            "Folder save failed; cached media is intact. Reconnect its folder and retry.")
        .apply();
    OfflineStore.dirty = true;
  }

  private StorageLocations() {}
}
