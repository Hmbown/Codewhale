package codewhale.pet;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.os.ParcelFileDescriptor;
import java.io.File;
import java.io.FileNotFoundException;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardOpenOption;
import java.util.concurrent.atomic.AtomicInteger;

/** Test APK only. Uses framework/Java APIs because this separate provider
 * process does not load the target app's Kotlin or QuickJS dependencies. */
public class PetLiveTestProvider extends ContentProvider {
    private final AtomicInteger reads = new AtomicInteger();
    private File tape() { return new File(getContext().getCacheDir(), "synthetic-live.jsonl"); }
    @Override public boolean onCreate() { return true; }
    @Override public ParcelFileDescriptor openFile(Uri uri, String mode) throws FileNotFoundException {
        if (!"/tape".equals(uri.getPath()) || !"r".equals(mode)) throw new FileNotFoundException();
        reads.incrementAndGet();
        return ParcelFileDescriptor.open(tape(), ParcelFileDescriptor.MODE_READ_ONLY);
    }
    @Override public Bundle call(String method, String arg, Bundle extras) {
        try {
            switch (method) {
                case "replace": case "append":
                    if (arg == null || arg.length() > 65_536) throw new IllegalArgumentException();
                    if (method.equals("append")) Files.write(tape().toPath(), arg.getBytes(StandardCharsets.UTF_8), StandardOpenOption.CREATE, StandardOpenOption.APPEND);
                    else Files.write(tape().toPath(), arg.getBytes(StandardCharsets.UTF_8));
                    break;
                case "delete": Files.deleteIfExists(tape().toPath()); break;
                case "stats": break;
                default: throw new IllegalArgumentException("Unknown fixture action");
            }
        } catch (IOException error) { throw new IllegalStateException(error); }
        Bundle result = new Bundle(); result.putInt("reads", reads.get()); return result;
    }
    @Override public String getType(Uri uri) { return "text/plain"; }
    @Override public Cursor query(Uri uri, String[] projection, String selection, String[] args, String sort) { return null; }
    @Override public Uri insert(Uri uri, ContentValues values) { throw new UnsupportedOperationException(); }
    @Override public int update(Uri uri, ContentValues values, String selection, String[] args) { throw new UnsupportedOperationException(); }
    @Override public int delete(Uri uri, String selection, String[] args) { throw new UnsupportedOperationException(); }
}
