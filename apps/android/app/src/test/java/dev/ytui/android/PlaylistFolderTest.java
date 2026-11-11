package dev.ytui.android;

import static org.junit.Assert.*;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.Bundle;
import android.provider.DocumentsContract;
import android.provider.DocumentsContract.Document;
import java.util.*;
import org.junit.*;
import org.junit.runner.RunWith;
import org.robolectric.Robolectric;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;
import org.robolectric.annotation.Config;

/**
 * Playlist downloads share one folder per playlist, and removing a download only removes that
 * folder once it is empty. Runs against an in-memory document provider.
 */
@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 35)
public class PlaylistFolderTest {
  private static final String AUTHORITY = "dev.ytui.test.documents";
  /** DocumentsContract.EXTRA_URI, which the SDK hides. */
  private static final String EXTRA_URI = "uri";

  /** A minimal tree: document ids are paths, "root" is the granted folder. */
  public static final class Provider extends ContentProvider {
    static final Map<String, String> MIME = new LinkedHashMap<>();
    static int creates;

    @Override
    public boolean onCreate() {
      return true;
    }

    private static String parentOf(String id) {
      int slash = id.lastIndexOf('/');
      return slash < 0 ? null : id.substring(0, slash);
    }

    private static String name(String id) {
      return id.substring(id.lastIndexOf('/') + 1);
    }

    @Override
    public Cursor query(Uri uri, String[] projection, String s, String[] a, String o) {
      String parent = DocumentsContract.getDocumentId(uri);
      MatrixCursor cursor = new MatrixCursor(projection);
      for (Map.Entry<String, String> entry : MIME.entrySet()) {
        if (!parent.equals(parentOf(entry.getKey()))) continue;
        Object[] row = new Object[projection.length];
        for (int i = 0; i < projection.length; i++)
          row[i] =
              projection[i].equals(Document.COLUMN_DOCUMENT_ID)
                  ? entry.getKey()
                  : projection[i].equals(Document.COLUMN_DISPLAY_NAME)
                      ? name(entry.getKey())
                      : entry.getValue();
        cursor.addRow(row);
      }
      return cursor;
    }

    @Override
    public Bundle call(String method, String arg, Bundle extras) {
      Uri uri = extras.getParcelable(EXTRA_URI);
      String id = DocumentsContract.getDocumentId(uri);
      Bundle out = new Bundle();
      if (method.equals("android:createDocument")) {
        creates++;
        String base = id + "/" + extras.getString(Document.COLUMN_DISPLAY_NAME), child = base;
        // Like real providers: a taken name gets " (1)", " (2)", ...
        for (int n = 1; MIME.containsKey(child); n++) child = base + " (" + n + ")";
        MIME.put(child, extras.getString(Document.COLUMN_MIME_TYPE));
        out.putParcelable(
            EXTRA_URI,
            DocumentsContract.buildDocumentUriUsingTree(
                DocumentsContract.buildTreeDocumentUri(AUTHORITY, "root"), child));
      } else if (method.equals("android:deleteDocument")) {
        // Real providers delete a folder with everything in it.
        MIME.keySet().removeIf(key -> key.equals(id) || key.startsWith(id + "/"));
      }
      return out;
    }

    @Override
    public String getType(Uri uri) {
      return MIME.get(DocumentsContract.getDocumentId(uri));
    }

    @Override
    public Uri insert(Uri uri, ContentValues values) {
      return null;
    }

    @Override
    public int delete(Uri uri, String s, String[] a) {
      return 0;
    }

    @Override
    public int update(Uri uri, ContentValues v, String s, String[] a) {
      return 0;
    }
  }

  private android.app.Application previous;
  private final Uri tree = DocumentsContract.buildTreeDocumentUri(AUTHORITY, "root");

  @Before
  public void setUp() {
    previous = MediaBridge.context;
    MediaBridge.context = RuntimeEnvironment.getApplication();
    Provider.MIME.clear();
    Provider.MIME.put("root", Document.MIME_TYPE_DIR);
    Provider.creates = 0;
    Robolectric.setupContentProvider(Provider.class, AUTHORITY);
  }

  @After
  public void tearDown() {
    MediaBridge.context = previous;
  }

  @Test
  public void everyDownloadOfAPlaylistUsesTheSameFolder() throws Exception {
    Uri first = StorageLocations.childFolder(tree, "Road trip");
    Uri second = StorageLocations.childFolder(tree, "Road trip");
    assertEquals(first, second);
    assertEquals("root/Road trip", DocumentsContract.getDocumentId(first));
    assertEquals(1, Provider.creates);
    assertFalse(Provider.MIME.containsKey("root/Road trip (1)"));
    assertNotEquals(first, StorageLocations.childFolder(tree, "Other"));
  }

  @Test
  public void aFileWithThePlaylistNameIsNotMistakenForItsFolder() throws Exception {
    Provider.MIME.put("root/Road trip", "audio/mpeg");
    Uri folder = StorageLocations.childFolder(tree, "Road trip");
    assertEquals("root/Road trip (1)", DocumentsContract.getDocumentId(folder));
  }

  @Test
  public void aFolderIsOnlyDeletedOnceItIsEmpty() throws Exception {
    Uri folder = StorageLocations.childFolder(tree, "Road trip");
    Provider.MIME.put("root/Road trip/001 - One.mp3", "audio/mpeg");
    StorageLocations.deleteIfEmpty(folder);
    assertTrue("a folder with another track must stay", Provider.MIME.containsKey("root/Road trip"));
    assertTrue(Provider.MIME.containsKey("root/Road trip/001 - One.mp3"));
    Provider.MIME.remove("root/Road trip/001 - One.mp3");
    StorageLocations.deleteIfEmpty(folder);
    assertFalse(Provider.MIME.containsKey("root/Road trip"));
  }
}
